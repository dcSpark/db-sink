use cml_core::serialization::{Serialize, ToBytes};
use cml_multi_era::utils::MultiEraTransactionOutput;
use cml_multi_era::MultiEraTransactionBody;
use std::collections::BTreeMap;

use entity::{
    prelude::*,
    sea_orm::{prelude::*, DatabaseTransaction, Set},
};

use super::multiera_address::MultieraAddressTask;
use crate::config::ReadonlyConfig::ReadonlyConfig;
use crate::dsl::database_task::BlockGlobalInfo;
use crate::dsl::task_macro::*;
use crate::era_common::get_truncated_address;
use crate::era_common::output_from_pointer;

carp_task! {
  name MultieraOutputTask;
  configuration ReadonlyConfig;
  doc "Adds the used outputs to the database (regular inputs in most cases, collateral inputs if tx fails)";
  era multiera;
  dependencies [MultieraAddressTask];
  read [multiera_txs, multiera_addresses];
  write [multiera_outputs];
  should_add_task |block, _properties| {
    // recall: txs may have no outputs if they just burn all inputs as fee
    block.1.transaction_bodies().iter().any(|tx| !tx.outputs().is_empty())
  };
  execute |previous_data, task| handle_output(
      task.db_tx,
      task.block,
      &previous_data.multiera_txs,
      &previous_data.multiera_addresses,
      task.config.readonly
  );
  merge_result |previous_data, result| {
    *previous_data.multiera_outputs = result;
  };
}

struct QueuedOutput {
    // note: no need to use a map type
    // because the pair <tx_id, idx> should only ever be inserted once
    tx_id: i64,
    idx: usize,
    payload: Vec<u8>,
    address: Vec<u8>, // pallas::crypto::hash::Hash<32>
}

async fn handle_output(
    db_tx: &DatabaseTransaction,
    block: BlockInfo<'_, cml_multi_era::MultiEraBlock, BlockGlobalInfo>,
    multiera_txs: &[TransactionModel],
    addresses: &BTreeMap<Vec<u8>, AddressInBlock>,
    readonly: bool,
) -> Result<Vec<TransactionOutputModel>, DbErr> {
    let mut queued_output = Vec::<QueuedOutput>::default();

    for (tx_body, cardano_transaction) in block.1.transaction_bodies().iter().zip(multiera_txs) {
        let outputs = tx_body.outputs();
        if cardano_transaction.is_valid {
            for (idx, output) in outputs.iter().enumerate() {
                queue_output(&mut queued_output, cardano_transaction.id, output, idx);
            }
        }
        if !cardano_transaction.is_valid {
            if let Some(output) = tx_body.collateral_return() {
                queue_output(
                    &mut queued_output,
                    cardano_transaction.id,
                    &output,
                    // only one collateral output is allowed
                    // and its index is output.len()
                    outputs.len(),
                );
            }
        }
    }

    if readonly {
        Ok(output_from_pointer(
            db_tx,
            queued_output
                .iter()
                .map(|output| (output.tx_id, output.idx))
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .await?)
    } else {
        Ok(insert_outputs(addresses, &queued_output, db_tx).await?)
    }
}

fn queue_output(
    queued_output: &mut Vec<QueuedOutput>,
    tx_id: i64,
    output: &MultiEraTransactionOutput,
    idx: usize,
) {
    let addr = output.address();

    queued_output.push(QueuedOutput {
        payload: match output {
            MultiEraTransactionOutput::Byron(byron) => byron.to_bytes(),
            MultiEraTransactionOutput::Shelley(shelley) => shelley.to_canonical_cbor_bytes(),
        },
        address: addr.to_raw_bytes().to_vec(),
        tx_id,
        idx,
    });
}

async fn insert_outputs(
    address_to_model_map: &BTreeMap<Vec<u8>, AddressInBlock>,
    queued_output: &[QueuedOutput],
    txn: &DatabaseTransaction,
) -> Result<Vec<TransactionOutputModel>, DbErr> {
    if queued_output.is_empty() {
        return Ok(vec![]);
    };

    TransactionOutput::insert_many(queued_output.iter().map(|entry| {
        TransactionOutputActiveModel {
            address_id: Set(address_to_model_map
                .get(get_truncated_address(&entry.address))
                .unwrap()
                .model
                .id),
            tx_id: Set(entry.tx_id),
            payload: Set(entry.payload.clone()),
            output_index: Set(entry.idx as i32),
            ..Default::default()
        }
    }))
    .exec_many_with_returning(txn)
    .await
}

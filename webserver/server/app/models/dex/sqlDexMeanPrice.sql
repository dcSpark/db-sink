/* @name sqlDexMeanPrice */
WITH "AssetPairs" AS (
  SELECT policy_id1, asset_name1, policy_id2, asset_name2
  FROM
    unnest(
      /*
      Aparrently, we can't make pgtyped understand that these are actually (bytea | NULL)[].
      We will pass in ('', '') instead of (NULL, NULL) for ADA and do the NULL->'' conversion
      below when filtering the assets (see the COALESCE).
      */
      (:policy_id1)::bytea[],
      (:asset_name1)::bytea[],
      (:policy_id2)::bytea[],
      (:asset_name2)::bytea[]
    ) x(policy_id1, asset_name1, policy_id2, asset_name2)
)
SELECT
  "Transaction".hash AS tx_hash,
  "Address".payload AS address,
  "Asset1".policy_id AS "policy_id1?",
  "Asset1".asset_name AS "asset_name1?",
  "Asset2".policy_id AS "policy_id2?",
  "Asset2".asset_name AS "asset_name2?",
  "DexMeanPrice".amount1,
  "DexMeanPrice".amount2
FROM "DexMeanPrice"
JOIN "Transaction" ON "Transaction".id = "DexMeanPrice".tx_id
JOIN "Address" ON "Address".id = "DexMeanPrice".address_id
LEFT JOIN "NativeAsset" as "Asset1" ON "Asset1".id = "DexMeanPrice".asset1_id
LEFT JOIN "NativeAsset" as "Asset2" ON "Asset2".id = "DexMeanPrice".asset2_id
WHERE
  "Address".payload = ANY (:addresses)
  AND
  (
    COALESCE("Asset1".policy_id, ''::bytea),
    COALESCE("Asset1".asset_name, ''::bytea),
    COALESCE("Asset2".policy_id, ''::bytea),
    COALESCE("Asset2".asset_name, ''::bytea)
  ) IN (SELECT policy_id1, asset_name1, policy_id2, asset_name2 FROM "AssetPairs")
  AND
  "DexMeanPrice".tx_id <= (:until_tx_id)
  AND
  "DexMeanPrice".tx_id > (:after_tx_id)
ORDER BY "DexMeanPrice".tx_id, "DexMeanPrice".id
LIMIT (:limit);
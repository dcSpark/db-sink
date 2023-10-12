import { Body, Controller, TsoaResponse, Res, Post, Route, SuccessResponse } from 'tsoa';
import { StatusCodes } from 'http-status-codes';
import tx from 'pg-tx';
import pool from '../services/PgPoolSingleton';
import type { ErrorShape } from '../../../shared/errors';
import { genErrorMessage } from '../../../shared/errors';
import { Errors } from '../../../shared/errors';
import type { EndpointTypes } from '../../../shared/routes';
import { Routes } from '../../../shared/routes';
import { getAddressTypes } from '../models/utils';
import { delegationsForPool } from '../services/DelegationForPool';
import { DelegationForPoolResponse } from '../../../shared/models/DelegationForPool';

const route = Routes.delegationForPool;

@Route('delegation/pool')
export class DelegationForPoolController extends Controller {
    @SuccessResponse(`${StatusCodes.OK}`)
    @Post()
    public async delegationForPool(
        @Body()
        requestBody: EndpointTypes[typeof route]['input'],
        @Res()
        errorResponse: TsoaResponse<
            StatusCodes.BAD_REQUEST | StatusCodes.CONFLICT | StatusCodes.UNPROCESSABLE_ENTITY,
            ErrorShape
        >
    ): Promise<EndpointTypes[typeof route]['response']> {
        const response = await tx<
            DelegationForPoolResponse
        >(pool, async dbTx => {
            const data = await delegationsForPool({
                pools: requestBody.pools.map(poolId => Buffer.from(poolId, 'hex')),
                range: requestBody.range,
                dbTx
            });

            return data.map(data => ({
                credential: data.credential as string,
                isDelegation: data.is_delegation as boolean,
                txId: data.tx_id as string,
            }));
        });

        return response;
    }
}
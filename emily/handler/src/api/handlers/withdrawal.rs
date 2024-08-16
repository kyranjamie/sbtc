//! Handlers for withdrawal endpoints.
use serde_dynamo::Item;
use warp::reply::{json, with_status, Reply};

use crate::api::models::common::{BlockHeight, Status};
use crate::api::models::withdrawal::{
    requests::{CreateWithdrawalRequestBody, GetWithdrawalsQuery, UpdateWithdrawalsRequestBody},
    responses::{
        CreateWithdrawalResponse, GetWithdrawalResponse, GetWithdrawalsResponse,
        UpdateWithdrawalsResponse,
    },
    WithdrawalId,
};
use crate::api::models::withdrawal::{Withdrawal, WithdrawalInfo};
use crate::common::error::Error;
use crate::context::EmilyContext;
use crate::database::accessors;
use crate::database::entries::withdrawal::{
    build_withdrawal_update, ValidatedUpdateWithdrawalRequest, WithdrawalEntry, WithdrawalEntryKey,
    WithdrawalEvent, WithdrawalParametersEntry, WithdrawalUpdatePackage,
};
use crate::database::entries::StatusEntry;
use warp::http::StatusCode;

/// Get withdrawal handler.
#[utoipa::path(
    get,
    operation_id = "getWithdrawal",
    path = "/withdrawal/{id}",
    params(
        ("id" = WithdrawalId, Path, description = "id associated with the Withdrawal"),
    ),
    tag = "withdrawal",
    responses(
        // TODO(271): Add success body.
        (status = 200, description = "Withdrawal retrieved successfully", body = GetWithdrawalResponse),
        (status = 400, description = "Invalid request body"),
        (status = 404, description = "Address not found"),
        (status = 405, description = "Method not allowed"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_withdrawal(
    context: EmilyContext,
    request_id: WithdrawalId,
) -> impl warp::reply::Reply {
    // Internal handler so `?` can be used correctly while still returning a reply.
    async fn handler(
        context: EmilyContext,
        request_id: WithdrawalId,
    ) -> Result<impl warp::reply::Reply, Error> {
        // Get withdrawal.
        let withdrawal: Withdrawal = accessors::get_withdrawal_entry(&context, &request_id)
            .await?
            .try_into()?;

        // Respond.
        Ok(with_status(
            json(&(withdrawal as GetWithdrawalResponse)),
            StatusCode::OK,
        ))
    }
    // Handle and respond.
    handler(context, request_id)
        .await
        .map_or_else(Reply::into_response, Reply::into_response)
}

/// Get withdrawals handler.
#[utoipa::path(
    get,
    operation_id = "getWithdrawals",
    path = "/withdrawal",
    params(
        ("nextToken" = String, Query, description = "the next token value from the previous return of this api call."),
        ("pageSize" = String, Query, description = "the maximum number of items in the response list.")
    ),
    tag = "withdrawal",
    responses(
        // TODO(271): Add success body.
        (status = 200, description = "Withdrawals retrieved successfully", body = GetWithdrawalsResponse),
        (status = 400, description = "Invalid request body"),
        (status = 404, description = "Address not found"),
        (status = 405, description = "Method not allowed"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_withdrawals(
    context: EmilyContext,
    query: GetWithdrawalsQuery,
) -> impl warp::reply::Reply {
    // Internal handler so `?` can be used correctly while still returning a reply.
    async fn handler(
        context: EmilyContext,
        query: GetWithdrawalsQuery,
    ) -> Result<impl warp::reply::Reply, Error> {
        // Deserialize next token into the exclusive start key if present.
        let (entries, next_token) = accessors::get_withdrawal_entries(
            &context,
            &query.status,
            query.next_token,
            query.page_size,
        )
        .await?;
        // Convert data into resource types.
        let withdrawals: Vec<WithdrawalInfo> =
            entries.into_iter().map(|entry| entry.into()).collect();
        // Create response.
        let response = GetWithdrawalsResponse { withdrawals, next_token };
        // Respond.
        Ok(with_status(json(&response), StatusCode::OK))
    }
    // Handle and respond.
    handler(context, query)
        .await
        .map_or_else(Reply::into_response, Reply::into_response)
}

/// Create withdrawal handler.
#[utoipa::path(
    post,
    operation_id = "createWithdrawal",
    path = "/withdrawal",
    tag = "withdrawal",
    request_body = CreateWithdrawalRequestBody,
    responses(
        // TODO(271): Add success body.
        (status = 201, description = "Withdrawal created successfully", body = CreateWithdrawalResponse),
        (status = 400, description = "Invalid request body"),
        (status = 404, description = "Address not found"),
        (status = 405, description = "Method not allowed"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_withdrawal(
    context: EmilyContext,
    body: CreateWithdrawalRequestBody,
) -> impl warp::reply::Reply {
    // Internal handler so `?` can be used correctly while still returning a reply.
    async fn handler(
        context: EmilyContext,
        body: CreateWithdrawalRequestBody,
    ) -> Result<impl warp::reply::Reply, Error> {
        // Set variables.
        // TODO(396): Remove dummy hash; take hash from request.

        let CreateWithdrawalRequestBody {
            request_id,
            stacks_block_hash,
            recipient,
            amount,
            parameters,
        } = body;

        let stacks_block_height: BlockHeight = 0;
        let status = Status::Pending;

        // Make table entry.
        let withdrawal_entry: WithdrawalEntry = WithdrawalEntry {
            key: WithdrawalEntryKey {
                request_id,
                // TODO(396): Remove dummy hash.
                stacks_block_hash: stacks_block_hash.clone(),
            },
            recipient,
            amount,
            parameters: WithdrawalParametersEntry { max_fee: parameters.max_fee },
            history: vec![WithdrawalEvent {
                status: StatusEntry::Pending,
                message: "Just received withdrawal".to_string(),
                stacks_block_hash: stacks_block_hash.clone(),
                stacks_block_height,
            }],
            status,
            last_update_block_hash: stacks_block_hash,
            last_update_height: stacks_block_height,
            ..Default::default()
        };
        // Validate withdrawal entry.
        withdrawal_entry.validate()?;
        // Add entry to the table.
        accessors::add_withdrawal_entry(&context, &withdrawal_entry).await?;
        // Respond.
        let response: CreateWithdrawalResponse = withdrawal_entry.try_into()?;
        Ok(with_status(json(&response), StatusCode::CREATED))
    }
    // Handle and respond.
    handler(context, body)
        .await
        .map_or_else(Reply::into_response, Reply::into_response)
}

/// Update withdrawals handler.
#[utoipa::path(
    put,
    operation_id = "updateWithdrawals",
    path = "/withdrawal",
    tag = "withdrawal",
    request_body = UpdateWithdrawalsRequestBody,
    responses(
        (status = 201, description = "Withdrawals updated successfully", body = UpdateWithdrawalsResponse),
        (status = 400, description = "Invalid request body"),
        (status = 404, description = "Address not found"),
        (status = 405, description = "Method not allowed"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_withdrawals(
    context: EmilyContext,
    body: UpdateWithdrawalsRequestBody,
) -> impl warp::reply::Reply {
    // Internal handler so `?` can be used correctly while still returning a reply.
    async fn handler(
        context: EmilyContext,
        body: UpdateWithdrawalsRequestBody,
    ) -> Result<impl warp::reply::Reply, Error> {
        // Validate request.
        let validated_request: ValidatedUpdateWithdrawalRequest = body.try_into()?;
        // Create aggregator.
        let mut updated_withdrawals: Vec<Withdrawal> =
            Vec::with_capacity(validated_request.withdrawals.len());
        // Loop through all updates and execute.
        for update in validated_request.withdrawals {
            // Get original withdrawal entry.
            let withdrawal_entry =
                accessors::get_withdrawal_entry(&context, &update.request_id).await?;
            // Make the update package.
            let update_package = WithdrawalUpdatePackage::from(&withdrawal_entry, update)?;
            let update_output = build_withdrawal_update(&context, &update_package)?
                .send()
                .await?;
            // Get the updated withdrawal.
            let updated_withdrawal: Withdrawal = update_output
                .attributes
                .ok_or(Error::InternalServer)
                .and_then(|fields| {
                    serde_dynamo::from_item::<Item, WithdrawalEntry>(fields.into())
                        .map_err(Error::from)
                })
                .and_then(|entry| entry.try_into())?;
            // Append the updated withdrawal to the list.
            updated_withdrawals.push(updated_withdrawal);
        }
        let response = UpdateWithdrawalsResponse {
            withdrawals: updated_withdrawals,
        };
        Ok(with_status(json(&response), StatusCode::CREATED))
    }
    // Handle and respond.
    handler(context, body)
        .await
        .map_or_else(Reply::into_response, Reply::into_response)
}

// TODO(393): Add handler unit tests.

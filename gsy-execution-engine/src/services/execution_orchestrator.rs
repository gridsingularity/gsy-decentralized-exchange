use ::primitives::utils::timestamp_to_datetime_string;
use anyhow::Result;
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::db_api_schema::trades::TradeStatus;
use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
use tracing::info;

use crate::{
    connectors::evm_connector::complete_delivery_verification,
    primitives::penalty_calculator::compute_delivery_verifications,
};

/// The window of trade creation and measurement times checked for `timeslot`.
fn timeslot_window(timeslot: u64, market_duration: u64) -> (u64, u64) {
    let start_time = (timeslot / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;
    let end_time = start_time
        + (market_duration
            .checked_sub(1)
            .unwrap_or(GLOBAL_CONSTANTS.time_slot_sec));
    (start_time, end_time)
}

/// Higher-level function that does the repeated/polling logic
/// 1) fetch trades/measurements
/// 2) calculate verdicts for trades with available measurements
/// 3) atomically record penalties and verification on-chain
pub async fn run_execution_cycle(
    offchain_url: &str,
    evm_node_url: &str,
    trade_settlement_address: &str,
    execution_engine_private_key: &str,
    timeslot: u64,
    penalty_rate: f64,
    market_duration: u64,
) -> Result<usize> {
    let offchain_storage_client = OffchainStorageClient::new(
        OffchainStorageTransport::from_env(),
        offchain_url,
        "EWDS_EXECUTION_ENGINE_CLIENT_ID",
        "gsyexecutionengine",
    );

    // 1) fetch trades/measurements
    let (start_time, end_time) = timeslot_window(timeslot, market_duration);
    let (mut trades, measurements) = tokio::try_join!(
        offchain_storage_client.fetch_trades(None, Some(start_time), Some(end_time)),
        offchain_storage_client.fetch_measurements(start_time, end_time),
    )?;
    // A separately recorded penalty does not imply delivery verification is complete.
    // The contract check below skips trades already verified without resubmitting penalties.
    trades.retain(|trade| matches!(trade.status, TradeStatus::Matched | TradeStatus::Penalized));
    info!(
        "Fetched {} trades, {} measurements for timeslot {}.",
        trades.len(),
        measurements.len(),
        timestamp_to_datetime_string(timeslot),
    );

    if trades.is_empty() || measurements.is_empty() {
        info!(
            "No trades or measurements for timeslot {}. Skipping execution cycle.",
            timestamp_to_datetime_string(timeslot),
        );
        return Ok(0);
    }

    // 1.2) fetch facility_id>owner_id mapping
    let facility_owner_mapping = offchain_storage_client
        .fetch_facility_owner_mapping()
        .await?;

    anyhow::ensure!(
        penalty_rate.is_finite() && penalty_rate >= 0.0,
        "Invalid penalty rate"
    );
    // A measured zero-penalty trade is complete; an unmeasured one remains pending.
    let verifications = compute_delivery_verifications(
        &trades,
        &measurements,
        &facility_owner_mapping,
        penalty_rate,
    );
    let all_measured = verifications.len() == trades.len();
    info!("Computed {} delivery verifications", verifications.len());

    let processed = complete_delivery_verification(
        evm_node_url,
        trade_settlement_address,
        execution_engine_private_key,
        verifications,
    )
    .await?;
    // Do not release a retained slot just because a subset was verified.
    Ok(if all_measured { processed } else { 0 })
}

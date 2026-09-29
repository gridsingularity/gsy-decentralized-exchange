use ::primitives::utils::timestamp_to_datetime_string;
use anyhow::Result;
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
use tracing::info;

use crate::{
    connectors::evm_connector::submit_penalties,
    primitives::penalty_calculator::{compute_penalties, Penalty},
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
/// 2) compute penalties
/// 3) submit them
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
    let (trades, measurements) = tokio::try_join!(
        offchain_storage_client.fetch_trades(start_time, end_time),
        offchain_storage_client.fetch_measurements(start_time, end_time),
    )?;
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
    let facility_owner_mapping = offchain_storage_client.fetch_facility_owner_mapping().await?;

    // 2) compute penalties
    let penalties: Vec<Penalty> = compute_penalties(
        &trades,
        &measurements,
        &facility_owner_mapping,
        penalty_rate,
    );
    info!("Computed {} penalties", penalties.len());

    // 3) submit penalties
    let processed_penalties = submit_penalties(
        evm_node_url,
        trade_settlement_address,
        execution_engine_private_key,
        penalties,
    )
    .await?;
    Ok(processed_penalties)
}

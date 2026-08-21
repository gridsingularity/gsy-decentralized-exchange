use ::primitives::utils::timestamp_to_datetime_string;
use anyhow::Result;
use tracing::info;

use crate::{
    connectors::{
        evm_connector::submit_penalties,
        offchain_storage::{
            fetch_trades_and_measurements_for_timeslot,
            fetch_facility_owner_mapping
        },
    },
    primitives::penalty_calculator::{compute_penalties, Penalty},
};

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
) -> Result<()> {
    // 1.1) fetch trades/measurements
    let (trades, measurements) =
        fetch_trades_and_measurements_for_timeslot(offchain_url, timeslot, market_duration).await?;
    info!(
        "Fetched {} trades, {} measurements for timeslot {}.",
        trades.len(),
        measurements.len(),
        timestamp_to_datetime_string(timeslot),
    );
    // 1.2) fetch facility_id>owner_id mapping
    let facility_owner_mapping = fetch_facility_owner_mapping(offchain_url).await?;

    // 2) compute penalties
    let penalties: Vec<Penalty> = compute_penalties(
        &trades,
        &measurements,
        &facility_owner_mapping,
        penalty_rate);
    info!("Computed {} penalties", penalties.len());

    // 3) submit penalties
    submit_penalties(
        evm_node_url,
        trade_settlement_address,
        execution_engine_private_key,
        penalties,
    )
    .await?;
    Ok(())
}

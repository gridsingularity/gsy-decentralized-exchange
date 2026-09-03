use anyhow::Result;
use tracing::info;
use gsy_offchain_primitives::utils::timestamp_to_datetime_string;

use crate::{
    primitives::{
        penalty_calculator::{compute_penalties, evaluated_trade_uuids, Penalty},
    },
    connectors::{
        offchain_storage::fetch_trades_and_measurements_for_timeslot,
        substrate_connector::submit_penalties,
    },
};

/// Higher-level function that does the repeated/polling logic
/// 1) fetch trades/measurements
/// 2) compute penalties and the evaluated trade set
/// 3) submit both
pub async fn run_execution_cycle(
    offchain_url: &str,
    node_url: &str,
    timeslot: u64,
    penalty_rate: f64,
    market_duration: u64,
) -> Result<()> {
    // 1) fetch trades/measurements
    let (trades, measurements) = fetch_trades_and_measurements_for_timeslot(offchain_url, timeslot, market_duration).await?;
    info!(
        "Fetched {} trades, {} measurements for timeslot {}.",
        trades.len(),
        measurements.len(),
        timestamp_to_datetime_string(timeslot),
    );

    // 2) compute penalties and the evaluated trade set
    let penalties: Vec<Penalty> = compute_penalties(&trades, &measurements, penalty_rate);
    let evaluated = evaluated_trade_uuids(&trades, &measurements);
    info!(
        "Computed {} penalties, {} evaluated trades",
        penalties.len(),
        evaluated.len()
    );

    // 3) submit penalties and evaluated trades
    submit_penalties(node_url, penalties, evaluated).await?;
    Ok(())
}

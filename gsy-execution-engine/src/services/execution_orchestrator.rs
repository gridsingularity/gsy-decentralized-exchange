use anyhow::Result;
use tracing::info;

use crate::{
    primitives::{
        penalty_calculator::{compute_penalties, Penalty},
        trades::Trade,
        measurements::Measurement,
    },
    connectors::{
        offchain_storage::fetch_trades_and_measurements_for_timeslot,
        substrate_connector::submit_penalties,
    },
};

/// Higher-level function that does the repeated/polling logic
/// 1) fetch trades/measurements
/// 2) compute penalties
/// 3) submit them
pub async fn run_execution_cycle(
    offchain_url: &str,
    node_url: &str,
    timeslot_str: &str,
) -> Result<()> {
    // 1) fetch trades/measurements
    let (trades, measurements) = fetch_trades_and_measurements_for_timeslot(offchain_url, timeslot_str).await?;
    info!(
        "Fetched {} trades, {} measurements for timeslot {}",
        trades.len(),
        measurements.len(),
        timeslot_str
    );

    // 2) compute penalties
    let penalties: Vec<Penalty> = compute_penalties(&trades, &measurements);
    info!("Computed {} penalties", penalties.len());

    // 3) submit penalties
    submit_penalties(node_url, &penalties).await?;
    Ok(())
}

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use gsy_offchain_primitives::utils::timestamp_to_datetime_string;

use crate::{
    primitives::{
        penalty_calculator::{compute_penalties, evaluated_trade_uuids, Penalty, PenaltyReason},
    },
    connectors::{
        offchain_storage::fetch_trades_and_measurements_for_timeslot,
        substrate_connector::submit_penalties,
    },
};

/// Keeps only the penalties and evaluated uuids of trades whose status is still `Settled`, so
/// the first verdict on a trade is final. Penalties must be computed over every fetched trade
/// beforehand, because already-judged trades still consume the waterfall budget.
pub fn retain_settled(
    trades: &[TradeSchema],
    penalties: Vec<Penalty>,
    evaluated: Vec<String>,
) -> (Vec<Penalty>, Vec<String>) {
    let settled: HashSet<&str> = trades
        .iter()
        .filter(|trade| trade.status == TradeStatus::Settled)
        .map(|trade| trade.trade_uuid.as_str())
        .collect();
    let penalties = penalties
        .into_iter()
        .filter(|penalty| settled.contains(penalty.trade_uuid.as_str()))
        .collect();
    let evaluated = evaluated
        .into_iter()
        .filter(|uuid| settled.contains(uuid.as_str()))
        .collect();
    (penalties, evaluated)
}

/// Per missing-measurement source, in first-seen order: the number of penalized sides and their
/// energy (a side missing its measurement is penalized on the trade's full energy).
fn missing_measurement_summary<'a>(
    trades: &[TradeSchema],
    penalties: &'a [Penalty],
) -> Vec<(&'a str, usize, f64)> {
    let energy_of: HashMap<&str, f64> = trades
        .iter()
        .map(|trade| (trade.trade_uuid.as_str(), trade.parameters.selected_energy))
        .collect();
    let mut summary: Vec<(&str, usize, f64)> = Vec::new();
    for penalty in penalties {
        let PenaltyReason::MissingMeasurement { source } = &penalty.reason else {
            continue;
        };
        let energy = energy_of.get(penalty.trade_uuid.as_str()).copied().unwrap_or(0.0);
        match summary.iter_mut().find(|(known, _, _)| *known == source.as_str()) {
            Some(entry) => {
                entry.1 += 1;
                entry.2 += energy;
            }
            None => summary.push((source.as_str(), 1, energy)),
        }
    }
    summary
}

/// Higher-level function that does the repeated/polling logic
/// 1) fetch trades/measurements
/// 2) compute penalties and the evaluated trade set, keeping only not-yet-judged trades
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

    // 2) compute penalties and the evaluated trade set over every fetched trade, then keep
    // only the trades that have not been judged yet
    let penalties: Vec<Penalty> = compute_penalties(&trades, &measurements, penalty_rate);
    let evaluated = evaluated_trade_uuids(&trades, &measurements);
    let evaluated_set: HashSet<&str> = evaluated.iter().map(String::as_str).collect();
    let unjudged: HashSet<&str> = trades
        .iter()
        .map(|trade| trade.trade_uuid.as_str())
        .filter(|uuid| !evaluated_set.contains(uuid))
        .collect();
    let unjudged_count = unjudged.len();
    let (penalties, evaluated) = retain_settled(&trades, penalties, evaluated);
    for (source, sides, energy_kwh) in missing_measurement_summary(&trades, &penalties) {
        warn!(
            "Missing or incomplete measurement for {}: penalized {} trade side(s) on their \
             full {:.3} kWh",
            source, sides, energy_kwh
        );
    }
    info!(
        "{} fetched trades have no measurement on either side and stay unjudged",
        unjudged_count
    );
    info!(
        "Computed {} penalties, {} newly evaluated trades",
        penalties.len(),
        evaluated.len()
    );

    // 3) submit penalties and evaluated trades
    submit_penalties(node_url, penalties, evaluated).await?;
    Ok(())
}

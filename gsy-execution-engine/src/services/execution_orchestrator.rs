use ::primitives::utils::{derive_onchain_id, timestamp_to_datetime_string};
use anyhow::{ensure, Context, Result};
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::db_api_schema::trades::TradeStatus;
use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
use std::collections::{BTreeMap, HashMap};
use tracing::info;

use crate::{
    connectors::evm_connector::complete_delivery_verification,
    primitives::penalty_calculator::compute_delivery_verifications,
};

/// Half-open delivery window, independent of trade creation/settlement times.
fn timeslot_window(timeslot: u64, market_duration: u64) -> Result<(u64, u64)> {
    ensure!(
        GLOBAL_CONSTANTS.time_slot_sec > 0,
        "TIME_SLOT_SEC must be positive"
    );
    let start_time = (timeslot / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;
    let duration = if market_duration == 0 {
        GLOBAL_CONSTANTS.time_slot_sec
    } else {
        market_duration
    };
    let end_time = start_time
        .checked_add(duration)
        .context("Delivery window overflow")?;
    Ok((start_time, end_time))
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
    penalty_tolerance_percent: f64,
) -> Result<usize> {
    anyhow::ensure!(
        penalty_tolerance_percent.is_finite() && (0.0..=100.0).contains(&penalty_tolerance_percent),
        "Invalid penalty tolerance percentage"
    );
    ensure!(
        penalty_rate.is_finite() && penalty_rate >= 0.0,
        "Invalid penalty rate"
    );
    let offchain_storage_client = OffchainStorageClient::new(
        OffchainStorageTransport::from_env(),
        offchain_url,
        "EWDS_EXECUTION_ENGINE_CLIENT_ID",
        "gsyexecutionengine",
    );

    // 1) fetch trades/measurements
    let (start_time, end_time) = timeslot_window(timeslot, market_duration)?;
    let (trades, measurements) = tokio::try_join!(
        offchain_storage_client.fetch_trades(None, Some(start_time), Some(end_time)),
        // Measurements use an inclusive end; trade queries use an exclusive end.
        offchain_storage_client.fetch_measurements(start_time, end_time - 1),
    )?;
    // Completed trades still contribute to facility totals and allocation weights.
    // Penalized trades may not be verified yet; the connector checks on-chain.
    let pending_count = trades
        .iter()
        .filter(|trade| matches!(trade.status, TradeStatus::Matched | TradeStatus::Penalized))
        .count();
    info!(
        "Fetched {} trades, {} measurements for timeslot {}.",
        trades.len(),
        measurements.len(),
        timestamp_to_datetime_string(timeslot),
    );

    if pending_count == 0 || measurements.is_empty() {
        info!(
            "No pending trades or measurements for timeslot {}. Skipping execution cycle.",
            timestamp_to_datetime_string(timeslot),
        );
        return Ok(0);
    }

    // Derive the same IDs as the ID service without creating mappings during verification.
    let mut facility_onchain_ids = HashMap::new();
    for measurement in measurements
        .iter()
        .filter(|measurement| measurement.energy_kwh.is_finite())
    {
        facility_onchain_ids
            .entry(measurement.facility_id.clone())
            .or_insert_with(|| derive_onchain_id(&measurement.facility_id));
    }

    // Resolve each market once, then isolate slots even when the query window
    // spans several slots. Creation times must never select a meter reading.
    let mut market_slots = HashMap::new();
    let mut trades_by_slot: BTreeMap<u64, Vec<_>> = BTreeMap::new();
    for trade in trades {
        let slot = if let Some(slot) = market_slots.get(&trade.market_id) {
            *slot
        } else {
            let market = offchain_storage_client
                .fetch_market(&trade.market_id)
                .await?
                .with_context(|| {
                    format!(
                        "Missing market {} for delivery verification",
                        trade.market_id
                    )
                })?;
            ensure!(
                market.market_id == trade.market_id,
                "Market lookup returned a different market"
            );
            let slot = market.delivery_start_time.parse::<u64>().with_context(|| {
                format!("Invalid delivery start for market {}", trade.market_id)
            })?;
            ensure!(
                (start_time..end_time).contains(&slot),
                "Market delivery slot outside requested window"
            );
            market_slots.insert(trade.market_id.clone(), slot);
            slot
        };
        trades_by_slot.entry(slot).or_default().push(trade);
    }
    let mut verifications = Vec::new();
    for (slot, trades) in trades_by_slot {
        verifications.extend(compute_delivery_verifications(
            &trades,
            &measurements,
            &facility_onchain_ids,
            slot,
            penalty_rate,
            penalty_tolerance_percent,
        )?);
    }
    let all_measured = verifications.len() == pending_count;
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

use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use primitives::db_api_schema::{
    profiles::MeasurementSchema,
    trades::{DbTradeSchema, TradeStatus},
};
use primitives::utils::NODE_FLOAT_SCALING_FACTOR;
use std::collections::{HashMap, HashSet};
use tracing::warn;

#[derive(Debug)]
pub struct Penalty {
    pub penalized_account: String,
    pub market_id: String,
    pub trade_uuid: String,
    pub penalty_cost: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryVerification {
    pub trade_uuid: String,
    pub buyer_penalty_cost: u64,
    pub seller_penalty_cost: u64,
}

/// Allocates each facility's delivery-slot penalty across its trades.
///
/// Measurements are mapped from facility IDs to the trade's buyer/seller owner IDs.
/// Compare signed measurements against purchases minus sales, across all indexed
/// trades in this slot (including completed trades). Allocate by absolute traded
/// quantity. Both parties need a usable reading before their trade can complete,
/// until the slot's grace period is over (see `compute_final_delivery_verifications`).
///
/// # Arguments
///
/// * `trades` - A slice of DbTradeSchema records.
/// * `measurements` - A slice of MeasurementSchema records.
/// * `delivery_slot` - All supplied trades must belong to this delivery slot.
/// * `penalty_rate` - The penalty rate as a f64 (e.g., 0.10 for 10%).
/// * `penalty_tolerance_percent` - Allowed deviation, inclusive (e.g., 5.0 for 5%).
///   Beyond this band, the penalty rate applies to the full deviation.
///
/// # Returns
///
/// A vector of Penalty structs.
pub fn compute_penalties(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    facility_owner_mapping: &HashMap<String, String>,
    delivery_slot: DateTime<Utc>,
    penalty_rate: f64,
    penalty_tolerance_percent: f64,
) -> Result<Vec<Penalty>> {
    let verifications = compute_delivery_verifications(
        trades,
        measurements,
        facility_owner_mapping,
        delivery_slot,
        penalty_rate,
        penalty_tolerance_percent,
    )?;
    let trades_by_id: HashMap<_, _> = trades
        .iter()
        .map(|trade| (trade.trade_uuid.as_str(), trade))
        .collect();
    Ok(verifications
        .into_iter()
        .flat_map(|verification| {
            let trade = trades_by_id[verification.trade_uuid.as_str()];
            [
                (trade.buyer.clone(), verification.buyer_penalty_cost),
                (trade.seller.clone(), verification.seller_penalty_cost),
            ]
            .into_iter()
            .filter(|(_, cost)| *cost > 0)
            .map(|(account, cost)| Penalty {
                penalized_account: account,
                market_id: trade.market_id.clone(),
                trade_uuid: trade.trade_uuid.clone(),
                penalty_cost: cost,
            })
            .collect::<Vec<_>>()
        })
        .collect())
}

#[derive(Default)]
struct FacilityTrades<'a> {
    bought: u128,
    sold: u128,
    quantities: Vec<(&'a str, u64)>,
}

/// Calculate one slot only; the caller resolves each trade's slot from its market.
/// Completed trades remain in the baseline, but receive no new verdict. Missing,
/// conflicting or non-finite readings on either side leave a trade pending.
pub fn compute_delivery_verifications(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    facility_owner_mapping: &HashMap<String, String>,
    delivery_slot: DateTime<Utc>,
    penalty_rate: f64,
    penalty_tolerance_percent: f64,
) -> Result<Vec<DeliveryVerification>> {
    delivery_verifications(
        trades,
        measurements,
        facility_owner_mapping,
        delivery_slot,
        penalty_rate,
        penalty_tolerance_percent,
        false,
    )
}

/// Final calculation for a slot whose grace period is over: every pending trade
/// receives a verdict. A party still without a usable reading (missing, conflicting
/// or non-finite) is treated as having delivered nothing, so it is penalized on its
/// whole net traded position, and its counterparties are charged from their own readings.
pub fn compute_final_delivery_verifications(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    facility_owner_mapping: &HashMap<String, String>,
    delivery_slot: DateTime<Utc>,
    penalty_rate: f64,
    penalty_tolerance_percent: f64,
) -> Result<Vec<DeliveryVerification>> {
    delivery_verifications(
        trades,
        measurements,
        facility_owner_mapping,
        delivery_slot,
        penalty_rate,
        penalty_tolerance_percent,
        true,
    )
}

fn delivery_verifications(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    facility_owner_mapping: &HashMap<String, String>,
    delivery_slot: DateTime<Utc>,
    penalty_rate: f64,
    penalty_tolerance_percent: f64,
    grace_period_over: bool,
) -> Result<Vec<DeliveryVerification>> {
    ensure!(
        penalty_rate.is_finite() && penalty_rate >= 0.0,
        "Invalid penalty rate"
    );
    ensure!(
        penalty_tolerance_percent.is_finite() && (0.0..=100.0).contains(&penalty_tolerance_percent),
        "Invalid penalty tolerance percentage"
    );
    let mut facilities: HashMap<&str, FacilityTrades<'_>> = HashMap::new();
    let mut trade_ids = HashSet::new();
    for trade in trades {
        ensure!(
            trade_ids.insert(&trade.trade_uuid),
            "Duplicate trade {}",
            trade.trade_uuid
        );
        ensure!(
            trade.buyer != trade.seller,
            "Self-trade {}",
            trade.trade_uuid
        );
        let energy = (trade.parameters.selected_energy_kWh * NODE_FLOAT_SCALING_FACTOR).round();
        ensure!(
            energy.is_finite() && energy >= 1.0 && energy < u64::MAX as f64,
            "Invalid traded energy for {}",
            trade.trade_uuid
        );
        let energy = energy as u64;
        let buyer = facilities.entry(&trade.buyer).or_default();
        buyer.bought += u128::from(energy);
        buyer.quantities.push((&trade.trade_uuid, energy));
        let seller = facilities.entry(&trade.seller).or_default();
        seller.sold += u128::from(energy);
        seller.quantities.push((&trade.trade_uuid, energy));
    }

    // This deployment guarantees one facility per owner; trade parties identify owners.
    let mut measurement_map: HashMap<String, Option<i128>> = HashMap::new();
    for meas in measurements
        .iter()
        .filter(|meas| meas.time_slot == delivery_slot)
    {
        let Some(owner_id) = facility_owner_mapping.get(&meas.facility_id) else {
            warn!(
                "No owner mapping supplied for facility_id={}",
                meas.facility_id
            );
            continue;
        };
        if !facilities.contains_key(owner_id.as_str()) {
            continue;
        }
        let energy = if meas.energy_kwh.is_finite() {
            let units = (meas.energy_kwh * NODE_FLOAT_SCALING_FACTOR).round();
            ensure!(units.abs() < u64::MAX as f64, "Measured energy exceeds on-chain range");
            Some(units as i128)
        } else {
            None
        };
        measurement_map
            .entry(owner_id.clone())
            .and_modify(|previous| {
                if *previous != energy {
                    *previous = None;
                }
            })
            .or_insert(energy);
    }

    let mut allocations = HashMap::new();
    for (facility, totals) in facilities {
        let reading = measurement_map.get(facility).copied();
        if reading == Some(None) {
            warn!(
                "Invalid or conflicting measurements for owner {} in slot {}",
                facility, delivery_slot
            );
        }
        let measured = match reading.flatten() {
            Some(measured) => measured,
            None if grace_period_over => {
                warn!(
                    "No usable measurement for owner {} in slot {} after its grace period; \
                     penalizing its whole net traded position",
                    facility, delivery_slot
                );
                0
            }
            None => continue,
        };
        let net = i128::try_from(totals.bought)? - i128::try_from(totals.sold)?;
        let cost = penalty_cost(net, measured, penalty_rate, penalty_tolerance_percent)?;
        for (trade_id, cost) in allocate_penalty(cost, &totals.quantities) {
            allocations.insert((facility, trade_id), cost);
        }
    }

    let mut verifications = Vec::new();
    // Completion is atomic for both parties, so neither side can be a fallback.
    for trade in trades
        .iter()
        .filter(|trade| matches!(trade.status, TradeStatus::Matched | TradeStatus::Penalized))
    {
        let (Some(&buyer_penalty_cost), Some(&seller_penalty_cost)) = (
            allocations.get(&(trade.buyer.as_str(), trade.trade_uuid.as_str())),
            allocations.get(&(trade.seller.as_str(), trade.trade_uuid.as_str())),
        ) else {
            continue;
        };
        verifications.push(DeliveryVerification {
            trade_uuid: trade.trade_uuid.clone(),
            buyer_penalty_cost,
            seller_penalty_cost,
        });
    }

    Ok(verifications)
}

/// Largest-remainder allocation in on-chain units preserves the total exactly.
/// Trade ID breaks remainder ties so query order cannot change a retry's amounts.
fn allocate_penalty<'a>(cost: u64, quantities: &[(&'a str, u64)]) -> Vec<(&'a str, u64)> {
    let total: u128 = quantities
        .iter()
        .map(|(_, energy)| u128::from(*energy))
        .sum();
    let mut shares = quantities
        .iter()
        .map(|(id, energy)| {
            let weighted = u128::from(cost) * u128::from(*energy);
            (*id, (weighted / total) as u64, weighted % total)
        })
        .collect::<Vec<_>>();
    let remainder = cost - shares.iter().map(|(_, share, _)| share).sum::<u64>();
    shares.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));
    for (_, share, _) in shares.iter_mut().take(remainder as usize) {
        *share += 1;
    }
    shares
        .into_iter()
        .map(|(id, share, _)| (id, share))
        .collect()
}

fn penalty_cost(
    traded_energy: i128,
    measured_energy: i128,
    rate: f64,
    tolerance_percent: f64,
) -> Result<u64> {
    let deviation = (measured_energy - traded_energy).unsigned_abs();
    let tolerance_percent = (tolerance_percent * NODE_FLOAT_SCALING_FACTOR).round() as u128;
    let within_band = deviation
        .checked_mul(100 * NODE_FLOAT_SCALING_FACTOR as u128)
        .zip(traded_energy.unsigned_abs().checked_mul(tolerance_percent))
        .map(|(deviation, band)| deviation <= band);
    let Some(within_band) = within_band else {
        anyhow::bail!("Energy exceeds the range of the tolerance comparison");
    };
    if within_band {
        Ok(0)
    } else {
        let cost = (deviation as f64 * rate).round();
        ensure!(
            cost.is_finite() && cost < u64::MAX as f64,
            "Penalty exceeds on-chain range"
        );
        Ok(cost as u64)
    }
}

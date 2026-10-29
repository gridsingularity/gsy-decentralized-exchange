use primitives::db_api_schema::{profiles::MeasurementSchema, trades::DbTradeSchema};
use std::collections::HashMap;
use tracing::debug;

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

/// Computes penalties for each trade based on the measured energy.
///
/// Measurements are mapped from facility IDs to the trade's buyer/seller IDs.
/// The delta is computed as:
///   delta = measured_energy - traded_energy
/// If delta > 0.0, it indicates under-trading for consumption and the buyer is penalized.
/// If delta < 0.0, it indicates under-trading for production and the seller is penalized.
///
/// # Arguments
///
/// * `trades` - A slice of DbTradeSchema records.
/// * `measurements` - A slice of MeasurementSchema records.
/// * `penalty_rate` - The penalty rate as a f64 (e.g., 0.10 for 10%).
///
/// # Returns
///
/// A vector of Penalty structs.
pub fn compute_penalties(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    facility_onchain_ids: &HashMap<String, String>,
    penalty_rate: f64,
) -> Vec<Penalty> {
    let verifications =
        compute_delivery_verifications(trades, measurements, facility_onchain_ids, penalty_rate);
    let trades_by_id: HashMap<_, _> = trades
        .iter()
        .map(|trade| (trade.trade_uuid.as_str(), trade))
        .collect();
    verifications
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
        .collect()
}

/// A missing measurement is not a successful zero-penalty verification.
/// Retains the existing buyer-first/seller-fallback policy pending a separate policy review.
pub fn compute_delivery_verifications(
    trades: &[DbTradeSchema],
    measurements: &[MeasurementSchema],
    facility_onchain_ids: &HashMap<String, String>,
    penalty_rate: f64,
) -> Vec<DeliveryVerification> {
    let mut verifications = Vec::new();

    // Use the same facility ID mapping as the submitting community client.
    let mut measurement_map: HashMap<String, f64> = HashMap::new();
    for meas in measurements {
        if !meas.energy_kwh.is_finite() {
            continue;
        }
        let Some(onchain_id) = facility_onchain_ids.get(&meas.facility_id) else {
            debug!("No derived identity supplied for facility_id={}", meas.facility_id);
            continue;
        };
        measurement_map.insert(onchain_id.clone(), meas.energy_kwh);
    }
    // Iterate over each trade and compute the penalty if a measurement exists.
    for trade in trades {
        if let Some(&measured_energy) = measurement_map
            .get(&trade.buyer)
            .or_else(|| measurement_map.get(&trade.seller))
        {
            let traded_energy = trade.parameters.selected_energy_kWh;

            // Compute delta = measured_energy - traded_energy.
            let delta = measured_energy - traded_energy;

            let penalty_cost = (delta.abs() * penalty_rate * 10_000.0).round() as u64;
            verifications.push(DeliveryVerification {
                trade_uuid: trade.trade_uuid.clone(),
                buyer_penalty_cost: if delta > 0.0 { penalty_cost } else { 0 },
                seller_penalty_cost: if delta < 0.0 { penalty_cost } else { 0 },
            });
        }
    }

    verifications
}

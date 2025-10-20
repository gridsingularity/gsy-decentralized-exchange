use gsy_offchain_primitives::db_api_schema::{profiles::MeasurementSchema, trades::TradeSchema};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Penalty {
	pub penalized_account: String,
	pub market_id: String,
	pub trade_uuid: String,
	pub penalty_cost: u64,
}

/// Computes penalties for each trade based on the measured energy.
///
/// For each trade, the measurement is looked up (by using the area_uuid and market_id from the Bid).
/// The delta is computed as:
///   delta = measured_energy - traded_energy
/// If delta > 0.0, it indicates under-trading for consumption and the buyer is penalized.
/// If delta < 0.0, it indicates under-trading for production and the seller is penalized.
///
/// # Arguments
///
/// * `trades` - A slice of TradeSchema records.
/// * `measurements` - A slice of MeasurementSchema records.
/// * `penalty_rate` - The penalty rate as a f64 (e.g., 0.10 for 10%).
///
/// # Returns
///
/// A vector of Penalty structs.
pub fn compute_penalties(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
	penalty_rate: f64,
) -> Vec<Penalty> {
	let mut penalties = Vec::new();

	// Create a lookup map for measurements by (area_uuid, market_id)
	let mut measurement_map: HashMap<(String, String), f64> = HashMap::new();
	for meas in measurements {
		measurement_map.insert(
			(meas.area_uuid.clone(), meas.community_uuid.clone()),
			meas.energy_kwh, // energy is f64; positive means consumption, negative means production
		);
	}

	// Iterate over each trade and compute the penalty if a measurement exists.
	for trade in trades {
		// For consumers, we use the Bid's area and market.
		let key =
			(trade.bid.bid_component.area_uuid.clone(), trade.bid.bid_component.market_id.clone());

		if let Some(&measured_energy) = measurement_map.get(&key) {
			let traded_energy = trade.parameters.selected_energy;

			// Compute delta = measured_energy - traded_energy.
			let delta = measured_energy - traded_energy;

			if delta > 0.0 {
				// This is a consumption trade: measured energy exceeds traded energy.
				// Penalize the buyer.

				let raw_penalty = delta * penalty_rate;

				// Scale and convert to u64: apply a scaling factor of 10,000.
				let penalty_cost = (raw_penalty * 10_000.0).round() as u64;

				penalties.push(Penalty {
					penalized_account: trade.buyer.clone(),
					market_id: trade.market_id.clone(),
					trade_uuid: trade.trade_uuid.clone(),
					penalty_cost,
				});
			} else if delta < 0.0 {
				// This is a production trade: measured energy is less than traded energy.
				// Penalize the seller.
				let raw_penalty = (-delta) * penalty_rate;
				let penalty_cost = (raw_penalty * 10_000.0).round() as u64;

				penalties.push(Penalty {
					penalized_account: trade.seller.clone(),
					market_id: trade.market_id.clone(),
					trade_uuid: trade.trade_uuid.clone(),
					penalty_cost,
				});
			}
		}
	}

	penalties
}

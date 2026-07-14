use gsy_offchain_primitives::aggregation::aggregate_net_import;
use gsy_offchain_primitives::db_api_schema::{profiles::MeasurementSchema, trades::TradeSchema};
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct Penalty {
	pub penalized_account: String,
	pub market_id: String,
	pub trade_uuid: String,
	pub penalty_cost: u64,
}

/// Computes penalties for each trade based on the measured energy.
pub fn compute_penalties(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
	penalty_rate: f64,
) -> Vec<Penalty> {
	let mut penalties = Vec::new();

	// Create a lookup map for measurements by area_uuid
	// TODO: temporarily use only the area_hash for identifying measurements. Should be improved
	// by adding market_id in the measurements, and use this too for identification.
	let mut measurement_map: HashMap<String, f64> = HashMap::new();
	for meas in measurements {
		measurement_map.insert(
			meas.area_hash.clone(),
			meas.energy_kwh, // energy is f64; positive means consumption, negative means production
		);
	}
	let mut seen_communities: HashSet<(&str, u64)> = HashSet::new();
	for meas in measurements {
		if seen_communities.insert((meas.community_uuid.as_str(), meas.time_slot)) {
			let community_net_import =
				aggregate_net_import(measurements, &meas.community_uuid, meas.time_slot);
			measurement_map.insert(
				h256_to_string(community_id_from_uuid(&meas.community_uuid)),
				community_net_import,
			);
		}
	}

	// Iterate over each trade and compute the penalty if a measurement exists.
	for trade in trades {
		// For consumers, we use the Bid's area and market.

		if let Some(&measured_energy) = measurement_map.get(&trade.bid.bid_component.area_uuid.clone()) {
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
					market_id: trade.offer.offer_component.market_id.clone(),
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

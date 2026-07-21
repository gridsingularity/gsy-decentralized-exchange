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
///
/// Each trade is validated with TWO independent checks, one per side, each against
/// that side's own meter and both compared to the trade's `selected_energy`:
///
/// * **Buyer check (over-consumption):** look up the bid area's measurement
///   (`trade.bid.bid_component.area_uuid`). Measurements store net energy with
///   consumption positive, so when the measured consumption exceeds the traded
///   `selected_energy` the buyer is penalized on the excess.
/// * **Seller check (under-production):** look up the offer area's measurement
///   (`trade.offer.offer_component.area_uuid`). Production is stored as negative net
///   energy, so the measured production magnitude is `(-measured).max(0.0)`; when it
///   falls short of the traded `selected_energy` the seller is penalized on the
///   shortfall.
///
/// The two checks are fully independent: a missing measurement on one side never
/// suppresses the other side's check (the old design conflated both into a single
/// signed delta keyed on the buyer's meter, so the seller was judged by the buyer's
/// measurement and skipped entirely whenever the buyer had none).
///
/// Community-level (inter-community) trades key both lookups on the community-id
/// hash, which is inserted into the same `measurement_map`, so they inherit the
/// aggregate net-import behavior.
///
/// Known limitation (pre-existing, deliberately not fixed here): when a single area
/// has several trades within one time slot, each trade is checked against the full
/// area measurement independently. This can under-penalize aggregate shortfalls,
/// because the same area production/consumption is re-used for every trade rather
/// than being apportioned across them.
pub fn compute_penalties(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
	penalty_rate: f64,
) -> Vec<Penalty> {
	let mut penalties = Vec::new();

	// Create a lookup map for measurements by area_uuid
	// TODO: temporarily use only the area_hash for identifying measurements. Should be improved
	// by adding market_id in the measurements, and use this too for identification.
	// Sign convention: `energy_kwh` is signed net energy; positive means consumption,
	// negative means production.
	let mut measurement_map: HashMap<String, f64> = HashMap::new();
	for meas in measurements {
		measurement_map.insert(meas.area_hash.clone(), meas.energy_kwh);
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

	// Iterate over each trade and run the two independent per-side checks.
	for trade in trades {
		let traded_energy = trade.parameters.selected_energy;

		// Buyer check (over-consumption): judged by the bid area's meter.
		// A negative (production) measurement can never exceed `selected_energy`, so a
		// production area sitting on the buyer side simply never triggers a buyer penalty.
		if let Some(&measured_energy) =
			measurement_map.get(&trade.bid.bid_component.area_uuid)
		{
			if measured_energy > traded_energy {
				let excess = measured_energy - traded_energy;
				let raw_penalty = excess * penalty_rate;
				// Scale and convert to u64: apply a scaling factor of 10,000.
				let penalty_cost = (raw_penalty * 10_000.0).round() as u64;

				penalties.push(Penalty {
					penalized_account: trade.buyer.clone(),
					market_id: trade.offer.offer_component.market_id.clone(),
					trade_uuid: trade.trade_uuid.clone(),
					penalty_cost,
				});
			}
		}

		// Seller check (under-production): judged by the offer area's meter.
		// Production is stored as negative net energy, so the measured production
		// magnitude is `(-measured).max(0.0)`.
		if let Some(&measured_energy) =
			measurement_map.get(&trade.offer.offer_component.area_uuid)
		{
			let measured_production = (-measured_energy).max(0.0);
			if measured_production < traded_energy {
				let delta = traded_energy - measured_production;
				let raw_penalty = delta * penalty_rate;
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

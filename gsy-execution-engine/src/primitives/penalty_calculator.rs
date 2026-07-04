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

	// In addition to the per-asset entries above (used by spot trades), insert one
	// community-aggregate entry per community present in the measurements, used by
	// inter-community trades. For those trades the trade's `area_uuid` is not a
	// per-asset `area_hash` but the community hash `community_id_from_uuid(community_uuid)`,
	// and no single per-asset measurement carries it. We therefore group the measurements
	// by community (and time_slot) and index the community net import under that same hash.
	//
	// The community net import is `aggregate_net_import`, i.e. Σ signed energy_kwh =
	// Σ consumption − Σ production, which keeps the exact sign convention of the per-asset
	// energy above, so `delta = measured − traded` retains its meaning for both paths.
	//
	// The community-hash keys live in a distinct H256 domain from the per-asset `area_hash`
	// keys, so these extra entries never shadow the per-asset entries: a spot trade still
	// resolves to its per-asset measurement and its penalty stays byte-identical, while an
	// inter-community trade resolves to the community aggregate. No `MarketType` branch is
	// needed at lookup time.
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

#[cfg(test)]
mod tests {
	use super::*;
	use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
	use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeStatus};

	const TIME_SLOT: u64 = 100;
	const PENALTY_RATE: f64 = 0.10;

	fn measurement(
		area_hash: &str,
		community_uuid: &str,
		energy_kwh: f64,
	) -> MeasurementSchema {
		MeasurementSchema {
			area_uuid: format!("{}_uuid", area_hash),
			area_hash: area_hash.to_string(),
			community_uuid: community_uuid.to_string(),
			time_slot: TIME_SLOT,
			creation_time: 0,
			energy_kwh,
		}
	}

	fn order_component(area_uuid: &str, market_id: &str, energy: f64) -> DbOrderComponent {
		DbOrderComponent {
			area_uuid: area_uuid.to_string(),
			market_id: market_id.to_string(),
			time_slot: TIME_SLOT,
			creation_time: 0,
			energy,
			energy_rate: 1.0,
		}
	}

	fn trade(
		buyer: &str,
		seller: &str,
		bid_area: &str,
		market_id: &str,
		selected_energy: f64,
		trade_uuid: &str,
	) -> TradeSchema {
		TradeSchema {
			_id: trade_uuid.to_string(),
			status: TradeStatus::Settled,
			seller: seller.to_string(),
			buyer: buyer.to_string(),
			market_id: market_id.to_string(),
			time_slot: TIME_SLOT,
			trade_uuid: trade_uuid.to_string(),
			creation_time: 0,
			offer: DbOffer {
				seller: seller.to_string(),
				nonce: 0,
				offer_component: order_component(&format!("{}_seller", bid_area), market_id, selected_energy),
			},
			offer_hash: String::new(),
			bid: DbBid {
				buyer: buyer.to_string(),
				nonce: 0,
				bid_component: order_component(bid_area, market_id, selected_energy),
			},
			bid_hash: String::new(),
			residual_offer: None,
			residual_bid: None,
			parameters: TradeParameters {
				selected_energy,
				energy_rate: 1.0,
				trade_uuid: trade_uuid.to_string(),
			},
		}
	}

	/// Byte-for-byte copy of the pre-change `compute_penalties` (per-asset lookup only).
	/// Used to prove the spot settlement path is unchanged by the community-aggregate addition.
	fn legacy_compute_penalties(
		trades: &[TradeSchema],
		measurements: &[MeasurementSchema],
		penalty_rate: f64,
	) -> Vec<Penalty> {
		let mut penalties = Vec::new();
		let mut measurement_map: HashMap<String, f64> = HashMap::new();
		for meas in measurements {
			measurement_map.insert(meas.area_hash.clone(), meas.energy_kwh);
		}
		for trade in trades {
			if let Some(&measured_energy) =
				measurement_map.get(&trade.bid.bid_component.area_uuid.clone())
			{
				let traded_energy = trade.parameters.selected_energy;
				let delta = measured_energy - traded_energy;
				if delta > 0.0 {
					let raw_penalty = delta * penalty_rate;
					let penalty_cost = (raw_penalty * 10_000.0).round() as u64;
					penalties.push(Penalty {
						penalized_account: trade.buyer.clone(),
						market_id: trade.offer.offer_component.market_id.clone(),
						trade_uuid: trade.trade_uuid.clone(),
						penalty_cost,
					});
				} else if delta < 0.0 {
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

	fn as_tuples(penalties: &[Penalty]) -> Vec<(String, String, String, u64)> {
		penalties
			.iter()
			.map(|p| {
				(
					p.penalized_account.clone(),
					p.market_id.clone(),
					p.trade_uuid.clone(),
					p.penalty_cost,
				)
			})
			.collect()
	}

	#[test]
	fn inter_community_trade_settles_against_community_net() {
		// Community "CommA" per-asset measurements: mixed +consumption / -production.
		// net import = 5.0 - 3.0 + 1.0 = 3.0 (Σ consumption − Σ production).
		let measurements = vec![
			measurement("assetA1", "CommA", 5.0),
			measurement("assetA2", "CommA", -3.0),
			measurement("assetA3", "CommA", 1.0),
		];

		// One inter-community trade: its area_uuid is the community hash, not any area_hash.
		let community_area = h256_to_string(community_id_from_uuid("CommA"));
		let market_id = "inter_community_market";
		let traded_energy = 2.0;
		let trades = vec![trade(
			"buyer_acc",
			"seller_acc",
			&community_area,
			market_id,
			traded_energy,
			"trade-ic-1",
		)];

		let penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);

		// delta = measured(net 3.0) - traded(2.0) = 1.0 > 0 -> buyer penalized.
		// penalty_cost = (1.0 * 0.10 * 10_000).round() = 1000.
		assert_eq!(penalties.len(), 1);
		assert_eq!(penalties[0].penalized_account, "buyer_acc");
		assert_eq!(penalties[0].market_id, market_id);
		assert_eq!(penalties[0].trade_uuid, "trade-ic-1");
		assert_eq!(penalties[0].penalty_cost, 1000);
	}

	#[test]
	fn spot_penalties_are_byte_identical_to_pre_change() {
		// A per-asset (spot) measurement and matching spot trade, PLUS community
		// measurements that now also produce community-aggregate entries. The spot
		// lookup keys off the per-asset area_hash, which lives in a different key space,
		// so the community aggregates must not perturb the spot penalties at all.
		let measurements = vec![
			measurement("spot_asset_hash", "SpotComm", 6.0),
			// Extra community members -> exercise the aggregate-insertion branch.
			measurement("other_asset_1", "SpotComm", -2.0),
			measurement("other_asset_2", "OtherComm", 4.0),
		];

		let market_id = "spot_market";
		let traded_energy = 4.0;
		let trades = vec![trade(
			"spot_buyer",
			"spot_seller",
			"spot_asset_hash",
			market_id,
			traded_energy,
			"trade-spot-1",
		)];

		let new_penalties = compute_penalties(&trades, &measurements, PENALTY_RATE);
		let legacy_penalties = legacy_compute_penalties(&trades, &measurements, PENALTY_RATE);

		// Byte-identical to the pre-change implementation for the same inputs.
		assert_eq!(as_tuples(&new_penalties), as_tuples(&legacy_penalties));

		// And the concrete expected spot penalty: delta = 6.0 - 4.0 = 2.0 > 0 -> buyer,
		// penalty_cost = (2.0 * 0.10 * 10_000).round() = 2000.
		assert_eq!(new_penalties.len(), 1);
		assert_eq!(new_penalties[0].penalized_account, "spot_buyer");
		assert_eq!(new_penalties[0].market_id, market_id);
		assert_eq!(new_penalties[0].penalty_cost, 2000);
	}
}

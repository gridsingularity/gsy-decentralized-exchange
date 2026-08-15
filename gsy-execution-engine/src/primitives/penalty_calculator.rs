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
/// Aggregate behavior: because residual trading routinely splits one order into several
/// trades within the same time slot, all traded energy for a given `(area, side, time
/// slot)` group is summed/allocated against that area's meter rather than re-comparing
/// the full area measurement against each trade independently (which under-penalized
/// aggregate shortfalls). The two sides allocate the aggregate differently:
///
/// * **Seller (under-production) — waterfall / time priority:** the measured production
///   is filled across the group's trades in `(creation_time, trade_uuid)` order, honoring
///   the earliest commitments first. Each trade is penalized only on its own uncovered
///   energy after production has been drawn down by the earlier trades, so a single area's
///   aggregate shortfall lands on its later trades rather than being split pro-rata.
/// * **Buyer (over-consumption) — pro-rata:** the aggregate excess consumption is
///   apportioned pro-rata across the group's trades (largest-remainder method, so the
///   parts sum exactly to the aggregate). Over-consumption is a flat overage with no
///   natural per-trade ordering, so there is nothing to give time priority to.
/// Builds the measurement lookup map keyed by area_hash, plus one aggregate entry per
/// `(community_uuid, time_slot)` keyed by the community-id hash.
///
/// TODO: temporarily use only the area_hash for identifying measurements. Should be improved
/// by adding market_id in the measurements, and use this too for identification.
/// Sign convention: `energy_kwh` is signed net energy; positive means consumption,
/// negative means production.
pub fn build_measurement_map(measurements: &[MeasurementSchema]) -> HashMap<String, f64> {
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
	measurement_map
}

/// Returns the `trade_uuid` of every trade that was actually evaluated — i.e. at least one of
/// its bid/offer areas (or their community aggregate) has a measurement — in input order,
/// de-duplicated.
///
/// `compute_penalties` `continue`s past a group whose area is missing from the measurement
/// map: it does not judge that side at all. A trade with no measurement on either side was
/// never judged and must not be reported as clean, or the caller would mark it `Executed`
/// before the engine ever saw its meter reading.
pub fn evaluated_trade_uuids(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
) -> Vec<String> {
	let measurement_map = build_measurement_map(measurements);
	let mut seen: HashSet<String> = HashSet::new();
	let mut uuids = Vec::new();
	for trade in trades {
		let measured = measurement_map.contains_key(&trade.bid.bid_component.area_uuid)
			|| measurement_map.contains_key(&trade.offer.offer_component.area_uuid);
		if measured && seen.insert(trade.trade_uuid.clone()) {
			uuids.push(trade.trade_uuid.clone());
		}
	}
	uuids
}

pub fn compute_penalties(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
	penalty_rate: f64,
) -> Vec<Penalty> {
	let mut penalties = Vec::new();

	let measurement_map = build_measurement_map(measurements);

	// Two independent passes, buyer then seller. This pass ordering keeps the output
	// deterministic. Inter-community trades carry the community hash as their
	// `area_uuid`, so they group under the community-hash key and compare against the
	// community net-import aggregate entry — a different key space from per-asset
	// trades, so there is no double counting; multiple inter-community trades for the
	// same community/slot correctly aggregate together too.

	// Buyer pass (over-consumption): judged by the bid area's meter.
	let buyer_groups = group_trades(trades, |trade| {
		(
			trade.bid.bid_component.area_uuid.clone(),
			trade.bid.bid_component.time_slot,
		)
	});
	for (key, indices) in &buyer_groups {
		let area_uuid = &key.0;
		let Some(&measured) = measurement_map.get(area_uuid) else {
			// No measurement for this area -> no penalty (matches previous behavior).
			continue;
		};
		let total_bought: f64 = indices
			.iter()
			.map(|&i| trades[i].parameters.selected_energy)
			.sum();
		// A negative (production) measurement can never exceed `total_bought`, so a
		// production area sitting on the buyer side never triggers a buyer penalty.
		if measured <= total_bought {
			continue;
		}
		let aggregate_excess = measured - total_bought;
		let aggregate_penalty_cost = (aggregate_excess * penalty_rate * 10_000.0).round() as u64;
		let weights: Vec<f64> = indices
			.iter()
			.map(|&i| trades[i].parameters.selected_energy)
			.collect();
		let parts = apportion(aggregate_penalty_cost, &weights);
		for (&i, &penalty_cost) in indices.iter().zip(parts.iter()) {
			if penalty_cost == 0 {
				continue;
			}
			let trade = &trades[i];
			penalties.push(Penalty {
				penalized_account: trade.buyer.clone(),
				market_id: trade.offer.offer_component.market_id.clone(),
				trade_uuid: trade.trade_uuid.clone(),
				penalty_cost,
			});
		}
	}

	// Seller pass (under-production): judged by the offer area's meter.
	// Production is stored as negative net energy, so the measured production
	// magnitude is `(-measured).max(0.0)`.
	let seller_groups = group_trades(trades, |trade| {
		(
			trade.offer.offer_component.area_uuid.clone(),
			trade.offer.offer_component.time_slot,
		)
	});
	for (key, indices) in &seller_groups {
		let area_uuid = &key.0;
		let Some(&measured) = measurement_map.get(area_uuid) else {
			continue;
		};
		let measured_production = (-measured).max(0.0);

		// Waterfall / time-priority fill: honor the earliest commitments first, ordering
		// the group's trades by `(creation_time asc, trade_uuid asc)`. Production covers
		// the earliest trades in full; once it is exhausted, the remaining (later) trades
		// absorb the shortfall and are penalized on their uncovered energy.
		let mut ordered: Vec<usize> = indices.clone();
		ordered.sort_by(|&a, &b| {
			trades[a]
				.creation_time
				.cmp(&trades[b].creation_time)
				.then_with(|| trades[a].trade_uuid.cmp(&trades[b].trade_uuid))
		});

		let mut remaining_budget = measured_production;
		for &i in &ordered {
			let trade = &trades[i];
			let selected_energy = trade.parameters.selected_energy;
			let covered = remaining_budget.min(selected_energy);
			let uncovered = selected_energy - covered;
			remaining_budget -= covered;
			if uncovered <= 0.0 {
				continue;
			}
			let penalty_cost = (uncovered * penalty_rate * 10_000.0).round() as u64;
			if penalty_cost == 0 {
				continue;
			}
			penalties.push(Penalty {
				penalized_account: trade.seller.clone(),
				market_id: trade.market_id.clone(),
				trade_uuid: trade.trade_uuid.clone(),
				penalty_cost,
			});
		}
	}

	penalties
}

/// Groups trade indices by a key derived from each trade, preserving first-seen group
/// order and input order within each group. Does not rely on `HashMap` iteration order,
/// so the returned order is fully deterministic.
fn group_trades<F>(trades: &[TradeSchema], key_of: F) -> Vec<((String, u64), Vec<usize>)>
where
	F: Fn(&TradeSchema) -> (String, u64),
{
	let mut order: Vec<(String, u64)> = Vec::new();
	let mut groups: HashMap<(String, u64), Vec<usize>> = HashMap::new();
	for (i, trade) in trades.iter().enumerate() {
		let key = key_of(trade);
		if !groups.contains_key(&key) {
			order.push(key.clone());
		}
		groups.entry(key).or_default().push(i);
	}
	order
		.into_iter()
		.map(|key| {
			let indices = groups.remove(&key).unwrap();
			(key, indices)
		})
		.collect()
}

/// Apportions `total` integer units across entries weighted by `weights` using the
/// largest-remainder method, so the returned parts sum EXACTLY to `total` (no drift
/// from independent rounding). If `weights` is empty or their sum is 0.0, all zeros
/// are returned. Leftover units are handed to the largest fractional remainders,
/// breaking ties by lowest index for determinism.
fn apportion(total: u64, weights: &[f64]) -> Vec<u64> {
	let n = weights.len();
	if n == 0 {
		return Vec::new();
	}
	let sum_weights: f64 = weights.iter().sum();
	if sum_weights == 0.0 {
		return vec![0; n];
	}

	let mut result = vec![0u64; n];
	let mut remainders: Vec<(f64, usize)> = Vec::with_capacity(n);
	let mut assigned: u64 = 0;
	for (i, &w) in weights.iter().enumerate() {
		let ideal = total as f64 * w / sum_weights;
		let floor = ideal.floor();
		result[i] = floor as u64;
		assigned += floor as u64;
		remainders.push((ideal - floor, i));
	}

	let mut leftover = total - assigned;
	// Largest remainder first; ties broken by lowest index.
	remainders.sort_by(|a, b| {
		b.0.partial_cmp(&a.0)
			.unwrap_or(std::cmp::Ordering::Equal)
			.then(a.1.cmp(&b.1))
	});
	for &(_, i) in &remainders {
		if leftover == 0 {
			break;
		}
		result[i] += 1;
		leftover -= 1;
	}

	debug_assert_eq!(result.iter().sum::<u64>(), total);
	result
}

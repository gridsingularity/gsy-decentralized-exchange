use gsy_offchain_primitives::db_api_schema::{
	profiles::{MeasurementCompleteness, MeasurementSchema},
	trades::{TradeSchema, TradeStatus},
};
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use tracing::warn;

/// A metering-point deviation at or below this (kWh) counts as no deviation.
const METERING_POINT_TOLERANCE_KWH: f64 = 1e-9;

/// Why a penalty was issued. Informational only: it is not submitted on-chain.
#[derive(Debug, Clone, PartialEq)]
pub enum PenaltyReason {
	/// The measured energy deviated from the traded energy.
	Deviation,
	/// The side's measurement is incomplete or missing, so it is penalized on its full energy.
	/// `source` is the metering point's name, or the community uuid of an inter-community side.
	MissingMeasurement { source: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Penalty {
	pub penalized_account: String,
	pub market_id: String,
	pub trade_uuid: String,
	pub penalty_cost: u64,
	pub reason: PenaltyReason,
}

/// A value in the measurement map built by `build_measurement_map`.
#[derive(Debug, Clone, PartialEq)]
pub enum MeasuredEnergy {
	/// Signed net energy in kWh; positive means consumption, negative production.
	Energy(f64),
	/// A community aggregate that cannot be used, because at least one of the community's
	/// metering-point rows for the slot is incomplete or missing.
	Unreliable { community_uuid: String },
}

/// Which side of a trade is being judged.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Side {
	Bid,
	Offer,
}

/// Builds the measurement lookup map keyed by `(area_hash, time_slot)`, plus one aggregate entry
/// per `(community_uuid, time_slot)` keyed by `(community-id hash, time_slot)`.
///
/// Only plain per-area rows (`metering_point: None`) get a per-area entry; metering-point rows
/// are looked up through `metering_point_index` instead. A community aggregate sums the per-area
/// rows and the `Complete` metering-point rows of the community and slot; it is `Unreliable` if
/// any metering-point row of that community and slot is `Incomplete` or `Missing`. A `Missing`
/// row with no `missing_meters` is an unmetered point (it expects no meter, so it can never be
/// measured) and is skipped by the aggregate; its members are still judged through
/// `metering_point_index`.
///
/// TODO: temporarily use only the area_hash for identifying measurements. Should be improved
/// by adding market_id in the measurements, and use this too for identification.
/// Sign convention: `energy_kwh` is signed net energy; positive means consumption,
/// negative means production.
pub fn build_measurement_map(
	measurements: &[MeasurementSchema],
) -> HashMap<(String, u64), MeasuredEnergy> {
	let mut measurement_map: HashMap<(String, u64), MeasuredEnergy> = HashMap::new();
	for meas in measurements.iter().filter(|meas| meas.metering_point.is_none()) {
		let key = (meas.area_hash.clone(), meas.time_slot);
		measurement_map.insert(key, MeasuredEnergy::Energy(meas.energy_kwh));
	}
	// `None` marks a community slot with an incomplete or missing metering-point row.
	let mut community_aggregates: HashMap<(&str, u64), Option<f64>> = HashMap::new();
	for meas in measurements.iter().filter(|meas| !is_unmetered_point(meas)) {
		let aggregate = community_aggregates
			.entry((meas.community_uuid.as_str(), meas.time_slot))
			.or_insert(Some(0.0));
		let contribution = match &meas.metering_point {
			None => Some(meas.energy_kwh),
			Some(point) => match point.completeness {
				MeasurementCompleteness::Complete => Some(meas.energy_kwh),
				MeasurementCompleteness::Incomplete | MeasurementCompleteness::Missing => None,
			},
		};
		*aggregate = match (*aggregate, contribution) {
			(Some(sum), Some(energy)) => Some(sum + energy),
			_ => None,
		};
	}
	for ((community_uuid, time_slot), aggregate) in community_aggregates {
		let value = match aggregate {
			Some(energy) => MeasuredEnergy::Energy(energy),
			None => MeasuredEnergy::Unreliable { community_uuid: community_uuid.to_string() },
		};
		measurement_map
			.insert((h256_to_string(community_id_from_uuid(community_uuid)), time_slot), value);
	}
	measurement_map
}

/// Whether `meas` is the row of a metering point that expects no meter: `Missing` with no
/// `missing_meters`.
fn is_unmetered_point(meas: &MeasurementSchema) -> bool {
	meas.metering_point.as_ref().is_some_and(|point| {
		point.completeness == MeasurementCompleteness::Missing && point.missing_meters.is_empty()
	})
}

/// Indexes the metering-point rows by `(member area hash, time_slot)`. If two rows claim the same
/// member and slot, the first one is kept and a warning is logged.
pub fn metering_point_index(
	measurements: &[MeasurementSchema],
) -> HashMap<(String, u64), &MeasurementSchema> {
	metering_point_rows(measurements)
		.into_iter()
		.map(|(key, row)| (key, &measurements[row]))
		.collect()
}

/// Same as `metering_point_index`, with row indices into `measurements` as values.
fn metering_point_rows(measurements: &[MeasurementSchema]) -> HashMap<(String, u64), usize> {
	let mut index: HashMap<(String, u64), usize> = HashMap::new();
	for (row, meas) in measurements.iter().enumerate() {
		let Some(point) = &meas.metering_point else {
			continue;
		};
		for member in &point.member_area_hashes {
			match index.entry((member.clone(), meas.time_slot)) {
				Entry::Vacant(vacant) => {
					vacant.insert(row);
				},
				Entry::Occupied(occupied) if *occupied.get() != row => {
					let kept = measurements[*occupied.get()]
						.metering_point
						.as_ref()
						.map_or("", |kept| kept.name.as_str());
					warn!(
						"Area {} is a member of metering points {} and {} for slot {}; \
						 judging it at {}",
						member, kept, point.name, meas.time_slot, kept
					);
				},
				Entry::Occupied(_) => {},
			}
		}
	}
	index
}

/// Returns one record per `trade_uuid`, in first-seen order.
///
/// Storage can hold a trade twice: a re-post by the orderbook worker used to add a second record
/// with the same `trade_uuid` and a new `_id`. Counting both copies would double the trade's
/// energy in every sum of `compute_penalties`. The copies differ only in `_id` and status, so the
/// first copy seen is kept, replaced by the first copy with a verdict (`Executed`/`Penalized`)
/// while the kept one is still `Settled`: a trade counts as judged if any copy is.
pub fn dedupe_trades(trades: &[TradeSchema]) -> Vec<TradeSchema> {
	let mut position: HashMap<&str, usize> = HashMap::new();
	let mut unique: Vec<TradeSchema> = Vec::new();
	for trade in trades {
		match position.entry(trade.trade_uuid.as_str()) {
			Entry::Vacant(entry) => {
				entry.insert(unique.len());
				unique.push(trade.clone());
			},
			Entry::Occupied(entry) => {
				let kept = &mut unique[*entry.get()];
				if kept.status == TradeStatus::Settled && trade.status != TradeStatus::Settled {
					*kept = trade.clone();
				}
			},
		}
	}
	unique
}

/// Returns the `trade_uuid` of every trade that was actually evaluated, in input order,
/// de-duplicated. A trade is evaluated if one of its sides is covered by a metering-point row
/// (any completeness) or has a per-area or community entry for its `(area, time_slot)` (a number
/// or `Unreliable`).
///
/// `compute_penalties` skips a side with no row at all: it does not judge that side. A trade
/// with no row on either side was never judged and must not be reported as clean, or the
/// caller would mark it `Executed` before the engine ever saw its meter reading.
pub fn evaluated_trade_uuids(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
) -> Vec<String> {
	let measurement_map = build_measurement_map(measurements);
	let point_rows = metering_point_rows(measurements);
	let judged = |key: &(String, u64)| {
		point_rows.contains_key(key) || measurement_map.contains_key(key)
	};
	let mut seen: HashSet<String> = HashSet::new();
	let mut uuids = Vec::new();
	for trade in trades {
		let measured = judged(&bid_key(trade)) || judged(&offer_key(trade));
		if measured && seen.insert(trade.trade_uuid.clone()) {
			uuids.push(trade.trade_uuid.clone());
		}
	}
	uuids
}

/// Computes penalties for each trade based on the measured energy.
///
/// Each trade side is looked up with its own `(area_uuid, time_slot)`. Sides whose area is a
/// member of a metering-point row for their slot are judged at that point (metering-point pass,
/// below); all other sides go through the per-area buyer and seller passes. One trade can have
/// a side in each, and a trade whose sides sit at two metering points is judged at both.
///
/// **Metering-point pass**, per `(metering point, slot)`, in first-seen order:
///
/// * `Complete` row: `committed = Σ bought − Σ sold` over the member sides and
///   `deviation = measured − committed`. A non-positive deviation is not penalized. Otherwise
///   the member offers are charged first with the time-priority waterfall (their production
///   budget is `max(0, sold − deviation)`), and the rest of the deviation beyond the sold
///   energy is apportioned pro-rata over the member bids (not charged if there are none). A
///   trade with both sides at the point nets out.
/// * `Incomplete` or `Missing` row: every member side is penalized on its full energy.
///
/// **Per-area passes.** Each side is checked against that side's own meter and compared to the
/// trade's `selected_energy`:
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
/// aggregate net-import behavior. If the aggregate is `Unreliable`, the side is penalized
/// on its full energy; with no aggregate at all it stays unjudged.
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
///
/// Trades are first de-duplicated by `trade_uuid` (`dedupe_trades`), so a trade stored twice
/// counts once in every sum and yields at most one penalty per side.
///
/// Output order: metering-point pass, then buyer pass, then seller pass.
pub fn compute_penalties(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
	penalty_rate: f64,
) -> Vec<Penalty> {
	let trades = &dedupe_trades(trades);
	let measurement_map = build_measurement_map(measurements);
	let point_rows = metering_point_rows(measurements);

	let mut penalties = metering_point_penalties(trades, measurements, &point_rows, penalty_rate);

	// Two independent passes over the sides not at a metering point, buyer then seller. This
	// pass ordering keeps the output deterministic. Inter-community trades carry the community
	// hash as their `area_uuid`, so they group under the community-hash key and compare against
	// the community net-import aggregate entry — a different key space from per-asset trades,
	// so there is no double counting; multiple inter-community trades for the same
	// community/slot correctly aggregate together too.

	// Buyer pass (over-consumption): judged by the bid area's meter.
	let buyer_groups = group_trades(trades, |trade| {
		let key = bid_key(trade);
		(!point_rows.contains_key(&key)).then_some(key)
	});
	for (key, indices) in &buyer_groups {
		let measured = match measurement_map.get(key) {
			// No measurement for this area -> no penalty (matches previous behavior).
			None => continue,
			Some(MeasuredEnergy::Unreliable { community_uuid }) => {
				for &i in indices {
					push_full_energy_penalty(
						&mut penalties,
						&trades[i],
						Side::Bid,
						community_uuid,
						penalty_rate,
					);
				}
				continue;
			},
			Some(MeasuredEnergy::Energy(measured)) => *measured,
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
			penalties.push(side_penalty(trade, Side::Bid, penalty_cost, PenaltyReason::Deviation));
		}
	}

	// Seller pass (under-production): judged by the offer area's meter.
	// Production is stored as negative net energy, so the measured production
	// magnitude is `(-measured).max(0.0)`.
	let seller_groups = group_trades(trades, |trade| {
		let key = offer_key(trade);
		(!point_rows.contains_key(&key)).then_some(key)
	});
	for (key, indices) in &seller_groups {
		let measured = match measurement_map.get(key) {
			None => continue,
			Some(MeasuredEnergy::Unreliable { community_uuid }) => {
				for &i in indices {
					push_full_energy_penalty(
						&mut penalties,
						&trades[i],
						Side::Offer,
						community_uuid,
						penalty_rate,
					);
				}
				continue;
			},
			Some(MeasuredEnergy::Energy(measured)) => *measured,
		};
		let measured_production = (-measured).max(0.0);
		push_waterfall_penalties(
			&mut penalties,
			trades,
			indices,
			measured_production,
			penalty_rate,
		);
	}

	penalties
}

/// The metering-point pass of `compute_penalties`.
fn metering_point_penalties(
	trades: &[TradeSchema],
	measurements: &[MeasurementSchema],
	point_rows: &HashMap<(String, u64), usize>,
	penalty_rate: f64,
) -> Vec<Penalty> {
	// Member sides grouped by metering-point row, in first-seen order.
	let mut order: Vec<usize> = Vec::new();
	let mut groups: HashMap<usize, Vec<(usize, Side)>> = HashMap::new();
	for (i, trade) in trades.iter().enumerate() {
		for (side, key) in [(Side::Bid, bid_key(trade)), (Side::Offer, offer_key(trade))] {
			if let Some(&row) = point_rows.get(&key) {
				groups
					.entry(row)
					.or_insert_with(|| {
						order.push(row);
						Vec::new()
					})
					.push((i, side));
			}
		}
	}

	let mut penalties = Vec::new();
	for row in order {
		let meas = &measurements[row];
		let Some(point) = &meas.metering_point else {
			continue;
		};
		let sides = &groups[&row];
		match point.completeness {
			MeasurementCompleteness::Complete => {
				let of_side = |wanted: Side| -> Vec<usize> {
					sides.iter().filter(|(_, side)| *side == wanted).map(|&(i, _)| i).collect()
				};
				let bids = of_side(Side::Bid);
				let offers = of_side(Side::Offer);
				let bought: f64 = bids.iter().map(|&i| trades[i].parameters.selected_energy).sum();
				let sold: f64 = offers.iter().map(|&i| trades[i].parameters.selected_energy).sum();
				let deviation = meas.energy_kwh - (bought - sold);
				if deviation <= METERING_POINT_TOLERANCE_KWH {
					continue;
				}
				// Under-delivery first: the offers keep a production budget of what was sold
				// minus the deviation, filled in time priority.
				let production_budget = (sold - deviation).max(0.0);
				push_waterfall_penalties(
					&mut penalties,
					trades,
					&offers,
					production_budget,
					penalty_rate,
				);
				// Any deviation beyond the sold energy is over-consumption by the member bids.
				let remainder = (deviation - sold).max(0.0);
				if remainder <= METERING_POINT_TOLERANCE_KWH || bids.is_empty() {
					continue;
				}
				let weights: Vec<f64> =
					bids.iter().map(|&i| trades[i].parameters.selected_energy).collect();
				let parts = apportion(energy_cost(remainder, penalty_rate), &weights);
				for (&i, &penalty_cost) in bids.iter().zip(parts.iter()) {
					if penalty_cost == 0 {
						continue;
					}
					penalties.push(side_penalty(
						&trades[i],
						Side::Bid,
						penalty_cost,
						PenaltyReason::Deviation,
					));
				}
			},
			MeasurementCompleteness::Incomplete | MeasurementCompleteness::Missing => {
				for &(i, side) in sides {
					push_full_energy_penalty(
						&mut penalties,
						&trades[i],
						side,
						&point.name,
						penalty_rate,
					);
				}
			},
		}
	}
	penalties
}

/// Waterfall / time-priority fill of offer sides: `production` covers the trades in
/// `(creation_time, trade_uuid)` order; once it is exhausted, the remaining (later) trades
/// absorb the shortfall and their sellers are penalized on their uncovered energy.
fn push_waterfall_penalties(
	penalties: &mut Vec<Penalty>,
	trades: &[TradeSchema],
	indices: &[usize],
	production: f64,
	penalty_rate: f64,
) {
	let mut ordered: Vec<usize> = indices.to_vec();
	ordered.sort_by(|&a, &b| {
		trades[a]
			.creation_time
			.cmp(&trades[b].creation_time)
			.then_with(|| trades[a].trade_uuid.cmp(&trades[b].trade_uuid))
	});

	let mut remaining_budget = production;
	for &i in &ordered {
		let trade = &trades[i];
		let selected_energy = trade.parameters.selected_energy;
		let covered = remaining_budget.min(selected_energy);
		let uncovered = selected_energy - covered;
		remaining_budget -= covered;
		if uncovered <= 0.0 {
			continue;
		}
		let penalty_cost = energy_cost(uncovered, penalty_rate);
		if penalty_cost == 0 {
			continue;
		}
		penalties.push(side_penalty(trade, Side::Offer, penalty_cost, PenaltyReason::Deviation));
	}
}

/// Penalizes one side on its full `selected_energy` because its measurement from `source` is
/// incomplete or missing.
fn push_full_energy_penalty(
	penalties: &mut Vec<Penalty>,
	trade: &TradeSchema,
	side: Side,
	source: &str,
	penalty_rate: f64,
) {
	let penalty_cost = energy_cost(trade.parameters.selected_energy, penalty_rate);
	if penalty_cost == 0 {
		return;
	}
	penalties.push(side_penalty(
		trade,
		side,
		penalty_cost,
		PenaltyReason::MissingMeasurement { source: source.to_string() },
	));
}

/// A buyer penalty is booked on the offer's market, a seller penalty on the trade's market.
fn side_penalty(
	trade: &TradeSchema,
	side: Side,
	penalty_cost: u64,
	reason: PenaltyReason,
) -> Penalty {
	let (penalized_account, market_id) = match side {
		Side::Bid => (&trade.buyer, &trade.offer.offer_component.market_id),
		Side::Offer => (&trade.seller, &trade.market_id),
	};
	Penalty {
		penalized_account: penalized_account.clone(),
		market_id: market_id.clone(),
		trade_uuid: trade.trade_uuid.clone(),
		penalty_cost,
		reason,
	}
}

fn energy_cost(energy_kwh: f64, penalty_rate: f64) -> u64 {
	(energy_kwh * penalty_rate * 10_000.0).round() as u64
}

fn bid_key(trade: &TradeSchema) -> (String, u64) {
	(trade.bid.bid_component.area_uuid.clone(), trade.bid.bid_component.time_slot)
}

fn offer_key(trade: &TradeSchema) -> (String, u64) {
	(trade.offer.offer_component.area_uuid.clone(), trade.offer.offer_component.time_slot)
}

/// Groups trade indices by a key derived from each trade (trades with no key are skipped),
/// preserving first-seen group order and input order within each group. Does not rely on
/// `HashMap` iteration order, so the returned order is fully deterministic.
fn group_trades<F>(trades: &[TradeSchema], key_of: F) -> Vec<((String, u64), Vec<usize>)>
where
	F: Fn(&TradeSchema) -> Option<(String, u64)>,
{
	let mut order: Vec<(String, u64)> = Vec::new();
	let mut groups: HashMap<(String, u64), Vec<usize>> = HashMap::new();
	for (i, trade) in trades.iter().enumerate() {
		let Some(key) = key_of(trade) else {
			continue;
		};
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

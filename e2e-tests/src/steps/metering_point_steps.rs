//! End-to-end steps for the execution engine's metering-point verdicts.
//!
//! A metering-point measurement row stands for one building: its `energy_kwh` is the building's
//! net import for the slot and `member_area_hashes` lists the assets behind it. The execution
//! engine judges every trade side whose area is a member of such a row at the building, not at
//! the asset:
//!
//! * `Complete`: `committed = Σ bought − Σ sold` over the member sides and
//!   `deviation = measured − committed`. No deviation is clean; otherwise the member offers are
//!   charged first in time priority, and any deviation beyond the sold energy is charged pro rata
//!   to the member bids.
//! * `Incomplete` / `Missing`: every member side is charged its full traded energy.
//!
//! The steps are data driven: the feature lists the community areas, the orders, the expected
//! trades, the measurement rows, the expected verdicts, statuses and certificates. Trades are
//! identified by their bid (buyer) area, never by the order in which they settle. An expected
//! trade's `seller_area` may list several areas, meaning it is sold by one of them; the trades
//! listing the same areas must each be sold by a different one.

use crate::world::{gsy_node, CapturedTrade, MeteringPointOrder, MyWorld};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use cucumber::gherkin::Step;
use cucumber::{then, when};
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_forecasts::pv_api::parse_response;
use gsy_community_client::external_forecasts::pv_pricing::PvCommitmentConfig;
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::{
	deterministic_area_hash, deterministic_area_uuid, AreaMarketInfoAdapter,
};
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::market::{
	AreaTopologySchema, AssetType, MarketTopologySchema,
};
use gsy_offchain_primitives::db_api_schema::profiles::{
	ForecastSchema, MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
};
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use gsy_offchain_primitives::utils::{h256_to_string, string_to_h256};
use gsy_offchain_primitives::MarketType;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use subxt::utils::{AccountId32, H256};
use tracing::info;

const BID_RATE: f64 = 0.3;
/// Flat offer rate handed to `publish_orders`, matching the MIN_ORDER_RATE default that the
/// offer ramp reaches at market close (same as the PV-penalty scenario).
const OFFER_RATE: f64 = 0.07;
/// Demand forecaster fixed confidence (see manager.rs `DEMAND_FORECAST_CONFIDENCE`).
const DEMAND_FORECAST_CONFIDENCE: f64 = 0.9;
/// On-chain energies are kWh scaled ×10000.
const ENERGY_SCALE: f64 = 10000.0;
/// Finalized blocks to wait for all the trades of the market to settle.
const SETTLEMENT_BLOCKS: usize = 60;
/// Finalized blocks to wait for every expected verdict.
const VERDICT_BLOCKS: usize = 60;
/// Finalized blocks to keep watching once every expected verdict is seen, so that an unexpected
/// extra penalty on one of our trades (e.g. on the other side) is still caught. Several blocks
/// cover more than one 30 s execution-engine cycle.
const VERDICT_GRACE_BLOCKS: usize = 10;

fn orderbook_url() -> String {
	std::env::var("OFFCHAIN_STORAGE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn node_url() -> String {
	std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string())
}

/// A slot two hours out, aligned to the 15-minute market cadence (matches every other scenario).
fn next_delivery_slot() -> u64 {
	let now = Utc::now();
	((now + ChronoDuration::hours(2)).timestamp() as u64 / GlobalConstants.TIME_SLOT_SEC)
		* GlobalConstants.TIME_SLOT_SEC
}

/// Build a PV-forecaster response body with one daytime point aligned to `slot` and one all-zero
/// night point (same shape as `penalty_steps::pv_response_body`).
fn pv_response_body(slot: u64, pv_watts: f64, p5_watts: f64, p95_watts: f64) -> String {
	let day_ts = DateTime::<Utc>::from_timestamp(slot as i64, 0)
		.expect("valid slot timestamp")
		.naive_utc()
		.format("%Y-%m-%dT%H:%M:%S");
	format!(
		r#"{{
            "data": {{
                "pv_forecasts": [
                    {{
                        "timestamp": "{day_ts}",
                        "pv_forecast": {pv_watts},
                        "p5": [{p5_watts}, {p5_watts}],
                        "p95": [{p95_watts}, {p95_watts}]
                    }},
                    {{
                        "timestamp": "2020-01-01T00:00:00",
                        "pv_forecast": 0,
                        "p5": [0, 0],
                        "p95": [0, 0]
                    }}
                ]
            }}
        }}"#
	)
}

/// The step's data table as one map per row, keyed by the header row.
fn table_rows(step: &Step) -> Vec<HashMap<String, String>> {
	let table = step
		.table
		.as_ref()
		.unwrap_or_else(|| panic!("step \"{}\" needs a data table", step.value));
	let (header, rows) =
		table.rows.split_first().unwrap_or_else(|| panic!("step \"{}\" has an empty table", step.value));
	rows.iter()
		.map(|row| {
			header
				.iter()
				.map(|column| column.trim().to_string())
				.zip(row.iter().map(|cell| cell.trim().to_string()))
				.collect()
		})
		.collect()
}

/// A cell of a table row, panicking with the column name if the column is absent.
fn cell<'a>(row: &'a HashMap<String, String>, column: &str) -> &'a str {
	row.get(column)
		.unwrap_or_else(|| panic!("table row {:?} has no \"{}\" column", row, column))
		.as_str()
}

fn parse_kwh(value: &str, what: &str) -> f64 {
	value.parse::<f64>().unwrap_or_else(|_| panic!("{} \"{}\" is not a number", what, value))
}

/// A comma-separated cell as a list; an empty cell is an empty list.
fn split_list(value: &str) -> Vec<String> {
	value
		.split(',')
		.map(str::trim)
		.filter(|item| !item.is_empty())
		.map(str::to_string)
		.collect()
}

fn parse_asset_type(value: &str) -> AssetType {
	match value {
		"PV" => AssetType::PV,
		"SMART_METER" => AssetType::SMART_METER,
		"BATTERY" => AssetType::BATTERY,
		"GRID_METER" => AssetType::GRID_METER,
		"EV" => AssetType::EV,
		"HEAT_PUMP" => AssetType::HEAT_PUMP,
		"BOILER" => AssetType::BOILER,
		other => panic!("unknown area type \"{}\"", other),
	}
}

fn parse_completeness(value: &str) -> MeasurementCompleteness {
	match value {
		"complete" => MeasurementCompleteness::Complete,
		"incomplete" => MeasurementCompleteness::Incomplete,
		"missing" => MeasurementCompleteness::Missing,
		other => panic!("unknown completeness \"{}\" (complete, incomplete or missing)", other),
	}
}

fn scaled_energy(kwh: f64) -> u64 {
	(kwh * ENERGY_SCALE).round() as u64
}

fn market(world: &MyWorld) -> MarketTopologySchema {
	world
		.metering_point_market
		.clone()
		.expect("the metering-point community must be created first")
}

/// Find an area of a given name in a market topology.
fn area_by_name<'a>(market: &'a MarketTopologySchema, name: &str) -> &'a AreaTopologySchema {
	market.community_areas.iter().find(|a| a.name == name).unwrap_or_else(|| {
		panic!("area \"{}\" is not in the topology of community {}", name, market.community_name)
	})
}

fn account_of(world: &MyWorld, user: &str) -> AccountId32 {
	world
		.users
		.get(user)
		.unwrap_or_else(|| panic!("unknown user \"{}\"", user))
		.public_key()
		.into()
}

/// The captured trade whose bid area is `buyer_area`.
fn trade_bought_by<'a>(world: &'a MyWorld, buyer_area: &str) -> &'a CapturedTrade {
	world.metering_point_trades.get(buyer_area).unwrap_or_else(|| {
		panic!(
			"no settled trade bought by \"{}\" was captured; captured trades are bought by {:?}",
			buyer_area,
			world.metering_point_trades.keys().collect::<Vec<_>>()
		)
	})
}

#[when(
	regex = r#"^a metering-point community "([^"]*)" is created for the next delivery slot with areas:$"#
)]
async fn create_metering_point_topology(world: &mut MyWorld, step: &Step, community: String) {
	world.target_delivery_time = next_delivery_slot();
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	let areas = table_rows(step)
		.iter()
		.map(|row| ExternalAreaTopology {
			area_type: parse_asset_type(cell(row, "type")),
			area_name: cell(row, "area").to_string(),
		})
		.collect::<Vec<_>>();
	let topology = ExternalCommunityTopology { community_name: community.clone(), areas };
	let market = adapter
		.get_or_create_market_topology(vec![topology], world.target_delivery_time)
		.await
		.into_iter()
		.next()
		.unwrap_or_else(|| panic!("the market of community {} must be created", community));

	assert_eq!(market.community_name, community, "the market must belong to the new community");
	assert_eq!(
		string_to_h256(market.market_id.clone()),
		world.generate_market_id(&community, MarketType::Spot),
		"market id must match the community-aware hash"
	);
	info!(
		"Created metering-point market {} of community {} for slot {}",
		market.market_id, community, world.target_delivery_time
	);
	world.metering_point_market = Some(market);
}

#[when("these metering-point orders are built:")]
async fn build_metering_point_orders(world: &mut MyWorld, step: &Step) {
	let market = market(world);
	let slot = world.target_delivery_time;
	let now = Utc::now().timestamp() as u64;
	let mut orders = Vec::new();

	for row in table_rows(step) {
		let user = cell(&row, "user").to_string();
		assert!(world.users.contains_key(&user), "unknown user \"{}\"", user);
		let area_name = cell(&row, "area").to_string();
		let area = area_by_name(&market, &area_name).clone();
		let energy = parse_kwh(cell(&row, "energy_kwh"), "order energy");
		assert!(energy > 0.0, "order energies are positive, the side gives the sign");

		let forecast = match cell(&row, "side") {
			"offer" => {
				assert_eq!(
					area.area_type,
					AssetType::PV,
					"an offer must come from a PV area, \"{}\" is not one",
					area_name
				);
				// A degenerate p5 == p95 == pv_forecast band yields the maximum confidence and a
				// zero-width band, so the s = -1 commitment is just the point forecast; the
				// average power that delivers `energy` kWh in one slot is `energy × 3600 / slot`
				// kW.
				let watts = energy * 1000.0 * 3600.0 / GlobalConstants.TIME_SLOT_SEC as f64;
				let body = pv_response_body(slot, watts, watts, watts);
				let response = parse_response(&body).expect("PV response must parse");
				let offer = ForecastsManager::pv_forecast_schema_from_point(
					&response.data.pv_forecasts[0],
					&area,
					&market.community_uuid,
					&PvCommitmentConfig::for_offers(),
				)
				.expect("daytime PV point yields a forecast");
				assert!(offer.energy_kwh < 0.0, "a PV production forecast must be negative energy");
				assert!(
					(offer.energy_kwh.abs() - energy).abs() < 1e-9,
					"the PV offer of \"{}\" must commit exactly {} kWh, got {}",
					area_name,
					energy,
					offer.energy_kwh.abs()
				);
				assert_eq!(offer.time_slot, slot, "time_slot aligned to the trade slot");
				offer
			},
			"bid" => ForecastSchema {
				area_uuid: area.area_uuid.clone(),
				area_hash: area.area_hash.clone(),
				community_uuid: market.community_uuid.clone(),
				time_slot: slot,
				creation_time: now,
				energy_kwh: energy,
				confidence: DEMAND_FORECAST_CONFIDENCE,
			},
			other => panic!("unknown order side \"{}\" (offer or bid)", other),
		};
		info!("Built {} kWh {} of {} from {}", energy, cell(&row, "side"), user, area_name);
		orders.push(MeteringPointOrder { user, area_name, forecast });
	}
	world.metering_point_orders = orders;
}

#[when("the Market Orchestrator opens the metering-point Spot market")]
async fn wait_for_metering_point_market_open(world: &mut MyWorld) {
	let community = market(world).community_name;
	let market_id = world.generate_market_id(&community, MarketType::Spot);
	info!("Waiting for the orchestrator to open the metering-point market {:?}...", market_id);

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		info!("Waiting for MarketStatusUpdated... check {}/40", i + 1);
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();
		let events = block.events().await.unwrap();
		for e in events
			.find::<gsy_node::orderbook_registry::events::MarketStatusUpdated>()
			.flatten()
		{
			if e.0 == market_id && e.1 {
				info!("Metering-point market opened on-chain: {:?}", market_id);
				tokio::time::sleep(Duration::from_secs(6)).await;
				return;
			}
		}
	}
	panic!("Timeout: the orchestrator did not open the metering-point market {:?}", market_id);
}

#[when("the metering-point offers and bids are published")]
async fn publish_metering_point_orders(world: &mut MyWorld) {
	let market = market(world);
	assert!(!world.metering_point_orders.is_empty(), "the orders must be built first");

	// One `publish_orders` call per user, in the order the users first appear, so each account's
	// nonce is handled in a single batch (offers are negative energy, bids positive).
	let mut users: Vec<String> = Vec::new();
	for order in &world.metering_point_orders {
		if !users.contains(&order.user) {
			users.push(order.user.clone());
		}
	}
	for user in users {
		let forecasts: Vec<ForecastSchema> = world
			.metering_point_orders
			.iter()
			.filter(|order| order.user == user)
			.map(|order| order.forecast.clone())
			.collect();
		let signer = world.users.get(&user).unwrap().clone();
		let count = forecasts.len();
		publish_orders(node_url(), forecasts, market.clone(), BID_RATE, OFFER_RATE, &signer)
			.await
			.unwrap_or_else(|e| panic!("Failed to publish the orders of {}: {:?}", user, e));
		info!("Published {} order(s) signed by {}", count, user);
	}
}

#[then("the metering-point market settles these trades:")]
async fn capture_metering_point_trades(world: &mut MyWorld, step: &Step) {
	let market = market(world);
	let market_id = world.generate_market_id(&market.community_name, MarketType::Spot);
	let area_names: HashMap<H256, String> = market
		.community_areas
		.iter()
		.map(|area| (string_to_h256(area.area_hash.clone()), area.name.clone()))
		.collect();
	let signer_of_area: HashMap<String, String> = world
		.metering_point_orders
		.iter()
		.map(|order| (order.area_name.clone(), order.user.clone()))
		.collect();

	// Expected trades keyed by buyer area: (candidate seller areas, scaled energy). A single
	// candidate may sell several trades; a trade with several candidates is sold by one of them.
	let mut expected: HashMap<String, (Vec<String>, u64)> = HashMap::new();
	for row in table_rows(step) {
		let buyer_area = cell(&row, "buyer_area").to_string();
		let mut seller_areas = split_list(cell(&row, "seller_area"));
		assert!(
			!seller_areas.is_empty(),
			"the trade bought by \"{}\" needs a seller area",
			buyer_area
		);
		seller_areas.sort();
		let energy = scaled_energy(parse_kwh(cell(&row, "energy_kwh"), "trade energy"));
		assert!(
			expected.insert(buyer_area.clone(), (seller_areas, energy)).is_none(),
			"the expected trades must have distinct buyer areas, \"{}\" is repeated",
			buyer_area
		);
	}

	let mut captured: HashMap<String, CapturedTrade> = HashMap::new();
	// For the trades with several candidate sellers: the buyer area each such seller sold to, per
	// candidate list, so that the trades sharing a list are each sold by a different area of it.
	let mut sold_from_candidates: HashMap<Vec<String>, HashMap<String, String>> = HashMap::new();
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..SETTLEMENT_BLOCKS {
		if captured.len() == expected.len() {
			break;
		}
		info!(
			"Waiting for the metering-point trades... check {}/{} (captured {}/{})",
			i + 1,
			SETTLEMENT_BLOCKS,
			captured.len(),
			expected.len()
		);
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();
		let events = block.events().await.unwrap();
		for e in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>().flatten() {
			let trade = e.0;
			if trade.market_id != market_id {
				continue;
			}
			let area_name = |hash: &H256| {
				area_names.get(hash).cloned().unwrap_or_else(|| format!("{:?}", hash))
			};
			let buyer_area = area_name(&trade.bid.bid_component.area_uuid);
			let seller_area = area_name(&trade.offer.offer_component.area_uuid);
			if let Some(existing) = captured.get(&buyer_area) {
				assert_eq!(
					existing.trade_uuid, trade.trade_uuid,
					"expected one trade bought by \"{}\", but a second one settled: {:?} and {:?}",
					buyer_area, existing.trade_uuid, trade.trade_uuid
				);
				continue;
			}
			let Some((expected_sellers, expected_energy)) = expected.get(&buyer_area) else {
				panic!(
					"unexpected trade {:?} in the metering-point market: bought by \"{}\", sold by \
					 \"{}\", {} kWh",
					trade.trade_uuid,
					buyer_area,
					seller_area,
					trade.parameters.selected_energy as f64 / ENERGY_SCALE
				);
			};
			assert!(
				expected_sellers.contains(&seller_area),
				"the trade bought by \"{}\" must be sold by one of {:?}, got \"{}\"",
				buyer_area,
				expected_sellers,
				seller_area
			);
			if expected_sellers.len() > 1 {
				let sold = sold_from_candidates.entry(expected_sellers.clone()).or_default();
				if let Some(other_buyer) = sold.insert(seller_area.clone(), buyer_area.clone()) {
					panic!(
						"\"{}\" sold both the trade bought by \"{}\" and the one bought by \"{}\", \
						 but each of {:?} must sell a different trade",
						seller_area, other_buyer, buyer_area, expected_sellers
					);
				}
			}
			assert_eq!(
				trade.parameters.selected_energy,
				*expected_energy,
				"the trade bought by \"{}\" must clear {} kWh (scaled ×10000)",
				buyer_area,
				*expected_energy as f64 / ENERGY_SCALE
			);
			let signer_of = |area: &str| {
				signer_of_area.get(area).unwrap_or_else(|| {
					panic!("no order was built from \"{}\", which traded in {:?}", area, trade.trade_uuid)
				})
			};
			let seller_user = signer_of(&seller_area);
			let buyer_user = signer_of(&buyer_area);
			assert_eq!(
				trade.seller,
				account_of(world, seller_user),
				"the trade bought by \"{}\" must be sold by {}",
				buyer_area,
				seller_user
			);
			assert_eq!(
				trade.buyer,
				account_of(world, buyer_user),
				"the trade bought by \"{}\" must be bought by {}",
				buyer_area,
				buyer_user
			);
			info!(
				"Captured trade {:?}: {} kWh from \"{}\" to \"{}\"",
				trade.trade_uuid,
				trade.parameters.selected_energy as f64 / ENERGY_SCALE,
				seller_area,
				buyer_area
			);
			captured.insert(
				buyer_area,
				CapturedTrade {
					trade_uuid: trade.trade_uuid,
					selected_energy: trade.parameters.selected_energy,
					creation_time: trade.creation_time,
				},
			);
		}
	}

	let mut missing: Vec<&String> =
		expected.keys().filter(|buyer| !captured.contains_key(*buyer)).collect();
	missing.sort();
	assert!(
		missing.is_empty(),
		"Timeout: no trade bought by {:?} settled in the metering-point market (captured {:?})",
		missing,
		captured.keys().collect::<Vec<_>>()
	);
	world.metering_point_trades = captured;
}

#[when("these metering-point measurements are submitted for the slot:")]
async fn submit_metering_point_measurements(world: &mut MyWorld, step: &Step) {
	let market = market(world);
	let community = market.community_name.clone();
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));
	let now = Utc::now().timestamp() as u64;

	// One POST per row, in table order: the engine may run a cycle between two rows, and the
	// feature orders the rows so that no partial set gives a different verdict.
	for row in table_rows(step) {
		let name = cell(&row, "metering_point").to_string();
		let mut member_area_hashes: Vec<String> = split_list(cell(&row, "members"))
			.iter()
			.map(|member| {
				let area = area_by_name(&market, member);
				assert_eq!(
					area.area_hash,
					h256_to_string(deterministic_area_hash(&community, member)),
					"the area hash of \"{}\" must be the deterministic one",
					member
				);
				area.area_hash.clone()
			})
			.collect();
		assert!(!member_area_hashes.is_empty(), "metering point {} needs members", name);
		// Sorted, as the community client builds them.
		member_area_hashes.sort();
		let completeness = parse_completeness(cell(&row, "completeness"));
		let energy_kwh = parse_kwh(cell(&row, "energy_kwh"), "measured energy");
		let missing_meters = split_list(cell(&row, "missing_meters"));

		let measurement = MeasurementSchema {
			area_uuid: deterministic_area_uuid(&community, &name),
			area_hash: h256_to_string(deterministic_area_hash(&community, &name)),
			community_uuid: market.community_uuid.clone(),
			time_slot: world.target_delivery_time,
			creation_time: now,
			energy_kwh,
			metering_point: Some(MeteringPointMeasurement {
				name: name.clone(),
				member_area_hashes,
				completeness: completeness.clone(),
				missing_meters: missing_meters.clone(),
			}),
		};
		adapter
			.forward_measurement(vec![measurement])
			.await
			.unwrap_or_else(|e| panic!("forwarding the row of metering point {} failed: {:?}", name, e));
		info!(
			"Submitted metering point {} ({:?}, {} kWh, missing meters {:?}) for slot {}",
			name, completeness, energy_kwh, missing_meters, world.target_delivery_time
		);
	}
}

/// The verdict expected for one trade.
#[derive(Debug)]
enum ExpectedVerdict {
	/// `TradeExecuted`, no penalty.
	Executed,
	/// Exactly one penalty, on this account for this energy, and no `TradeExecuted`.
	Penalized { user: String, account: AccountId32, energy: u64 },
}

#[then("the execution engine gives these metering-point verdicts on-chain:")]
async fn verify_metering_point_verdicts(world: &mut MyWorld, step: &Step) {
	let mut expected: Vec<(String, H256, ExpectedVerdict)> = Vec::new();
	for row in table_rows(step) {
		let buyer_area = cell(&row, "trade_bought_by").to_string();
		let trade_uuid = trade_bought_by(world, &buyer_area).trade_uuid;
		let verdict = match cell(&row, "verdict") {
			"executed" => ExpectedVerdict::Executed,
			"penalized" => {
				let user = cell(&row, "penalized_user").to_string();
				let energy = cell(&row, "penalty_energy");
				ExpectedVerdict::Penalized {
					account: account_of(world, &user),
					user,
					energy: energy.parse().unwrap_or_else(|_| {
						panic!("penalty_energy \"{}\" is not an integer", energy)
					}),
				}
			},
			other => panic!("unknown verdict \"{}\" (executed or penalized)", other),
		};
		expected.push((buyer_area, trade_uuid, verdict));
	}
	let our_uuids: HashSet<H256> = expected.iter().map(|(_, uuid, _)| *uuid).collect();

	info!("Waiting for the execution engine's verdicts on {} trade(s)...", expected.len());
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	let mut penalties: HashMap<H256, Vec<(AccountId32, u64)>> = HashMap::new();
	let mut executed: HashSet<H256> = HashSet::new();
	let mut grace_left: Option<usize> = None;
	for i in 0..VERDICT_BLOCKS + VERDICT_GRACE_BLOCKS {
		if grace_left == Some(0) {
			break;
		}
		if i == VERDICT_BLOCKS && grace_left.is_none() {
			break;
		}
		info!(
			"Waiting for PenaltiesSubmitted / TradeExecuted... check {} ({} executed, {} penalized \
			 so far)",
			i + 1,
			executed.len(),
			penalties.len()
		);
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block for the verdict check")
			.unwrap()
			.unwrap();
		let events = block.events().await.unwrap();
		for e in events
			.find::<gsy_node::trades_settlement::events::PenaltiesSubmitted>()
			.flatten()
		{
			let penalty = e.0;
			if our_uuids.contains(&penalty.trade_uuid) {
				info!(
					"Penalty on trade {:?}: account {:?}, energy {}",
					penalty.trade_uuid, penalty.penalized_account, penalty.penalty_energy
				);
				penalties
					.entry(penalty.trade_uuid)
					.or_default()
					.push((penalty.penalized_account, penalty.penalty_energy));
			}
		}
		for e in events.find::<gsy_node::trades_settlement::events::TradeExecuted>().flatten() {
			if our_uuids.contains(&e.0) {
				info!("Trade {:?} executed", e.0);
				executed.insert(e.0);
			}
		}

		if let Some(left) = grace_left.as_mut() {
			*left -= 1;
		} else if expected.iter().all(|(_, uuid, verdict)| match verdict {
			ExpectedVerdict::Executed => executed.contains(uuid),
			ExpectedVerdict::Penalized { .. } => penalties.contains_key(uuid),
		}) {
			info!(
				"Every expected verdict observed; watching {} more blocks for extra penalties",
				VERDICT_GRACE_BLOCKS
			);
			grace_left = Some(VERDICT_GRACE_BLOCKS);
		}
	}

	for (buyer_area, uuid, verdict) in &expected {
		let trade_penalties = penalties.get(uuid).cloned().unwrap_or_default();
		match verdict {
			ExpectedVerdict::Executed => {
				assert!(
					trade_penalties.is_empty(),
					"the trade bought by \"{}\" ({:?}) must not be penalized, got {:?}",
					buyer_area,
					uuid,
					trade_penalties
				);
				assert!(
					executed.contains(uuid),
					"Timeout: the trade bought by \"{}\" ({:?}) never got TradeExecuted",
					buyer_area,
					uuid
				);
			},
			ExpectedVerdict::Penalized { user, account, energy } => {
				assert_eq!(
					trade_penalties,
					vec![(account.clone(), *energy)],
					"the trade bought by \"{}\" ({:?}) must get exactly one penalty, on {} for \
					 {}; got {:?}",
					buyer_area,
					uuid,
					user,
					energy,
					trade_penalties
				);
				assert!(
					!executed.contains(uuid),
					"the penalized trade bought by \"{}\" ({:?}) must not get TradeExecuted",
					buyer_area,
					uuid
				);
			},
		}
		info!("Verified verdict {:?} on the trade bought by \"{}\" ({:?})", verdict, buyer_area, uuid);
	}
}

/// GET the trade with `trade_uuid` from offchain storage at `slot` (same as
/// `trade_status_steps::fetch_trade`).
async fn fetch_trade(world: &MyWorld, slot: u64, trade_uuid: H256) -> Option<TradeSchema> {
	let url = format!("{}/trades?start_time={}&end_time={}", orderbook_url(), slot, slot);
	let resp = world.http_client.get(url).send().await.expect("GET /trades failed");
	assert!(resp.status().is_success(), "GET /trades returned {}", resp.status());
	let trades = resp.json::<Vec<TradeSchema>>().await.expect("deserialize trades response");
	let target = h256_to_string(trade_uuid);
	trades.into_iter().find(|t| t.trade_uuid == target)
}

/// The current storage status of every `(buyer_area, trade_uuid)`; `None` means not found.
async fn fetch_statuses(
	world: &MyWorld,
	trades: &[(String, H256, TradeStatus)],
) -> Vec<Option<TradeStatus>> {
	let slot = world.target_delivery_time;
	let mut statuses = Vec::new();
	for (_, uuid, _) in trades {
		statuses.push(fetch_trade(world, slot, *uuid).await.map(|t| t.status));
	}
	statuses
}

fn describe_statuses(
	trades: &[(String, H256, TradeStatus)],
	statuses: &[Option<TradeStatus>],
) -> String {
	trades
		.iter()
		.zip(statuses)
		.map(|((buyer_area, uuid, expected), seen)| {
			format!(
				"trade bought by \"{}\" ({:?}) expected {:?}, was {}",
				buyer_area,
				uuid,
				expected,
				seen.as_ref()
					.map(|s| format!("{:?}", s))
					.unwrap_or_else(|| "not found in storage".to_string())
			)
		})
		.collect::<Vec<_>>()
		.join("; ")
}

#[then("the metering-point trades are marked in the offchain storage:")]
async fn verify_metering_point_statuses(world: &mut MyWorld, step: &Step) {
	let mut trades: Vec<(String, H256, TradeStatus)> = Vec::new();
	for row in table_rows(step) {
		let buyer_area = cell(&row, "trade_bought_by").to_string();
		let uuid = trade_bought_by(world, &buyer_area).trade_uuid;
		let status = match cell(&row, "status") {
			"Executed" => TradeStatus::Executed,
			"Penalized" => TradeStatus::Penalized,
			"Settled" => TradeStatus::Settled,
			other => panic!("unknown trade status \"{}\"", other),
		};
		trades.push((buyer_area, uuid, status));
	}
	let wanted: Vec<Option<TradeStatus>> =
		trades.iter().map(|(_, _, status)| Some(status.clone())).collect();

	let mut statuses = Vec::new();
	let mut converged = false;
	for i in 0..40 {
		info!("Waiting for the metering-point trade statuses to converge... check {}/40", i + 1);
		statuses = fetch_statuses(world, &trades).await;
		if statuses == wanted {
			converged = true;
			break;
		}
		tokio::time::sleep(Duration::from_secs(5)).await;
	}
	assert!(
		converged,
		"Timeout: the metering-point trade statuses did not converge: {}",
		describe_statuses(&trades, &statuses)
	);

	// Stability re-check, as in `trade_status_steps`: sleep strictly longer than the engine's
	// 30 s polling interval so at least one further evaluation cycle runs, then assert on a
	// single observation (polling here would hide a status that flips between cycles).
	tokio::time::sleep(Duration::from_secs(45)).await;
	let statuses = fetch_statuses(world, &trades).await;
	assert!(
		statuses == wanted,
		"the metering-point trade statuses were not stable across an evaluation cycle: {}",
		describe_statuses(&trades, &statuses)
	);
	info!("Verified stable metering-point trade statuses: {}", describe_statuses(&trades, &statuses));
}

/// The certificates currently derivable that reference one of our captured trades.
async fn certificates_of_our_trades(world: &MyWorld) -> Vec<Value> {
	let ours: HashSet<String> = world
		.metering_point_trades
		.values()
		.map(|trade| h256_to_string(trade.trade_uuid))
		.collect();
	// The window bounds validation time (when the trade reached `Executed`), stamped by the
	// storage's wall clock within this run, so an hour back is comfortably early enough. It is
	// not scoped to this scenario, so the records are narrowed to our own trades.
	let validated_after = (Utc::now() - ChronoDuration::hours(1)).timestamp() as u64;
	let url = format!(
		"{}/guarantees-of-origin-measurements?start_time={}",
		orderbook_url(),
		validated_after
	);
	let resp = world.http_client.get(&url).send().await.expect("GET certificates failed");
	assert!(
		resp.status().is_success(),
		"GET /guarantees-of-origin-measurements returned {}",
		resp.status()
	);
	let records: Vec<Value> = resp.json().await.expect("deserialize certificates response");
	records
		.into_iter()
		.filter(|record| {
			record["trade_and_delivery"]["trade_reference"]
				.as_array()
				.is_some_and(|refs| refs.iter().any(|r| r.as_str().is_some_and(|r| ours.contains(r))))
		})
		.collect()
}

#[then(
	regex = r#"^the offchain storage certifies only the metering-point trade bought by "([^"]*)", for ([0-9.]+) kWh from metering point "([^"]*)"$"#
)]
async fn verify_metering_point_certificate(
	world: &mut MyWorld,
	buyer_area: String,
	energy_kwh: f64,
	metering_point: String,
) {
	let certified = h256_to_string(trade_bought_by(world, &buyer_area).trade_uuid);
	let market = market(world);
	let slot = world.target_delivery_time;

	// The statuses have converged by now, so the certificate is derivable at once; poll anyway to
	// absorb event-listener lag on a loaded stack.
	let mut ours: Vec<Value> = Vec::new();
	for i in 0..20 {
		ours = certificates_of_our_trades(world).await;
		if !ours.is_empty() {
			break;
		}
		info!("Waiting for the certificate to become derivable... check {}/20", i + 1);
		tokio::time::sleep(Duration::from_secs(5)).await;
	}

	assert_eq!(
		ours.len(),
		1,
		"exactly one certificate among the metering-point trades, for the trade bought by \"{}\"; \
		 got {:?}",
		buyer_area,
		ours
	);
	let record = &ours[0];
	assert_eq!(
		record["trade_and_delivery"]["trade_reference"][0].as_str(),
		Some(certified.as_str()),
		"the certificate must reference the trade bought by \"{}\"",
		buyer_area
	);
	let quantity = record["time_and_quantity"]["energy_quantity"]
		.as_f64()
		.expect("the certificate carries an energy quantity");
	assert!(
		(quantity - energy_kwh).abs() < 1e-9,
		"the certified quantity must be {} kWh, got {}",
		energy_kwh,
		quantity
	);
	assert_eq!(
		record["production_asset"]["metering_point_id"].as_str(),
		Some(metering_point.as_str()),
		"the production asset must name its building's metering point"
	);
	assert_eq!(
		record["beneficiary_and_claim"]["facility_id"].as_str(),
		Some(metering_point.as_str()),
		"the facility must be the seller's building"
	);
	assert_eq!(
		record["measurement_provenance"]["measuring_sensor_id"].as_str(),
		Some(metering_point.as_str()),
		"the measuring sensor must be the seller's building metering point"
	);
	assert_eq!(record["identity"]["record_type"].as_str(), Some("local_origin_record"));
	assert_eq!(record["production_asset"]["asset_class"].as_str(), Some("PV"));
	assert_eq!(
		record["trade_and_delivery"]["trade_status_at_issuance"].as_str(),
		Some("delivery_verified")
	);
	// The building nets negative (it exports), so the evidence is an export.
	assert_eq!(record["measurement_provenance"]["flow_direction"].as_str(), Some("export"));
	assert_eq!(record["time_and_quantity"]["source_slot_timestamp"].as_u64(), Some(slot));
	assert_eq!(
		record["location"]["community_id_origin"].as_str(),
		Some(market.community_name.as_str())
	);
	info!(
		"Verified one certificate, for the trade bought by \"{}\" ({}, {} kWh) from metering point {}",
		buyer_area, certified, energy_kwh, metering_point
	);
}

#[then("the offchain storage certifies none of the metering-point trades")]
async fn verify_no_metering_point_certificate(world: &mut MyWorld) {
	// Nothing to wait for: poll for a while and fail as soon as a certificate shows up.
	for i in 0..6 {
		let ours = certificates_of_our_trades(world).await;
		assert!(ours.is_empty(), "no metering-point trade may be certified, got {:?}", ours);
		info!("No certificate for the metering-point trades... check {}/6", i + 1);
		tokio::time::sleep(Duration::from_secs(5)).await;
	}
	info!("Verified no certificate for the metering-point trades");
}

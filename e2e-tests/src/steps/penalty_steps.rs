//! End-to-end steps for the PV production-shortfall penalty waterfall.
//!
//! One PV asset commits 5.0 kWh and is matched into two trades (3.0 + 2.0 kWh) against two
//! demand bids. A production measurement of 4.0 kWh then falls 1.0 kWh short of the committed
//! 5.0 kWh. The execution engine allocates the measured production across the two trades in
//! `(creation_time, trade_uuid)` order (time priority), so the earlier trade is honored in full
//! and only the LATER trade is penalized on the 1.0 kWh tail shortfall. This mirrors the unit
//! test `seller_aggregate_shortfall_is_waterfalled_to_later_trade` end-to-end on the docker stack.

use crate::world::{gsy_node, CapturedTrade, MyWorld};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use cucumber::{then, when};
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_forecasts::pv_api::parse_response;
use gsy_community_client::external_forecasts::pv_pricing::PvCommitmentConfig;
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::{h256_to_string, string_to_h256};
use gsy_offchain_primitives::MarketType;
use serde_json::Value;
use std::time::Duration;
use subxt::utils::{AccountId32, H256};
use tracing::info;

const PV_PENALTY_COMMUNITY: &str = "PvPenaltyCommunity";
const PV_AREA: &str = "PvPenaltyCommunity_pv";
const METER_A: &str = "PvPenaltyCommunity_meter_a";
const METER_B: &str = "PvPenaltyCommunity_meter_b";
const BID_RATE: f64 = 0.3;
/// Demand forecaster fixed confidence (see manager.rs `DEMAND_FORECAST_CONFIDENCE`).
const DEMAND_FORECAST_CONFIDENCE: f64 = 0.9;

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
/// night point (same shape as `pv_forecasting_steps::pv_response_body`).
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

/// Find an area of a given name in a market topology.
fn area_by_name<'a>(areas: &'a [AreaTopologySchema], name: &str) -> &'a AreaTopologySchema {
	areas
		.iter()
		.find(|a| a.name == name)
		.unwrap_or_else(|| panic!("area {} present in topology", name))
}

#[when(
	"a PV-penalty community topology with one PV asset and two meters is created for the next \
	 delivery slot"
)]
async fn create_pv_penalty_topology(world: &mut MyWorld) {
	world.target_delivery_time = next_delivery_slot();
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	let topology = ExternalCommunityTopology {
		community_name: PV_PENALTY_COMMUNITY.to_string(),
		areas: vec![
			ExternalAreaTopology { area_type: AssetType::PV, area_name: PV_AREA.to_string() },
			ExternalAreaTopology {
				area_type: AssetType::SMART_METER,
				area_name: METER_A.to_string(),
			},
			ExternalAreaTopology {
				area_type: AssetType::SMART_METER,
				area_name: METER_B.to_string(),
			},
		],
	};
	let market = adapter
		.get_or_create_market_topology(vec![topology], world.target_delivery_time)
		.await
		.into_iter()
		.next()
		.expect("PV-penalty market must be created");

	assert_eq!(
		string_to_h256(market.market_id.clone()),
		world.generate_market_id(PV_PENALTY_COMMUNITY, MarketType::Spot),
		"market id must match the community-aware hash"
	);
	info!("Created PV-penalty market {} for slot {}", market.market_id, world.target_delivery_time);
	world.pv_penalty_market = Some(market);
}

#[when("a single 5 kWh PV production offer forecast and two demand bid forecasts of 3 and 2 kWh are built")]
async fn build_pv_penalty_forecasts(world: &mut MyWorld) {
	let market = world.pv_penalty_market.clone().expect("topology created first");
	let pv_area = area_by_name(&market.community_areas, PV_AREA).clone();
	let cfg = PvCommitmentConfig::from_constants();

	// A degenerate p5 == p95 == pv_forecast band (all 20000 W) yields the maximum confidence, so
	// the offer rate stays at the low floor and undercuts the bids for a deterministic match. The
	// q5-based commitment is pv_avg_watts_to_kwh(20000) = 5.0 kWh.
	let body = pv_response_body(world.target_delivery_time, 20000.0, 20000.0, 20000.0);
	let response = parse_response(&body).expect("PV response must parse");
	let day = ForecastsManager::pv_forecast_schema_from_point(
		&response.data.pv_forecasts[0],
		&pv_area,
		&market.community_uuid,
		&cfg,
	)
	.expect("daytime PV point yields a forecast");
	assert!(day.energy_kwh < 0.0, "a PV production forecast must be negative energy");
	assert!(
		(day.energy_kwh.abs() - 5.0).abs() < 1e-9,
		"the PV offer must commit exactly 5.0 kWh, got {}",
		day.energy_kwh.abs()
	);
	assert_eq!(day.time_slot, world.target_delivery_time, "time_slot aligned to the trade slot");
	world.pv_penalty_offer = Some(day);

	// Two positive-energy demand bid forecasts, 3.0 kWh on meter_a and 2.0 kWh on meter_b, using
	// the fixed demand confidence exactly as the demand branch of the manager emits them.
	let now = Utc::now().timestamp() as u64;
	let mut bids = Vec::new();
	for (area_name, energy) in [(METER_A, 3.0_f64), (METER_B, 2.0_f64)] {
		let meter = area_by_name(&market.community_areas, area_name);
		let bid = ForecastSchema {
			area_uuid: meter.area_uuid.clone(),
			area_hash: meter.area_hash.clone(),
			community_uuid: market.community_uuid.clone(),
			time_slot: world.target_delivery_time,
			creation_time: now,
			energy_kwh: energy,
			confidence: DEMAND_FORECAST_CONFIDENCE,
		};
		assert!(bid.energy_kwh > 0.0, "a demand bid is a positive (consumption) forecast");
		bids.push(bid);
	}
	world.pv_penalty_bids = bids;
	info!("Built one 5.0 kWh PV offer forecast and two demand bids (3.0 + 2.0 kWh)");
}

#[when("the Market Orchestrator opens the PV-penalty Spot market")]
async fn wait_for_pv_penalty_market_open(world: &mut MyWorld) {
	let market_id = world.generate_market_id(PV_PENALTY_COMMUNITY, MarketType::Spot);
	info!("Waiting for the orchestrator to open the PV-penalty market {:?}...", market_id);

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
				info!("PV-penalty market opened on-chain: {:?}", market_id);
				tokio::time::sleep(Duration::from_secs(6)).await;
				return;
			}
		}
	}
	panic!("Timeout: the orchestrator did not open the PV-penalty market {:?}", market_id);
}

#[when("the PV production offer and both demand bids are published")]
async fn publish_pv_penalty_orders(world: &mut MyWorld) {
	let market = world.pv_penalty_market.clone().expect("topology created first");
	let offer = world.pv_penalty_offer.clone().expect("PV offer forecast built");
	let bids = world.pv_penalty_bids.clone();
	assert_eq!(bids.len(), 2, "two demand bids must have been built");
	let seller = world.users.get("bob").unwrap().clone();
	let buyer = world.users.get("charlie").unwrap().clone();
	let slot = world.target_delivery_time;

	// The single PV offer (negative energy) is signed by bob; both demand bids (positive energy)
	// are signed by charlie in ONE call so the account nonce is handled in a single batch.
	// open_time == close_time fully progresses the offer rate ramp to the confidence-lifted floor.
	publish_orders(node_url(), vec![offer], market.clone(), BID_RATE, slot, slot, &seller)
		.await
		.expect("Failed to publish PV production offer");
	publish_orders(node_url(), bids, market, BID_RATE, slot, slot, &buyer)
		.await
		.expect("Failed to publish demand bids");
	info!("Published one PV offer (bob) and two demand bids (charlie)");
}

#[then("two trades settle on the PV asset splitting its production into 3 and 2 kWh")]
async fn capture_pv_penalty_trades(world: &mut MyWorld) {
	let market = world.pv_penalty_market.clone().expect("topology created first");
	let pv_area = area_by_name(&market.community_areas, PV_AREA).clone();
	let pv_area_uuid = string_to_h256(pv_area.area_hash.clone());
	let market_id = world.generate_market_id(PV_PENALTY_COMMUNITY, MarketType::Spot);
	let seller_account: AccountId32 = world.users.get("bob").unwrap().public_key().into();

	let mut captured: Vec<CapturedTrade> = Vec::new();
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		if captured.len() >= 2 {
			break;
		}
		info!("Waiting for the two PV trades... check {}/40 (captured {})", i + 1, captured.len());
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
			// Only trades whose OFFER side is the PV asset (the 5 kWh offer split into 3 + 2).
			if trade.offer.offer_component.area_uuid != pv_area_uuid {
				continue;
			}
			if captured.iter().any(|c| c.trade_uuid == trade.trade_uuid) {
				continue;
			}
			assert_eq!(trade.seller, seller_account, "both PV trades must be sold by bob");
			captured.push(CapturedTrade {
				trade_uuid: trade.trade_uuid,
				selected_energy: trade.parameters.selected_energy,
				creation_time: trade.creation_time,
			});
		}
	}

	assert_eq!(captured.len(), 2, "the 5 kWh PV offer must settle into exactly two trades");
	let mut energies: Vec<u64> = captured.iter().map(|c| c.selected_energy).collect();
	energies.sort_unstable();
	assert_eq!(
		energies,
		vec![20000, 30000],
		"the two PV trades must clear 2.0 and 3.0 kWh (scaled ×10000)"
	);
	info!(
		"Captured two PV trades: uuids {:?}, energies {:?}",
		captured.iter().map(|c| c.trade_uuid).collect::<Vec<_>>(),
		captured.iter().map(|c| c.selected_energy).collect::<Vec<_>>()
	);
	world.pv_penalty_trades = captured;
}

#[when("a PV-asset production measurement of 4 kWh is submitted for the slot")]
async fn submit_pv_penalty_measurement(world: &mut MyWorld) {
	let market = world.pv_penalty_market.clone().expect("topology created first");
	let pv_area = area_by_name(&market.community_areas, PV_AREA).clone();
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	// A single seller-side measurement for the PV area: net -4.0 kWh -> production magnitude 4.0.
	// No meter measurements are submitted, so the buyer side is never penalized.
	let measurement = MeasurementSchema {
		area_uuid: pv_area.area_uuid.clone(),
		area_hash: pv_area.area_hash.clone(),
		community_uuid: market.community_uuid.clone(),
		energy_kwh: -4.0,
		time_slot: world.target_delivery_time,
		creation_time: 1,
	};
	adapter.forward_measurement(vec![measurement]).await.expect("forwarding measurement failed");
	info!("Submitted PV-area production measurement of 4.0 kWh (net -4.0)");
}

#[then("only the later of the two PV trades is penalized for the 1 kWh production shortfall")]
async fn verify_pv_penalty_waterfall(world: &mut MyWorld) {
	let mut trades = world.pv_penalty_trades.clone();
	assert_eq!(trades.len(), 2, "two PV trades must have been captured");

	// Replicate the execution engine's waterfall order: (creation_time asc, then trade_uuid asc).
	trades.sort_by(|a, b| {
		a.creation_time
			.cmp(&b.creation_time)
			.then_with(|| a.trade_uuid.cmp(&b.trade_uuid))
	});
	let first = trades[0].clone(); // earlier commitment -> covered first
	let second = trades[1].clone(); // later commitment -> absorbs the shortfall
	let our_uuids = [first.trade_uuid, second.trade_uuid];
	let seller_account: AccountId32 = world.users.get("bob").unwrap().public_key().into();

	info!("Waiting for execution engine to submit penalties for our two PV trades...");
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	// Collect penalties whose trade_uuid matches EITHER captured trade. We do not early-exit on the
	// first hit: the whole window is scanned so we can prove the earlier trade is never penalized.
	let mut matched: Vec<(H256, AccountId32, u64)> = Vec::new();
	for i in 0..40 {
		info!("Waiting for PenaltiesSubmitted... check {}/40", i + 1);
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block for penalty check")
			.unwrap()
			.unwrap();
		let events = block.events().await.unwrap();
		for e in events
			.find::<gsy_node::trades_settlement::events::PenaltiesSubmitted>()
			.flatten()
		{
			let penalty = e.0;
			if our_uuids.contains(&penalty.trade_uuid)
				&& !matched.iter().any(|(u, _, _)| *u == penalty.trade_uuid)
			{
				matched.push((penalty.trade_uuid, penalty.penalized_account, penalty.penalty_energy));
			}
		}
	}

	// Exactly one of OUR two trades is penalized (we assert nothing about penalties on other slots).
	assert_eq!(
		matched.len(),
		1,
		"exactly one of the two PV trades must be penalized, found {:?}",
		matched
	);
	let (uuid, account, energy) = matched[0].clone();
	assert_eq!(uuid, second.trade_uuid, "the penalty must land on the LATER trade");
	assert_eq!(account, seller_account, "the penalized account must decode to bob (the seller)");
	assert_eq!(
		energy, 1000,
		"the 1.0 kWh tail shortfall at penalty_rate 0.10 must yield penalty_energy 1000"
	);
	assert!(
		!matched.iter().any(|(u, _, _)| *u == first.trade_uuid),
		"the earlier trade must NOT be penalized (production covers it in full)"
	);
	info!(
		"Verified waterfall penalty: later trade {:?} penalized {} (bob); earlier trade {:?} unpenalized",
		second.trade_uuid, energy, first.trade_uuid
	);
}

/// The certificate endpoint's end-to-end contract: a certificate is issued for the trade that
/// survived validation and for nothing else. Exercises the whole chain on the docker stack —
/// the negative production measurement reaching storage, the `Executed`/`Penalized` split, and
/// the requirement that a certificate carry seller-side evidence.
#[then("the offchain storage issues a certificate for the executed PV trade only")]
async fn verify_pv_penalty_certificates(world: &mut MyWorld) {
	let mut trades = world.pv_penalty_trades.clone();
	assert_eq!(trades.len(), 2, "two PV trades must have been captured");
	// Same waterfall order as the penalty assertion: earlier commitment is honored in full.
	trades.sort_by(|a, b| {
		a.creation_time
			.cmp(&b.creation_time)
			.then_with(|| a.trade_uuid.cmp(&b.trade_uuid))
	});
	let executed = h256_to_string(trades[0].trade_uuid);
	let penalized = h256_to_string(trades[1].trade_uuid);
	// Which of the two trades clears 3.0 and which 2.0 kWh is not pinned by the settlement
	// (the shortfall is 1.0 kWh either way), so the expected quantity comes from the trade
	// itself rather than a literal. `selected_energy` is scaled ×10000 on-chain.
	let expected_energy = trades[0].selected_energy as f64 / 10000.0;

	let slot = world.target_delivery_time;
	let url = format!(
		"{}/guarantees-of-origin-measurements?start_time={}&end_time={}",
		orderbook_url(),
		slot,
		slot
	);

	// Both trade statuses have already converged by this point, so the certificate is derivable
	// at once; poll anyway to absorb event-listener lag on a loaded stack.
	let mut records: Vec<Value> = Vec::new();
	for i in 0..20 {
		let resp = world.http_client.get(&url).send().await.expect("GET certificates failed");
		assert!(
			resp.status().is_success(),
			"GET /guarantees-of-origin-measurements returned {}",
			resp.status()
		);
		records = resp.json().await.expect("deserialize certificates response");
		if !records.is_empty() {
			break;
		}
		info!("Waiting for the certificate to become derivable... check {}/20", i + 1);
		tokio::time::sleep(Duration::from_secs(5)).await;
	}

	assert_eq!(
		records.len(),
		1,
		"exactly one certificate for the slot: the penalized trade earns none, got {:?}",
		records
	);
	let record = &records[0];
	assert_eq!(
		record["trade_and_delivery"]["trade_reference"][0].as_str(),
		Some(executed.as_str()),
		"the certificate must reference the executed trade"
	);
	assert_ne!(
		record["trade_and_delivery"]["trade_reference"][0].as_str(),
		Some(penalized.as_str()),
		"the penalized trade must never earn a certificate"
	);
	assert_eq!(
		record["time_and_quantity"]["energy_quantity"].as_f64(),
		Some(expected_energy),
		"the certified quantity is the traded quantity"
	);
	assert_eq!(record["identity"]["record_type"].as_str(), Some("local_origin_record"));
	assert_eq!(record["production_asset"]["production_asset_id"].as_str(), Some(PV_AREA));
	assert_eq!(record["production_asset"]["asset_class"].as_str(), Some("PV"));
	assert_eq!(
		record["trade_and_delivery"]["trade_status_at_issuance"].as_str(),
		Some("delivery_verified")
	);
	// The PV meter nets negative, which is what `validate_measurement` had to stop rejecting.
	assert_eq!(record["measurement_provenance"]["flow_direction"].as_str(), Some("export"));
	assert_eq!(record["time_and_quantity"]["source_slot_timestamp"].as_u64(), Some(slot));
	info!(
		"Verified one certificate for executed trade {} ({} kWh); none for penalized {}",
		executed, expected_energy, penalized
	);
}

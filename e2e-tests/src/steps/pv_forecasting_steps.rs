//! End-to-end steps for the PV-production and demand forecasting flows.
//!
//! These scenarios drive the REAL gsy-community-client production seams — the PV
//! response parser (`pv_api::parse_response`), the percentile-based commitment /
//! confidence mapping (`ForecastsManager::pv_forecast_schema_from_point` +
//! `pv_pricing`), forecast validation/forwarding (`AreaMarketInfoAdapter`), order
//! creation/pricing (`publish_orders`, which computes the confidence-lifted offer
//! rate floor internally), and the inter-community net aggregation
//! (`aggregate_net_import` + `create_inter_community_order`). Inputs are constructed
//! programmatically (no live FEDECOM endpoints), mirroring the other e2e scenarios.

use crate::world::{gsy_node, InterCommunityParticipant, MyWorld};
use gsy_community_client::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::InputOrder as NodeInputOrder;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use cucumber::{then, when};
use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_forecasts::pv_api::{parse_response, pv_avg_watts_to_kwh};
use gsy_community_client::external_forecasts::pv_pricing::{
	commitment_from_point, effective_offer_min_rate, PvCommitmentConfig,
};
use gsy_community_client::inter_community::{eligible_inter_community, inter_community_market_id};
use gsy_community_client::node_connector::orders::{create_inter_community_order, publish_orders};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::aggregation::aggregate_net_import;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{community_id_from_uuid, string_to_h256};
use gsy_offchain_primitives::{constants::GlobalConstants, MarketType};
use std::time::Duration;
use tracing::info;

const PV_DEMAND_COMMUNITY: &str = "PvDemandCommunity";
const PV_AREA: &str = "PvDemandCommunity_pv";
const METER_AREA: &str = "PvDemandCommunity_meter";
const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;
/// Demand forecaster fixed confidence (see manager.rs `DEMAND_FORECAST_CONFIDENCE`).
const DEMAND_FORECAST_CONFIDENCE: f64 = 0.9;
/// kWh delivered by the metered load in the single-community scenario.
const DEMAND_ENERGY_KWH: f64 = 5.0;

fn orderbook_url() -> String {
	std::env::var("OFFCHAIN_STORAGE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn node_url() -> String {
	std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string())
}

/// A slot two hours out, aligned to the 15-minute market cadence (matches how every
/// other scenario picks its delivery slot).
fn next_delivery_slot() -> u64 {
	let now = Utc::now();
	((now + ChronoDuration::hours(2)).timestamp() as u64 / GlobalConstants.TIME_SLOT_SEC)
		* GlobalConstants.TIME_SLOT_SEC
}

/// Build a realistic PV-forecaster response body with one daytime point aligned to
/// `slot` and one all-zero night point. The daytime `pv_forecast`/`p5`/`p95` are
/// average-power watts (2-element percentile arrays, per the real endpoint).
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

/// GET forecasts back from off-chain storage for one area at one slot.
async fn get_stored_forecasts(
	world: &MyWorld,
	area_uuid: &str,
	slot: u64,
) -> Vec<ForecastSchema> {
	let url = format!(
		"{}/forecasts?area_uuid={}&start_time={}&end_time={}",
		orderbook_url(),
		area_uuid,
		slot,
		slot
	);
	let resp = world
		.http_client
		.get(url)
		.send()
		.await
		.expect("GET /forecasts failed");
	assert!(resp.status().is_success(), "GET /forecasts returned {}", resp.status());
	resp.json::<Vec<ForecastSchema>>()
		.await
		.expect("deserialize forecasts response")
}

// ----------------------------------------------------------------------------------
// Scenario A — single community
// ----------------------------------------------------------------------------------

#[when("a PV-and-demand community topology is created for the next delivery slot")]
async fn create_pv_demand_topology(world: &mut MyWorld) {
	world.target_delivery_time = next_delivery_slot();
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	let topology = ExternalCommunityTopology {
		community_name: PV_DEMAND_COMMUNITY.to_string(),
		areas: vec![
			ExternalAreaTopology { area_type: AssetType::PV, area_name: PV_AREA.to_string() },
			ExternalAreaTopology {
				area_type: AssetType::SMART_METER,
				area_name: METER_AREA.to_string(),
			},
		],
	};
	let market = adapter
		.get_or_create_market_topology(vec![topology], world.target_delivery_time)
		.await
		.into_iter()
		.next()
		.expect("PV-and-demand market must be created");

	assert_eq!(
		string_to_h256(market.market_id.clone()),
		world.generate_market_id(PV_DEMAND_COMMUNITY, MarketType::Spot),
		"market id must match the community-aware hash"
	);
	info!("Created PV-and-demand market {} for slot {}", market.market_id, world.target_delivery_time);
	world.pv_market = Some(market);
}

#[when("a PV forecaster response is ingested into a production offer forecast")]
async fn ingest_pv_forecast(world: &mut MyWorld) {
	let market = world.pv_market.clone().expect("topology created first");
	let pv_area = area_by_name(&market.community_areas, PV_AREA).clone();
	let cfg = PvCommitmentConfig::from_constants();

	// A wide p5..p95 band (2.0..4.0 kWh around a 3.0 kWh point) => low confidence, which
	// lifts the offer rate floor later.
	let body = pv_response_body(world.target_delivery_time, 12000.0, 8000.0, 16000.0);
	let response = parse_response(&body).expect("PV response must parse");
	let points = &response.data.pv_forecasts;
	assert_eq!(points.len(), 2, "one daytime and one night point");

	// FLOW 1: the daytime point maps to a production offer forecast.
	let day = ForecastsManager::pv_forecast_schema_from_point(
		&points[0],
		&pv_area,
		&market.community_uuid,
		&cfg,
	)
	.expect("daytime PV point yields a forecast");

	// Negative energy marks a production offer; magnitude equals the q5-based (risk
	// aversion 1.0) commitment: q5 = 8000 W => 2.0 kWh over the 15-min slot.
	let expected_commitment = commitment_from_point(&points[0], &cfg);
	assert!(day.energy_kwh < 0.0, "PV production forecast must be negative energy");
	assert!(
		(day.energy_kwh + expected_commitment.energy_kwh).abs() < 1e-9,
		"committed energy must equal the q5 commitment"
	);
	assert!(
		(day.energy_kwh.abs() - pv_avg_watts_to_kwh(8000.0)).abs() < 1e-9,
		"committed energy must equal q5 watts converted to kWh (2.0 kWh)"
	);
	// Real per-slot confidence, strictly inside (PV_MIN_CONFIDENCE, 1.0) for a wide band.
	assert!(
		(day.confidence - expected_commitment.confidence).abs() < 1e-12,
		"schema confidence must equal the computed confidence"
	);
	assert!(
		day.confidence > CommunityClientConstants.PV_MIN_CONFIDENCE
			&& day.confidence < 1.0,
		"a wide band yields confidence strictly between the floor and 1.0, got {}",
		day.confidence
	);
	assert_eq!(day.time_slot, world.target_delivery_time, "time_slot aligned to the trade slot");

	// Night / all-zero point commits no energy => no forecast, no order.
	assert!(
		ForecastsManager::pv_forecast_schema_from_point(
			&points[1],
			&pv_area,
			&market.community_uuid,
			&cfg
		)
		.is_none(),
		"an all-zero night point must not yield a forecast"
	);

	info!(
		"Ingested PV offer forecast: energy {} kWh, confidence {}",
		day.energy_kwh, day.confidence
	);
	world.pv_offer_confidence = day.confidence;
	world.pv_offer_forecast = Some(day);
}

#[when("a demand forecast is constructed for the consumption meter")]
async fn construct_demand_forecast(world: &mut MyWorld) {
	// FLOW 2: demand forecasts are positive energy with the fixed demand confidence,
	// exactly as the demand branch of the manager emits them.
	let market = world.pv_market.clone().expect("topology created first");
	let meter = area_by_name(&market.community_areas, METER_AREA);
	let demand = ForecastSchema {
		area_uuid: meter.area_uuid.clone(),
		area_hash: meter.area_hash.clone(),
		community_uuid: market.community_uuid.clone(),
		time_slot: world.target_delivery_time,
		creation_time: Utc::now().timestamp() as u64,
		energy_kwh: DEMAND_ENERGY_KWH,
		confidence: DEMAND_FORECAST_CONFIDENCE,
	};
	assert!(demand.energy_kwh > 0.0, "demand forecast is a consumption (positive) forecast");
	world.demand_bid_forecast = Some(demand);
}

#[when("the PV and demand forecasts are validated and forwarded to offchain storage")]
async fn validate_and_forward(world: &mut MyWorld) {
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));
	let pv = world.pv_offer_forecast.clone().expect("PV forecast ingested");
	let demand = world.demand_bid_forecast.clone().expect("demand forecast built");
	let now = Utc::now().timestamp() as u64;

	// FLOW 3: validation now accepts negative (production) energy for a future slot, and
	// still accepts positive demand. Zero energy and past slots are rejected.
	assert!(adapter.validate_forecast(&pv, now), "negative future PV forecast must validate");
	assert!(adapter.validate_forecast(&demand, now), "positive future demand forecast must validate");

	let mut zero = pv.clone();
	zero.energy_kwh = 0.0;
	assert!(!adapter.validate_forecast(&zero, now), "zero-energy forecast must be rejected");

	let mut past = demand.clone();
	past.time_slot = now.saturating_sub(GlobalConstants.TIME_SLOT_SEC);
	assert!(!adapter.validate_forecast(&past, now), "past-slot forecast must be rejected");

	adapter
		.forward_forecast(vec![pv.clone(), demand.clone()])
		.await
		.expect("forwarding forecasts failed");

	// Read both back from storage to confirm they landed with the correct sign.
	let stored_pv = get_stored_forecasts(world, &pv.area_uuid, world.target_delivery_time).await;
	assert!(
		stored_pv.iter().any(|f| f.energy_kwh < 0.0 && (f.energy_kwh - pv.energy_kwh).abs() < 1e-9),
		"the negative PV production forecast must be retrievable from storage"
	);
	let stored_demand =
		get_stored_forecasts(world, &demand.area_uuid, world.target_delivery_time).await;
	assert!(
		stored_demand
			.iter()
			.any(|f| f.energy_kwh > 0.0 && (f.energy_kwh - demand.energy_kwh).abs() < 1e-9),
		"the positive demand forecast must be retrievable from storage"
	);
	info!("Validated and forwarded PV + demand forecasts; both round-trip through storage");
}

#[when("the Market Orchestrator opens the PV-and-demand Spot market")]
async fn wait_for_pv_market_open(world: &mut MyWorld) {
	let market_id = world.generate_market_id(PV_DEMAND_COMMUNITY, MarketType::Spot);
	info!("Waiting for the orchestrator to open the PV-and-demand market {:?}...", market_id);

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
				info!("PV-and-demand market opened on-chain: {:?}", market_id);
				tokio::time::sleep(Duration::from_secs(6)).await;
				return;
			}
		}
	}
	panic!("Timeout: the orchestrator did not open the PV-and-demand market {:?}", market_id);
}

#[when("the PV production offer and the demand bid are published")]
async fn publish_pv_and_demand(world: &mut MyWorld) {
	let market = world.pv_market.clone().expect("topology created first");
	let pv = world.pv_offer_forecast.clone().expect("PV forecast ingested");
	let demand = world.demand_bid_forecast.clone().expect("demand forecast built");
	let seller = world.users.get("bob").unwrap().clone();
	let buyer = world.users.get("charlie").unwrap().clone();
	let slot = world.target_delivery_time;

	// FLOW 4: the PV forecast (negative energy) becomes an Offer signed by bob; the demand
	// forecast (positive energy) becomes a Bid signed by charlie. open_time == close_time
	// fully progresses the offer rate ramp so it resolves deterministically to the
	// confidence-lifted floor `effective_offer_min_rate`; the bid uses the flat bid rate.
	publish_orders(node_url(), vec![pv], market.clone(), BID_RATE, slot, slot, &seller)
		.await
		.expect("Failed to publish PV production offer");
	publish_orders(node_url(), vec![demand], market, BID_RATE, slot, slot, &buyer)
		.await
		.expect("Failed to publish demand bid");
	info!("Published PV offer (bob) and demand bid (charlie)");
}

#[then(
	"the PV forecast is stored as an offer with a confidence-lifted rate floor and the demand \
	 forecast as a flat-rate bid"
)]
async fn verify_offer_and_bid(world: &mut MyWorld) {
	let market = world.pv_market.clone().expect("topology created first");
	let pv = world.pv_offer_forecast.clone().expect("PV forecast ingested");
	let demand = world.demand_bid_forecast.clone().expect("demand forecast built");
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	// Match the ORIGINAL offer/bid by area and committed energy: a partial match on the bid
	// spawns a residual bid for the same area, which must not be mistaken for the original.
	let committed_pv = pv.energy_kwh.abs();
	let is_pv_offer = |o: &DbOrderSchema| {
		matches!(&o.order, Order::Offer(off)
			if off.offer_component.area_uuid == pv.area_hash
			&& (off.offer_component.energy - committed_pv).abs() < 1e-6)
	};
	let is_demand_bid = |o: &DbOrderSchema| {
		matches!(&o.order, Order::Bid(bid)
			if bid.bid_component.area_uuid == demand.area_hash
			&& (bid.bid_component.energy - demand.energy_kwh).abs() < 1e-6)
	};

	// Allow storage to catch up with the on-chain inserts.
	let mut orders: Vec<DbOrderSchema> = Vec::new();
	for _ in 0..30 {
		orders = adapter.get_orders_for_market(&market.market_id).await;
		if orders.iter().any(is_pv_offer) && orders.iter().any(is_demand_bid) {
			break;
		}
		tokio::time::sleep(Duration::from_secs(2)).await;
	}

	// The PV forecast produced an Offer with the committed (q5) energy.
	let offer = orders
		.iter()
		.find_map(|o| match &o.order {
			Order::Offer(off)
				if off.offer_component.area_uuid == pv.area_hash
					&& (off.offer_component.energy - committed_pv).abs() < 1e-6 =>
			{
				Some(off)
			}
			_ => None,
		})
		.expect("PV forecast must be stored as an Offer with the committed energy");
	// energy_rate is the TOTAL price (energy * per-kWh rate), so the per-kWh rate is the
	// quotient. It must equal the confidence-lifted floor and sit strictly above the flat
	// MIN_ORDER_RATE (proving the low-confidence offer's floor was raised).
	let offer_per_kwh = offer.offer_component.energy_rate / offer.offer_component.energy;
	let expected_floor = effective_offer_min_rate(
		CommunityClientConstants.MIN_ORDER_RATE,
		CommunityClientConstants.MAX_ORDER_RATE,
		world.pv_offer_confidence,
		CommunityClientConstants.PV_PRICE_CONFIDENCE_WEIGHT,
	);
	assert!(
		expected_floor > CommunityClientConstants.MIN_ORDER_RATE + 1e-9,
		"a low-confidence PV offer must lift the floor above MIN_ORDER_RATE"
	);
	assert!(
		(offer_per_kwh - expected_floor).abs() < 1e-3,
		"offer per-kWh rate {} must equal the confidence-lifted floor {}",
		offer_per_kwh,
		expected_floor
	);

	// The demand forecast produced a Bid at the flat bid rate.
	let bid = orders
		.iter()
		.find_map(|o| match &o.order {
			Order::Bid(bid)
				if bid.bid_component.area_uuid == demand.area_hash
					&& (bid.bid_component.energy - demand.energy_kwh).abs() < 1e-6 =>
			{
				Some(bid)
			}
			_ => None,
		})
		.expect("demand forecast must be stored as a Bid with the forecast energy");
	let bid_per_kwh = bid.bid_component.energy_rate / bid.bid_component.energy;
	assert!(
		(bid_per_kwh - BID_RATE).abs() < 1e-3,
		"bid per-kWh rate {} must equal the flat bid rate {}",
		bid_per_kwh,
		BID_RATE
	);
	info!(
		"Verified PV Offer (energy {}, rate {}/kWh floor) and demand Bid (energy {}, rate {}/kWh)",
		offer.offer_component.energy, offer_per_kwh, bid.bid_component.energy, bid_per_kwh
	);
}

#[then("a trade settles between the PV offer and the demand bid on-chain")]
async fn verify_pv_demand_trade(world: &mut MyWorld) {
	// FLOW 5: the PV offer (2.0 kWh, floor rate) undercuts the demand bid (5.0 kWh, 0.3),
	// so a 2.0 kWh trade settles in the community market.
	let market_id = world.generate_market_id(PV_DEMAND_COMMUNITY, MarketType::Spot);
	let seller = world.users.get("bob").unwrap().public_key();
	let buyer = world.users.get("charlie").unwrap().public_key();
	let seller_account: subxt::utils::AccountId32 = seller.into();
	let buyer_account: subxt::utils::AccountId32 = buyer.into();

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		info!("Waiting for the PV/demand trade... check {}/40", i + 1);
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();
		let events = block.events().await.unwrap();
		for e in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>().flatten() {
			let trade = e.0;
			if trade.market_id == market_id {
				assert_eq!(trade.seller, seller_account, "PV producer bob is the seller");
				assert_eq!(trade.buyer, buyer_account, "the metered load charlie is the buyer");
				// The trade clears the committed PV quantity (2.0 kWh = 20000 scaled).
				assert_eq!(
					trade.parameters.selected_energy, 20000,
					"the trade must clear the committed 2.0 kWh PV quantity"
				);
				info!(
					"PV/demand trade settled in {:?}: {} energy units",
					trade.market_id, trade.parameters.selected_energy
				);
				return;
			}
		}
	}
	panic!("Timeout: no PV/demand trade settled in {:?} within 40 blocks", market_id);
}

// ----------------------------------------------------------------------------------
// Scenario B — multiple communities, PV production included in the net
// ----------------------------------------------------------------------------------

/// (community, demand kWh, PV point watts, PV p5 watts, PV p95 watts).
/// PV commitment == q5 (risk aversion 1.0): p5 watts -> kWh.
///   Lugaggia: demand 10 + PV -2 = +8 kWh  -> net deficit  -> Bid
///   Garamè:   demand 3  + PV -10 = -7 kWh  -> net surplus  -> Offer
fn inter_community_specs() -> Vec<(&'static str, f64, f64, f64, f64)> {
	vec![
		("LugaggiaInnovationCommunity", 10.0, 12000.0, 8000.0, 16000.0),
		("GaramèDistrict", 3.0, 60000.0, 40000.0, 80000.0),
	]
}

#[when("two eligible communities each ingest PV and demand forecasts that net to a bid and an offer")]
async fn build_inter_community_pv_demand(world: &mut MyWorld) {
	world.target_delivery_time = next_delivery_slot();
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));
	let cfg = PvCommitmentConfig::from_constants();

	// Create a PV + SMART_METER topology per eligible community.
	let topologies: Vec<ExternalCommunityTopology> = inter_community_specs()
		.iter()
		.map(|(name, ..)| ExternalCommunityTopology {
			community_name: name.to_string(),
			areas: vec![
				ExternalAreaTopology {
					area_type: AssetType::PV,
					area_name: format!("{}_pv", name),
				},
				ExternalAreaTopology {
					area_type: AssetType::SMART_METER,
					area_name: format!("{}_meter", name),
				},
			],
		})
		.collect();
	let markets = adapter
		.get_or_create_market_topology(topologies, world.target_delivery_time)
		.await;
	assert_eq!(markets.len(), 2, "one market per community");

	world.inter_communities.clear();
	for (name, demand_kwh, pv_watts, p5_watts, p95_watts) in inter_community_specs() {
		assert!(eligible_inter_community(name), "{} must be inter-community eligible", name);
		let market = markets
			.iter()
			.find(|m| m.community_name == name)
			.unwrap_or_else(|| panic!("market for {} created", name))
			.clone();
		let pv_area = area_by_name(&market.community_areas, &format!("{}_pv", name)).clone();
		let meter_area = area_by_name(&market.community_areas, &format!("{}_meter", name)).clone();

		// PV production forecast built from the REAL parse + commitment mapping (negative).
		let body = pv_response_body(world.target_delivery_time, pv_watts, p5_watts, p95_watts);
		let response = parse_response(&body).expect("PV response parses");
		let pv_forecast = ForecastsManager::pv_forecast_schema_from_point(
			&response.data.pv_forecasts[0],
			&pv_area,
			&market.community_uuid,
			&cfg,
		)
		.expect("daytime PV point yields a forecast");
		assert!(pv_forecast.energy_kwh < 0.0, "PV production must be negative energy");

		// Demand (consumption) forecast, positive.
		let demand_forecast = ForecastSchema {
			area_uuid: meter_area.area_uuid.clone(),
			area_hash: meter_area.area_hash.clone(),
			community_uuid: market.community_uuid.clone(),
			time_slot: world.target_delivery_time,
			creation_time: Utc::now().timestamp() as u64,
			energy_kwh: demand_kwh,
			confidence: DEMAND_FORECAST_CONFIDENCE,
		};

		let forecasts = vec![demand_forecast, pv_forecast];
		adapter
			.forward_forecast(forecasts.clone())
			.await
			.expect("forwarding forecasts failed");

		let net_kwh =
			aggregate_net_import(&forecasts, &market.community_uuid, world.target_delivery_time);
		info!("Community '{}' net import (incl. PV) = {} kWh", name, net_kwh);

		world.inter_communities.push(InterCommunityParticipant {
			name: name.to_string(),
			community_uuid: market.community_uuid.clone(),
			community_id: community_id_from_uuid(&market.community_uuid),
			spot_market_id: world.generate_market_id(name, MarketType::Spot),
			forecasts,
			net_kwh,
		});
	}
	assert_eq!(world.inter_communities.len(), 2, "two participants");
}

#[when("the inter-community PV-and-demand forecasts are forwarded and read back from offchain storage")]
async fn read_back_inter_community_forecasts(world: &mut MyWorld) {
	// FLOW 3 (multi-community): confirm both the PV (negative) and demand (positive) legs
	// of every community round-trip through storage.
	for community in world.inter_communities.clone() {
		for forecast in &community.forecasts {
			let stored =
				get_stored_forecasts(world, &forecast.area_uuid, world.target_delivery_time).await;
			assert!(
				stored.iter().any(|f| (f.energy_kwh - forecast.energy_kwh).abs() < 1e-9),
				"forecast for area {} of {} must be retrievable from storage",
				forecast.area_uuid,
				community.name
			);
		}
	}
	info!("All inter-community PV + demand forecasts round-trip through storage");
}

#[then("each community's aggregated net import reflects its PV production and demand")]
async fn verify_inter_community_nets(world: &mut MyWorld) {
	let lugaggia = world
		.inter_communities
		.iter()
		.find(|c| c.name == "LugaggiaInnovationCommunity")
		.expect("Lugaggia present");
	let garame = world
		.inter_communities
		.iter()
		.find(|c| c.name == "GaramèDistrict")
		.expect("Garamè present");

	// PV commitment == q5: 8000 W -> 2.0 kWh, 40000 W -> 10.0 kWh.
	// Lugaggia: 10 - 2 = +8 (deficit / Bid). Garamè: 3 - 10 = -7 (surplus / Offer).
	assert!(
		(lugaggia.net_kwh - 8.0).abs() < 1e-9,
		"Lugaggia net (demand 10 + PV -2) must be +8 kWh, got {}",
		lugaggia.net_kwh
	);
	assert!(lugaggia.net_kwh > 0.0, "Lugaggia must net to a deficit (Bid)");
	assert!(
		(garame.net_kwh + 7.0).abs() < 1e-9,
		"Garamè net (demand 3 + PV -10) must be -7 kWh, got {}",
		garame.net_kwh
	);
	assert!(garame.net_kwh < 0.0, "Garamè must net to a surplus (Offer)");
	info!(
		"Verified per-community nets include PV: Lugaggia {} kWh, Garamè {} kWh",
		lugaggia.net_kwh, garame.net_kwh
	);
}

#[then("the aggregated inter-community orders reflect the per-community nets")]
async fn verify_aggregated_orders(world: &mut MyWorld) {
	// Build the aggregated inter-community order for each community with the SAME production
	// seam the inter-community scenario uses, and assert its side / area / energy reflect the
	// PV-inclusive net. (On-chain publication + matching of the shared inter-community market
	// is exercised by inter_community_market.feature; it is not duplicated here to avoid
	// colliding on the reserved per-slot market.)
	let reserved_id = inter_community_market_id(world.target_delivery_time);
	let signer = world.users.get("charlie").unwrap().clone();

	for community in world.inter_communities.clone() {
		let rate = if community.net_kwh > 0.0 { BID_RATE } else { OFFER_RATE };
		let order = create_inter_community_order(
			community.net_kwh,
			community.community_id,
			reserved_id,
			world.target_delivery_time,
			rate,
			&signer,
		)
		.expect("a non-zero net yields exactly one aggregated order");

		let expected_energy = (community.net_kwh.abs() * 10000.0) as u64;
		match order {
			NodeInputOrder::Bid(bid) => {
				assert!(community.net_kwh > 0.0, "{} nets to a deficit -> Bid", community.name);
				assert_eq!(
					bid.bid_component.area_uuid, community.community_id,
					"the aggregated bid carries the community_id as area_uuid"
				);
				assert_eq!(
					bid.bid_component.energy, expected_energy,
					"the aggregated bid energy must equal |net| for {}",
					community.name
				);
			}
			NodeInputOrder::Offer(offer) => {
				assert!(community.net_kwh < 0.0, "{} nets to a surplus -> Offer", community.name);
				assert_eq!(
					offer.offer_component.area_uuid, community.community_id,
					"the aggregated offer carries the community_id as area_uuid"
				);
				assert_eq!(
					offer.offer_component.energy, expected_energy,
					"the aggregated offer energy must equal |net| for {}",
					community.name
				);
			}
		}
		info!(
			"Aggregated {} for '{}' reflects net {} kWh",
			if community.net_kwh > 0.0 { "Bid" } else { "Offer" },
			community.name,
			community.net_kwh
		);
	}
}

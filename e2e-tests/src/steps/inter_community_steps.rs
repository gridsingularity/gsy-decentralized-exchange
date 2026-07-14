use crate::world::{gsy_node, InterCommunityParticipant, MyWorld};
use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{then, when};
use gsy_community_client::constants::INTER_COMMUNITY_MARKET_NAME;
use gsy_community_client::inter_community::{eligible_inter_community, inter_community_market_id};
use gsy_community_client::node_connector::orders::{
	create_inter_community_order, publish_input_orders,
};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::aggregation::aggregate_net_import;
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string, string_to_h256};
use gsy_offchain_primitives::{constants::GlobalConstants, MarketType};
use std::time::Duration;
use tracing::info;

const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

fn orderbook_url() -> String {
	std::env::var("OFFCHAIN_STORAGE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn node_url() -> String {
	std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string())
}

/// The `area_uuid` (a community hash) carried by an aggregated inter-community order.
fn order_area_uuid(order: &DbOrderSchema) -> &str {
	match &order.order {
		Order::Bid(bid) => &bid.bid_component.area_uuid,
		Order::Offer(offer) => &offer.offer_component.area_uuid,
	}
}

/// The `market_id` an aggregated inter-community order was posted into.
fn order_market_id(order: &DbOrderSchema) -> &str {
	match &order.order {
		Order::Bid(bid) => &bid.bid_component.market_id,
		Order::Offer(offer) => &offer.offer_component.market_id,
	}
}

#[when("the inter-community market is created for the next delivery slot")]
async fn create_inter_community_market(world: &mut MyWorld) {
	let now = Utc::now();
	world.target_delivery_time = ((now + ChronoDuration::hours(2)).timestamp() as u64
		/ GlobalConstants.TIME_SLOT_SEC)
		* GlobalConstants.TIME_SLOT_SEC;

	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	// T3: a single idempotent upsert of the shared inter-community market document,
	// performed once for the slot (outside any per-community loop).
	let market = adapter
		.get_or_create_inter_community_market(world.target_delivery_time)
		.await
		.expect("the inter-community market must be created");

	let reserved_id = inter_community_market_id(world.target_delivery_time);
	assert_eq!(
		string_to_h256(market.market_id.clone()),
		reserved_id,
		"the created market must carry the reserved inter-community market id"
	);
	// The reserved id is the generic hash of the reserved name + Spot + slot.
	assert_eq!(
		reserved_id,
		world.generate_market_id(INTER_COMMUNITY_MARKET_NAME, MarketType::Spot),
		"reserved id must equal generate_market_id(INTER_COMMUNITY, Spot, slot)"
	);
	assert_eq!(
		market.community_name, INTER_COMMUNITY_MARKET_NAME,
		"the inter-community market carries the reserved community name"
	);

	info!(
		"Created the shared inter-community market {} for slot {}",
		market.market_id, world.target_delivery_time
	);
	world.inter_community_market = Some(market);
}

#[when("two eligible communities submit forecasts that net to a bid and an offer")]
async fn submit_inter_community_forecasts(world: &mut MyWorld) {
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	// Two DISTINCT eligible communities. Each carries a mix of consumption (+) and
	// production (−) per-asset forecasts; we build them explicitly here rather than
	// relying on the community-client's production-forecast intake, which is a
	// deliberate placeholder (net would otherwise degenerate to consumption-only).
	//   - Lugaggia:  10 − 3 = +7 kWh  → net deficit → Bid
	//   - Garamè:     4 − 10 = −6 kWh → net surplus → Offer
	let specs: Vec<(&str, Vec<f64>)> = vec![
		("LugaggiaInnovationCommunity", vec![10.0, -3.0]),
		("GaramèDistrict", vec![4.0, -10.0]),
	];

	world.inter_communities.clear();
	for (name, energies) in specs {
		assert!(
			eligible_inter_community(name),
			"community {} must be on the inter-community allow-list",
			name
		);

		// A stable per-community uuid. Its hash is the community_id used both as the
		// order's area_uuid and (via the measurements' community_uuid) as the settlement
		// aggregation key.
		let community_uuid = format!("{}-inter-community-uuid", name);
		let community_id = community_id_from_uuid(&community_uuid);
		let spot_market_id = world.generate_market_id(name, MarketType::Spot);

		let forecasts: Vec<ForecastSchema> = energies
			.iter()
			.enumerate()
			.map(|(i, energy)| ForecastSchema {
				area_uuid: format!("{}_asset_{}", name, i),
				area_hash: format!("{}_asset_hash_{}", name, i),
				community_uuid: community_uuid.clone(),
				time_slot: world.target_delivery_time,
				creation_time: 1,
				energy_kwh: *energy,
				confidence: 1.0,
			})
			.collect();

		adapter
			.forward_forecast(forecasts.clone())
			.await
			.expect("Forecast forwarding failed.");

		let net_kwh = aggregate_net_import(&forecasts, &community_uuid, world.target_delivery_time);
		info!(
			"Community '{}' aggregated net import = {} kWh (community_id {:?})",
			name, net_kwh, community_id
		);

		world.inter_communities.push(InterCommunityParticipant {
			name: name.to_string(),
			community_uuid,
			community_id,
			spot_market_id,
			forecasts,
			net_kwh,
		});
	}

	assert_eq!(world.inter_communities.len(), 2, "expected exactly two participants");
	// Distinct community_ids ⇒ the area_uuid self-trade guard is satisfied and the two
	// aggregated orders can match each other.
	assert_ne!(
		world.inter_communities[0].community_id, world.inter_communities[1].community_id,
		"the two communities must have distinct community_ids"
	);
	// One nets to a Bid, the other to an Offer.
	assert!(
		world.inter_communities[0].net_kwh > 0.0,
		"first community must net to a deficit (Bid)"
	);
	assert!(
		world.inter_communities[1].net_kwh < 0.0,
		"second community must net to a surplus (Offer)"
	);
}

#[when("the Market Orchestrator opens the inter-community market")]
async fn wait_for_inter_community_market_to_open(world: &mut MyWorld) {
	let market_id = inter_community_market_id(world.target_delivery_time);
	info!("Waiting for the orchestrator to open the inter-community market {:?}...", market_id);

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..60 {
		info!("Waiting for MarketStatusUpdated for the inter-community market... check {}/60", i + 1);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block from node")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::MarketStatusUpdated>() {
			if let Ok(e) = ev {
				if e.0 == market_id && e.1 {
					info!("Inter-community market opened on-chain: {:?}", market_id);
					// Let the open status propagate before inserting orders.
					tokio::time::sleep(Duration::from_secs(6)).await;
					return;
				}
			}
		}
	}
	panic!("Timeout: the orchestrator did not open the inter-community market {:?}", market_id);
}

#[when("the aggregated inter-community orders are published")]
async fn publish_aggregated_inter_community_orders(world: &mut MyWorld) {
	let market_id = inter_community_market_id(world.target_delivery_time);

	// Design decision #3: the SAME shared account may sit on both sides of the
	// inter-community market — only the per-community area_uuid (community_id)
	// differentiates the orders and prevents a self-match. We therefore sign both the
	// Bid and the Offer with a single shared signer ("charlie", pre-funded/registered).
	let signer = world.users.get("charlie").unwrap().clone();

	for community in world.inter_communities.clone() {
		let rate = if community.net_kwh > 0.0 { BID_RATE } else { OFFER_RATE };
		let order = create_inter_community_order(
			community.net_kwh,
			community.community_id,
			market_id,
			world.target_delivery_time,
			rate,
			&signer,
		)
		.expect("an eligible community with non-zero net must yield exactly one order");

		publish_input_orders(node_url(), vec![order], &signer)
			.await
			.expect("Failed to publish the aggregated inter-community order");

		info!(
			"Published aggregated {} for community '{}' (net {} kWh)",
			if community.net_kwh > 0.0 { "bid" } else { "offer" },
			community.name,
			community.net_kwh
		);
	}
}

#[when("measurements for the inter-community community assets are submitted")]
async fn submit_inter_community_measurements(world: &mut MyWorld) {
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	for community in world.inter_communities.clone() {
		// Per-asset measurements, aggregated by community_uuid in settlement, reproduce
		// the community's net. Here they mirror the forecasts, so measured == forecast.
		let measurements: Vec<MeasurementSchema> = community
			.forecasts
			.iter()
			.map(|forecast| MeasurementSchema {
				area_uuid: forecast.area_uuid.clone(),
				area_hash: forecast.area_hash.clone(),
				community_uuid: community.community_uuid.clone(),
				energy_kwh: forecast.energy_kwh,
				time_slot: world.target_delivery_time,
				creation_time: 1,
			})
			.collect();
		adapter
			.forward_measurement(measurements)
			.await
			.expect("Measurement forwarding failed.");
	}
	info!("Submitted per-asset measurements for all inter-community participants");
}

#[then("exactly one aggregated order per community is stored in the inter-community market")]
async fn verify_one_order_per_community(world: &mut MyWorld) {
	let market = world
		.inter_community_market
		.clone()
		.expect("the inter-community market must have been created");
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));
	let expected = world.inter_communities.len();

	// Allow storage to catch up with the on-chain inserts.
	let mut orders: Vec<DbOrderSchema> = Vec::new();
	for _ in 0..30 {
		orders = adapter.get_orders_for_market(&market.market_id).await;
		if orders.len() >= expected {
			break;
		}
		tokio::time::sleep(Duration::from_secs(2)).await;
	}

	assert_eq!(
		orders.len(),
		expected,
		"expected exactly one aggregated order per community in the inter-community market, got {}",
		orders.len()
	);

	for community in &world.inter_communities {
		let area = h256_to_string(community.community_id);
		let matching: Vec<&DbOrderSchema> =
			orders.iter().filter(|o| order_area_uuid(o) == area).collect();
		assert_eq!(
			matching.len(),
			1,
			"exactly one aggregated order must carry community '{}'s area_uuid",
			community.name
		);
		assert_eq!(
			order_market_id(matching[0]),
			market.market_id,
			"the aggregated order for '{}' must sit in the reserved inter-community market",
			community.name
		);
	}
	info!("Verified exactly one aggregated order per community in the inter-community market");
}

#[then("a trade is settled in the inter-community market with the reserved market id")]
async fn verify_inter_community_trade_settled(world: &mut MyWorld) {
	let reserved_id = inter_community_market_id(world.target_delivery_time);

	// The reserved id must differ from every participant's per-community spot id.
	for community in &world.inter_communities {
		assert_ne!(
			reserved_id, community.spot_market_id,
			"the reserved inter-community id must differ from '{}'s spot market id",
			community.name
		);
	}

	info!("Waiting for a settled trade in the inter-community market {:?}...", reserved_id);
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		info!("Waiting for OrderExecuted in the inter-community market... check {}/40", i + 1);
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
			if let Ok(e) = ev {
				let trade = e.0;
				if trade.market_id == reserved_id {
					// Both legs are within the reserved market: a genuine inter-community trade.
					assert_eq!(
						trade.bid.bid_component.market_id, reserved_id,
						"the bid leg must be in the reserved inter-community market"
					);
					assert_eq!(
						trade.offer.offer_component.market_id, reserved_id,
						"the offer leg must be in the reserved inter-community market"
					);
					// The two legs come from distinct communities (distinct area_uuids).
					assert_ne!(
						trade.bid.bid_component.area_uuid, trade.offer.offer_component.area_uuid,
						"a community must not self-match in the inter-community market"
					);
					info!(
						"Inter-community trade settled in {:?}: {} energy units",
						trade.market_id, trade.parameters.selected_energy
					);
					return;
				}
			}
		}
	}
	panic!(
		"Timeout: no trade was settled in the inter-community market {:?} within 40 blocks",
		reserved_id
	);
}

#[then("no inter-community order cross-matches a spot order")]
async fn verify_no_inter_community_cross_match(world: &mut MyWorld) {
	let reserved_id = inter_community_market_id(world.target_delivery_time);

	info!("Scanning settled trades to confirm no inter-community/spot cross-match...");
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	// Matching partitions strictly by market_id, so every settled trade must keep both
	// legs in the same market; in particular a trade touching the reserved inter-community
	// market must be wholly within it and never pair with any spot market order.
	for i in 0..20 {
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
			if let Ok(e) = ev {
				let trade = e.0;
				let bid_market = trade.bid.bid_component.market_id;
				let offer_market = trade.offer.offer_component.market_id;

				assert_eq!(
					bid_market, offer_market,
					"cross-market match settled: bid in {:?} but offer in {:?} (trade {:?})",
					bid_market, offer_market, trade.market_id
				);

				if bid_market == reserved_id || offer_market == reserved_id {
					assert!(
						bid_market == reserved_id && offer_market == reserved_id,
						"an inter-community order cross-matched a spot order (bid {:?}, offer {:?})",
						bid_market,
						offer_market
					);
				}
			}
		}
		info!("Cross-match invariant held through block check {}/20", i + 1);
	}
	info!("No inter-community order cross-matched a spot order");
}

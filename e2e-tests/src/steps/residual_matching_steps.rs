use crate::world::{gsy_node, MyWorld};
use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{then, when};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::db_api_schema::market::{AssetType, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{string_to_h256, NODE_FLOAT_SCALING_FACTOR};
use gsy_offchain_primitives::{constants::GlobalConstants, MarketType};
use std::time::Duration;
use subxt::utils::{AccountId32, H256};
use tracing::info;

// Distinct community names isolate the two scenarios' markets (market_id is derived from the
// community name), since both run in the same delivery slot.
const RESIDUAL_COMMUNITY: &str = "Residual Community";
const PARTIAL_COMMUNITY: &str = "Partial Community";

// Shared area labels: the residual scenario uses one seller, the partial scenario uses both.
// Area uuids/hashes are random per market, so reusing these names across communities is safe.
const BUYER_AREA: &str = "buyer";
const SELLER_AREA_ONE: &str = "sellerOne";
const SELLER_AREA_TWO: &str = "sellerTwo";

const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

struct MatchedTrade {
	buyer: AccountId32,
	seller: AccountId32,
	selected_energy: u64,
	residual_bid_energy: Option<u64>,
	residual_offer_energy: Option<u64>,
}

fn orderbook_url() -> String {
	std::env::var("OFFCHAIN_STORAGE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

fn node_url() -> String {
	std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string())
}

fn scaled(energy: f64) -> u64 {
	(energy * NODE_FLOAT_SCALING_FACTOR) as u64
}

/// Build a forecast for a named area in the market topology. A positive `energy_kwh` produces a
/// bid, a negative one an offer (see `create_input_orders` in the community client).
fn forecast_for_area(
	market: &MarketTopologySchema,
	area_name: &str,
	energy_kwh: f64,
	time_slot: u64,
) -> ForecastSchema {
	let area_hash = market
		.community_areas
		.iter()
		.find(|a| a.name == area_name)
		.unwrap_or_else(|| panic!("area {} present in topology", area_name))
		.area_hash
		.clone();

	ForecastSchema {
		area_uuid: area_name.to_string(),
		area_hash,
		community_uuid: market.community_uuid.clone(),
		time_slot,
		creation_time: 1,
		energy_kwh,
		confidence: 1.0,
	}
}

/// Compute and store the next available delivery time slot on the world.
fn next_delivery_time(world: &mut MyWorld) -> u64 {
	let now = Utc::now();
	world.target_delivery_time = ((now + ChronoDuration::hours(2)).timestamp() as u64
		/ GlobalConstants.TIME_SLOT_SEC)
		* GlobalConstants.TIME_SLOT_SEC;
	world.target_delivery_time
}


#[when(
	regex = r#"the residual community topology is submitted with a bid of (\d+) energy and an offer of (\d+) energy"#
)]
async fn submit_residual_topology(world: &mut MyWorld, bid_energy: f64, offer_energy: f64) {
	let time_slot = next_delivery_time(world);
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	let market = adapter
		.get_or_create_market_topology(
			vec![ExternalCommunityTopology {
				community_name: RESIDUAL_COMMUNITY.to_string(),
				areas: vec![
					ExternalAreaTopology {
						area_type: AssetType::SMART_METER,
						area_name: BUYER_AREA.to_string(),
					},
					ExternalAreaTopology {
						area_type: AssetType::PV,
						area_name: SELLER_AREA_ONE.to_string(),
					},
				],
			}],
			world.target_delivery_time,
		)
		.await
		.get(0)
		.expect("expected a market topology for the residual community")
		.clone();

	world.last_market_id = Some(string_to_h256(market.market_id.clone()));
	assert_eq!(
		world.last_market_id.unwrap(),
		world.generate_market_id(RESIDUAL_COMMUNITY, MarketType::Spot),
		"market_id must match the community-aware hash for {}",
		RESIDUAL_COMMUNITY
	);

	let bid_forecast = forecast_for_area(&market, BUYER_AREA, bid_energy, time_slot);
	let offer_forecast = forecast_for_area(&market, SELLER_AREA_ONE, -offer_energy, time_slot);

	adapter
		.forward_forecast(vec![bid_forecast.clone(), offer_forecast.clone()])
		.await
		.expect("Forecast forwarding failed.");

	world.buyer_hash = Some(bid_forecast.area_hash.clone());
	world.seller_hash = Some(offer_forecast.area_hash.clone());
	world.bid_forecast = Some(bid_forecast);
	world.offer_forecast = Some(offer_forecast);
	world.topology_schema = Some(market);
	world.active_community_name = Some(RESIDUAL_COMMUNITY.to_string());
	world.initial_trade_energy = None;
	world.residual_trade_energy = None;
}

#[when(regex = r#""([^"]*)" submits the residual-trade bid"#)]
async fn submit_residual_bid(world: &mut MyWorld, user_name: String) {
	let user = world.users.get(&user_name).unwrap().clone();
	publish_orders(
		node_url(),
		vec![world.bid_forecast.clone().unwrap()],
		world.topology_schema.clone().unwrap(),
		BID_RATE,
		OFFER_RATE,
		&user,
	)
	.await
	.expect("Failed to publish residual-trade bid");
	info!("Submitted residual-trade bid for {}", user_name);
}

#[when(regex = r#""([^"]*)" submits the residual-trade offer"#)]
async fn submit_residual_offer(world: &mut MyWorld, user_name: String) {
	let user = world.users.get(&user_name).unwrap().clone();
	publish_orders(
		node_url(),
		vec![world.offer_forecast.clone().unwrap()],
		world.topology_schema.clone().unwrap(),
		BID_RATE,
		OFFER_RATE,
		&user,
	)
	.await
	.expect("Failed to publish residual-trade offer");
	info!("Submitted residual-trade offer for {}", user_name);
}

#[when(regex = r#""([^"]*)" submits a follow-up offer of (\d+) energy for the residual bid"#)]
async fn submit_followup_offer(world: &mut MyWorld, user_name: String, energy: f64) {
	let user = world.users.get(&user_name).unwrap().clone();
	// Reuse the seller area, but with the smaller energy volume that exactly covers the
	// residual bid left behind by the initial partial match.
	let mut forecast = world.offer_forecast.clone().unwrap();
	forecast.energy_kwh = -energy;

	publish_orders(
		node_url(),
		vec![forecast],
		world.topology_schema.clone().unwrap(),
		BID_RATE,
		OFFER_RATE,
		&user,
	)
	.await
	.expect("Failed to publish follow-up offer");
	info!("Submitted follow-up offer of {} energy for {}", energy, user_name);
}

#[then(
	regex = r#"the matching engine settles the initial trade of (\d+) energy and a residual bid for (\d+) energy remains"#
)]
async fn verify_initial_trade(world: &mut MyWorld, trade_energy: f64, residual_energy: f64) {
	let market_id = world.generate_market_id(RESIDUAL_COMMUNITY, MarketType::Spot);
	let expected_energy = scaled(trade_energy);
	let expected_residual = scaled(residual_energy);

	let trade = find_trade_in_market(world, market_id, expected_energy)
		.await
		.expect("Timeout: did not observe the initial residual-trade match");

	let buyer_account_id: AccountId32 = world.users.get("charlie").unwrap().public_key().into();
	let seller_account_id: AccountId32 = world.users.get("bob").unwrap().public_key().into();
	assert_eq!(trade.buyer, buyer_account_id, "initial trade buyer should be charlie");
	assert_eq!(trade.seller, seller_account_id, "initial trade seller should be bob");
	assert_eq!(
		trade.selected_energy, expected_energy,
		"initial trade should match the smaller offer volume"
	);

	let residual_bid_energy = trade
		.residual_bid_energy
		.expect("a residual bid should be created from the partially matched bid");
	assert_eq!(
		residual_bid_energy, expected_residual,
		"residual bid energy should be the leftover of the initial bid"
	);
	assert!(
		trade.residual_offer_energy.is_none(),
		"the offer was fully consumed and should not leave a residual"
	);

	info!(
		"Initial trade settled for {} energy units; residual bid for {} energy units created.",
		expected_energy, expected_residual
	);
	world.initial_trade_energy = Some(trade.selected_energy);
}

#[then(regex = r#"the matching engine settles the residual trade of (\d+) energy"#)]
async fn verify_residual_trade(world: &mut MyWorld, trade_energy: f64) {
	let market_id = world.generate_market_id(RESIDUAL_COMMUNITY, MarketType::Spot);
	let expected_energy = scaled(trade_energy);

	let trade = find_trade_in_market(world, market_id, expected_energy)
		.await
		.expect("Timeout: did not observe the residual trade match");

	let buyer_account_id: AccountId32 = world.users.get("charlie").unwrap().public_key().into();
	let seller_account_id: AccountId32 = world.users.get("bob").unwrap().public_key().into();
	assert_eq!(trade.buyer, buyer_account_id, "residual trade buyer should be charlie");
	assert_eq!(trade.seller, seller_account_id, "residual trade seller should be bob");
	assert_eq!(
		trade.selected_energy, expected_energy,
		"residual trade should clear the full residual energy"
	);
	assert!(trade.residual_bid_energy.is_none(), "residual trade should not leave a residual bid");
	assert!(
		trade.residual_offer_energy.is_none(),
		"residual trade should not leave a residual offer"
	);

	info!("Residual trade settled for {} energy units.", expected_energy);
	world.residual_trade_energy = Some(trade.selected_energy);
}

#[then(regex = r#"the settled trades for the market add up to the initial bid volume of (\d+) energy"#)]
async fn verify_total_traded_volume(world: &mut MyWorld, bid_energy: f64) {
	let expected_total = scaled(bid_energy);
	let initial = world.initial_trade_energy.expect("initial trade must have been observed");
	let residual = world.residual_trade_energy.expect("residual trade must have been observed");

	assert_eq!(
		initial + residual,
		expected_total,
		"the initial trade ({}) plus the residual trade ({}) must equal the initial bid volume ({})",
		initial,
		residual,
		expected_total
	);
	info!(
		"Initial trade {} + residual trade {} = {} energy units, matching the initial bid volume.",
		initial, residual, expected_total
	);
}

#[when("the partial community topology is submitted with one buyer and two sellers")]
async fn submit_partial_topology(world: &mut MyWorld) {
	next_delivery_time(world);
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url()));

	// One consumer (buyer) and two producers (sellers) so a single seller can place two
	// independent offers in the same matching cycle.
	let market = adapter
		.get_or_create_market_topology(
			vec![ExternalCommunityTopology {
				community_name: PARTIAL_COMMUNITY.to_string(),
				areas: vec![
					ExternalAreaTopology {
						area_type: AssetType::SMART_METER,
						area_name: BUYER_AREA.to_string(),
					},
					ExternalAreaTopology {
						area_type: AssetType::PV,
						area_name: SELLER_AREA_ONE.to_string(),
					},
					ExternalAreaTopology {
						area_type: AssetType::PV,
						area_name: SELLER_AREA_TWO.to_string(),
					},
				],
			}],
			world.target_delivery_time,
		)
		.await
		.get(0)
		.expect("expected a market topology for the partial-trade community")
		.clone();

	world.last_market_id = Some(string_to_h256(market.market_id.clone()));
	assert_eq!(
		world.last_market_id.unwrap(),
		world.generate_market_id(PARTIAL_COMMUNITY, MarketType::Spot),
		"market_id must match the community-aware hash for {}",
		PARTIAL_COMMUNITY
	);

	world.topology_schema = Some(market);
	world.active_community_name = Some(PARTIAL_COMMUNITY.to_string());
	world.initial_trade_energy = None;
	world.residual_trade_energy = None;
}

#[when(
	regex = r#""([^"]*)" submits a bid of (\d+) energy and "([^"]*)" submits offers of (\d+) and (\d+) energy in the same cycle"#
)]
async fn submit_partial_orders(
	world: &mut MyWorld,
	buyer_name: String,
	bid_energy: f64,
	seller_name: String,
	offer_one_energy: f64,
	offer_two_energy: f64,
) {
	let market = world.topology_schema.clone().expect("topology must be submitted first");
	let time_slot = market.time_slot as u64;
	let buyer = world.users.get(&buyer_name).unwrap().clone();
	let seller = world.users.get(&seller_name).unwrap().clone();

	let bid_forecast = forecast_for_area(&market, BUYER_AREA, bid_energy, time_slot);
	let offer_one = forecast_for_area(&market, SELLER_AREA_ONE, -offer_one_energy, time_slot);
	let offer_two = forecast_for_area(&market, SELLER_AREA_TWO, -offer_two_energy, time_slot);

	// Publish the single, over-sized bid.
	publish_orders(
		node_url(),
		vec![bid_forecast],
		market.clone(),
		BID_RATE,
		OFFER_RATE,
		&buyer,
	)
	.await
	.expect("Failed to publish partial-trade bid");

	// Publish both offers together so they enter the order book in the same matching cycle. The
	// first matching cycle can only clear one of them against the bid (leaving a residual bid);
	// the residual must then be cleared against the remaining offer in a following cycle.
	publish_orders(
		node_url(),
		vec![offer_one, offer_two],
		market.clone(),
		BID_RATE,
		OFFER_RATE,
		&seller,
	)
	.await
	.expect("Failed to publish partial-trade offers");

	info!(
		"Submitted a bid of {} energy ({}) and offers of {} and {} energy ({}) in the same cycle",
		bid_energy, buyer_name, offer_one_energy, offer_two_energy, seller_name
	);
}

#[then("the matching engine settles a partial trade that leaves a residual bid")]
async fn verify_partial_trade(world: &mut MyWorld) {
	let market_id = world.generate_market_id(PARTIAL_COMMUNITY, MarketType::Spot);

	let trade = find_trade_with_residual_bid(world, market_id)
		.await
		.expect("Timeout: did not observe a partial match leaving a residual bid");

	let buyer_account_id: AccountId32 = world.users.get("charlie").unwrap().public_key().into();
	let seller_account_id: AccountId32 = world.users.get("bob").unwrap().public_key().into();
	assert_eq!(trade.buyer, buyer_account_id, "partial trade buyer should be charlie");
	assert_eq!(trade.seller, seller_account_id, "partial trade seller should be bob");

	let residual = trade
		.residual_bid_energy
		.expect("the partially matched bid must leave a residual bid");
	assert!(residual > 0, "the residual bid energy must be positive");
	assert!(
		trade.residual_offer_energy.is_none(),
		"the matched offer was fully consumed and should not leave a residual"
	);

	info!(
		"Partial trade settled for {} energy units; residual bid for {} energy units created.",
		trade.selected_energy, residual
	);
	world.initial_trade_energy = Some(trade.selected_energy);
}

#[then(
	regex = r#"the residual bid is cleared by a later matching cycle and the trades add up to (\d+) energy"#
)]
async fn verify_residual_cleared(world: &mut MyWorld, total_energy: f64) {
	let market_id = world.generate_market_id(PARTIAL_COMMUNITY, MarketType::Spot);
	let total = scaled(total_energy);
	let initial = world
		.initial_trade_energy
		.expect("the partial trade must have been observed first");
	// The residual left by the partial match equals the leftover offer, so the clearing trade in
	// the next cycle must settle exactly that much energy.
	let expected_residual_trade = total - initial;

	let trade = find_trade_in_market(world, market_id, expected_residual_trade)
		.await
		.expect("Timeout: the residual bid was not cleared in a later matching cycle");

	let buyer_account_id: AccountId32 = world.users.get("charlie").unwrap().public_key().into();
	let seller_account_id: AccountId32 = world.users.get("bob").unwrap().public_key().into();
	assert_eq!(trade.buyer, buyer_account_id, "residual trade buyer should be charlie");
	assert_eq!(trade.seller, seller_account_id, "residual trade seller should be bob");
	assert!(
		trade.residual_bid_energy.is_none(),
		"clearing the residual must not leave another residual bid"
	);
	assert!(
		trade.residual_offer_energy.is_none(),
		"clearing the residual must not leave a residual offer"
	);
	assert_eq!(
		initial + trade.selected_energy,
		total,
		"the partial trade ({}) plus the residual trade ({}) must equal the bid volume ({})",
		initial,
		trade.selected_energy,
		total
	);

	world.residual_trade_energy = Some(trade.selected_energy);
	info!(
		"Residual bid cleared: partial trade {} + residual trade {} = {} energy units.",
		initial, trade.selected_energy, total
	);
}

#[when("the Market Orchestrator opens the residual-matching Spot market")]
async fn wait_for_market_to_open(world: &mut MyWorld) {
	let community = world
		.active_community_name
		.clone()
		.expect("a topology must be submitted before opening the market");
	let market_id = world.generate_market_id(&community, MarketType::Spot);
	info!("Waiting for the Market Orchestrator to open the Spot market {:?}...", market_id);

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block from node")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::MarketStatusUpdated>() {
			if let Ok(e) = ev {
				if e.0 == market_id && e.1 {
					info!("Market {:?} opened on-chain.", market_id);
					// Give the off-chain order book a moment to register the open market.
					tokio::time::sleep(Duration::from_secs(6)).await;
					return;
				}
			}
		}
		info!("Waiting for MarketStatusUpdated(true) for {:?}... check {}/40", market_id, i + 1);
	}
	panic!("Timeout: the orchestrator did not open the Spot market {:?}", market_id);
}

/// Wait for an `OrderExecuted` in `market_id` whose settled energy equals `expected_energy`.
async fn find_trade_in_market(
	world: &MyWorld,
	market_id: H256,
	expected_energy: u64,
) -> Option<MatchedTrade> {
	find_trade(world, market_id, &|trade| trade.selected_energy == expected_energy).await
}

/// Wait for an `OrderExecuted` in `market_id` that carries a residual bid (i.e. a partial match).
async fn find_trade_with_residual_bid(world: &MyWorld, market_id: H256) -> Option<MatchedTrade> {
	find_trade(world, market_id, &|trade| trade.residual_bid_energy.is_some()).await
}

/// Scan recently finalized history first, then follow new finalized blocks, looking for the first
/// trade in `market_id` matching `predicate`.
async fn find_trade(
	world: &MyWorld,
	market_id: H256,
	predicate: &(dyn Fn(&MatchedTrade) -> bool + Sync),
) -> Option<MatchedTrade> {
	if let Some(trade) = scan_recent_blocks(world, market_id, predicate).await {
		info!("Found matching trade in recently finalized history.");
		return Some(trade);
	}

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		if let Some(trade) = match_trade_in_events(&events, market_id, predicate) {
			return Some(trade);
		}
		info!("Waiting for OrderExecuted in market {:?}... check {}/40", market_id, i + 1);
	}
	None
}

async fn scan_recent_blocks(
	world: &MyWorld,
	market_id: H256,
	predicate: &(dyn Fn(&MatchedTrade) -> bool + Sync),
) -> Option<MatchedTrade> {
	const MAX_BLOCKS_TO_SCAN: u32 = 30;

	let mut cursor = world.subxt_client.blocks().at_latest().await.ok()?;
	let tip = cursor.number();

	loop {
		if let Ok(events) = cursor.events().await {
			if let Some(trade) = match_trade_in_events(&events, market_id, predicate) {
				return Some(trade);
			}
		}

		let number = cursor.number();
		if number == 0 || tip.saturating_sub(number) >= MAX_BLOCKS_TO_SCAN {
			return None;
		}

		let parent_hash = cursor.header().parent_hash;
		cursor = world.subxt_client.blocks().at(parent_hash).await.ok()?;
	}
}

fn match_trade_in_events(
	events: &subxt::events::Events<subxt::SubstrateConfig>,
	market_id: H256,
	predicate: &(dyn Fn(&MatchedTrade) -> bool + Sync),
) -> Option<MatchedTrade> {
	for ev in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
		if let Ok(e) = ev {
			let trade = e.0;
			if trade.market_id != market_id {
				continue;
			}
			let matched = MatchedTrade {
				buyer: trade.buyer,
				seller: trade.seller,
				selected_energy: trade.parameters.selected_energy,
				residual_bid_energy: trade.residual_bid.map(|bid| bid.bid_component.energy),
				residual_offer_energy: trade.residual_offer.map(|offer| offer.offer_component.energy),
			};
			if predicate(&matched) {
				return Some(matched);
			}
		}
	}
	None
}

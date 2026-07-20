use crate::world::{gsy_node, CommunityMarket, MyWorld};
use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{then, when};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::db_api_schema::market::AssetType;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::string_to_h256;
use gsy_offchain_primitives::{constants::GlobalConstants, MarketType};
use std::collections::HashSet;
use std::time::Duration;
use subxt::utils::H256;
use tracing::info;

#[when(
	regex = r#"the community topologies and forecasts of (\d+) energy are submitted for communities "([^"]*)" and "([^"]*)""#
)]
async fn submit_parallel_topologies(
	world: &mut MyWorld,
	energy: f64,
	community_a: String,
	community_b: String,
) {
	let now = Utc::now();
	world.target_delivery_time = ((now + ChronoDuration::hours(2)).timestamp() as u64
		/ GlobalConstants.TIME_SLOT_SEC)
		* GlobalConstants.TIME_SLOT_SEC;

	let orderbook_url = std::env::var("OFFCHAIN_STORAGE_URL")
		.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url));

	let community_names = vec![community_a, community_b];
	let external_topologies: Vec<ExternalCommunityTopology> = community_names
		.iter()
		.map(|name| ExternalCommunityTopology {
			community_name: name.clone(),
			areas: vec![
				ExternalAreaTopology {
					area_type: AssetType::SMART_METER,
					area_name: format!("{}_buyer", name),
				},
				ExternalAreaTopology {
					area_type: AssetType::PV,
					area_name: format!("{}_seller", name),
				},
			],
		})
		.collect();

	let markets = adapter
		.get_or_create_market_topology(external_topologies, world.target_delivery_time)
		.await;
	assert_eq!(
		markets.len(),
		community_names.len(),
		"Expected one market per community"
	);

	world.community_markets.clear();
	for market in markets {
		let buyer_area = format!("{}_buyer", market.community_name);
		let seller_area = format!("{}_seller", market.community_name);
		let buyer_hash = market
			.community_areas
			.iter()
			.find(|a| a.name == buyer_area)
			.expect("buyer area present in topology")
			.area_hash
			.clone();
		let seller_hash = market
			.community_areas
			.iter()
			.find(|a| a.name == seller_area)
			.expect("seller area present in topology")
			.area_hash
			.clone();

		// Each community must produce a distinct, community-aware market id.
		let market_id = string_to_h256(market.market_id.clone());
		assert_eq!(
			market_id,
			world.generate_market_id(&market.community_name, MarketType::Spot),
			"market_id must match the community-aware hash for {}",
			market.community_name
		);

		let bid_forecast = ForecastSchema {
			area_uuid: buyer_area.clone(),
			area_hash: buyer_hash.clone(),
			community_uuid: market.community_uuid.clone(),
			time_slot: world.target_delivery_time,
			creation_time: 1,
			energy_kwh: energy,
			confidence: 1.0,
		};
		let offer_forecast = ForecastSchema {
			area_uuid: seller_area.clone(),
			area_hash: seller_hash.clone(),
			community_uuid: market.community_uuid.clone(),
			time_slot: world.target_delivery_time,
			creation_time: 1,
			energy_kwh: -energy,
			confidence: 1.0,
		};

		adapter
			.forward_forecast(vec![bid_forecast.clone(), offer_forecast.clone()])
			.await
			.expect("Forecast forwarding failed.");

		world.community_markets.push(CommunityMarket {
			name: market.community_name.clone(),
			market_id,
			topology: market.clone(),
			buyer_area,
			seller_area,
			buyer_hash,
			seller_hash,
			bid_forecast,
			offer_forecast,
		});
	}

	info!(
		"Created {} community markets for delivery slot {}",
		world.community_markets.len(),
		world.target_delivery_time
	);
	assert!(
		world.community_markets.len() >= 2,
		"Expected at least two community markets to run in parallel"
	);
}

#[when("the Market Orchestrator opens the Spot markets for all communities")]
async fn wait_for_markets_to_open(world: &mut MyWorld) {
	let mut remaining: HashSet<H256> =
		world.community_markets.iter().map(|c| c.market_id).collect();
	info!(
		"Waiting for the Market Orchestrator to open {} community spot markets...",
		remaining.len()
	);

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..60 {
		if remaining.is_empty() {
			info!("All community spot markets are open on-chain.");
			tokio::time::sleep(Duration::from_secs(6)).await;
			return;
		}
		info!(
			"Waiting for MarketStatusUpdated events... {} market(s) remaining, check {}/60",
			remaining.len(),
			i + 1
		);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block from node")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::MarketStatusUpdated>() {
			if let Ok(e) = ev {
				if e.1 && remaining.remove(&e.0) {
					info!("-> Market opened: {:?}", e.0);
				}
			}
		}
	}
	panic!(
		"Timeout: the orchestrator did not open all community markets. Remaining: {:?}",
		remaining
	);
}

#[when("bids and offers are submitted for all communities")]
async fn submit_parallel_orders(world: &mut MyWorld) {
	let node_url =
		std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());
	let buyer = world.users.get("charlie").unwrap().clone();
	let seller = world.users.get("bob").unwrap().clone();

	for community in world.community_markets.clone() {
		// open_time == close_time fully progresses the offer rate ramp so it resolves to the
		// (confidence-1.0) floor MIN_ORDER_RATE, preserving the old flat offer rate.
		let slot = community.topology.time_slot as u64;
		publish_orders(
			node_url.clone(),
			vec![community.bid_forecast.clone()],
			community.topology.clone(),
			0.3,
			slot,
			slot,
			&buyer,
		)
		.await
		.expect("Failed to publish bid");

		publish_orders(
			node_url.clone(),
			vec![community.offer_forecast.clone()],
			community.topology.clone(),
			0.3,
			slot,
			slot,
			&seller,
		)
		.await
		.expect("Failed to publish offer");

		info!(
			"Submitted bid (charlie) and offer (bob) for community '{}'",
			community.name
		);
	}
}

#[when("measurements for all community assets are submitted")]
async fn submit_parallel_measurements(world: &mut MyWorld) {
	let orderbook_url = std::env::var("OFFCHAIN_STORAGE_URL")
		.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url));

	for community in world.community_markets.clone() {
		let measurements = vec![
			MeasurementSchema {
				area_uuid: community.buyer_area.clone(),
				area_hash: community.buyer_hash.clone(),
				community_uuid: community.topology.community_uuid.clone(),
				energy_kwh: 12.0,
				time_slot: world.target_delivery_time,
				creation_time: 1,
			},
			MeasurementSchema {
				area_uuid: community.seller_area.clone(),
				area_hash: community.seller_hash.clone(),
				community_uuid: community.topology.community_uuid.clone(),
				energy_kwh: -8.0,
				time_slot: world.target_delivery_time,
				creation_time: 1,
			},
		];
		adapter.forward_measurement(measurements).await.unwrap();
	}
	info!("Submitted measurements for all community assets");
}

#[then("a trade is settled on-chain for each community market")]
async fn verify_parallel_trades(world: &mut MyWorld) {
	let mut remaining: HashSet<H256> =
		world.community_markets.iter().map(|c| c.market_id).collect();
	let total = remaining.len();
	info!(
		"Waiting for a settled trade in each of the {} community markets...",
		total
	);

	scan_recent_trades_for_markets(world, &mut remaining).await;
	if remaining.is_empty() {
		info!("All community spot markets had already settled before watching started.");
		return;
	}

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		if remaining.is_empty() {
			info!(
				"Observed a settled trade for all {} community markets.",
				total
			);
			return;
		}
		info!(
			"Waiting for OrderExecuted events... {} market(s) remaining, check {}/40",
			remaining.len(),
			i + 1
		);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
			if let Ok(e) = ev {
				let trade = e.0;
				if remaining.remove(&trade.market_id) {
					info!(
						"Trade settled in market {:?}: buyer {:?}, seller {:?}, energy {}",
						trade.market_id, trade.buyer, trade.seller, trade.parameters.selected_energy
					);
				}
			}
		}
	}
	panic!(
		"Timeout: trades were not settled for all community markets. Remaining: {:?}",
		remaining
	);
}

async fn scan_recent_trades_for_markets(world: &MyWorld, remaining: &mut HashSet<H256>) {
	const MAX_BLOCKS_TO_SCAN: u32 = 60;

	let mut cursor = match world.subxt_client.blocks().at_latest().await {
		Ok(block) => block,
		Err(_) => return,
	};
	let tip = cursor.number();

	loop {
		if let Ok(events) = cursor.events().await {
			for ev in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
				if let Ok(e) = ev {
					let trade = e.0;
					if remaining.remove(&trade.market_id) {
						info!(
							"Trade already settled (found in history) in market {:?}",
							trade.market_id
						);
					}
				}
			}
		}

		let number = cursor.number();
		if remaining.is_empty() || number == 0 || tip.saturating_sub(number) >= MAX_BLOCKS_TO_SCAN {
			break;
		}

		let parent_hash = cursor.header().parent_hash;
		cursor = match world.subxt_client.blocks().at(parent_hash).await {
			Ok(block) => block,
			Err(_) => break,
		};
	}
}

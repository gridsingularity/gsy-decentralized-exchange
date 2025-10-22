use crate::world::{gsy_node, MyWorld};
use cucumber::{then, when};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::h256_to_string;
use gsy_offchain_primitives::MarketType;
use std::time::Duration;
use subxt::utils::H256;
use tokio::time::sleep;
use tracing::info;

#[when(regex = r#""([^"]*)" submits a bid for (\d+) energy"#)]
async fn submit_bid(world: &mut MyWorld, user_name: String, energy: f64) {
	let user = world.users.get(&user_name).unwrap().clone();
	let market_id = world.generate_market_id(MarketType::Spot);
	world.last_market_id = Some(market_id);

	let forecast = ForecastSchema {
		area_uuid: format!("area-{}", user_name),
		community_uuid: "community1".to_string(),
		time_slot: world.target_delivery_time,
		creation_time: 1,
		energy_kwh: energy,
		confidence: 1.0,
	};

	let market = MarketTopologySchema {
		market_id: h256_to_string(market_id),
		community_uuid: "community1".to_string(),
		community_name: "Test Community".to_string(),
		time_slot: world.target_delivery_time as u32,
		creation_time: 1,
		area_uuids: vec![AreaTopologySchema {
			area_uuid: format!("area-{}", user_name),
			name: user_name.clone(),
			area_hash: h256_to_string(H256::random()),
		}],
	};

	let node_url =
		std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());

	publish_orders(node_url, vec![forecast], market, &user)
		.await
		.expect("Failed to publish bid");
	println!("Submitted bid for {}", user_name);
}

#[when(regex = r#""([^"]*)" submits an offer for (\d+) energy"#)]
async fn submit_offer(world: &mut MyWorld, user_name: String, energy: f64) {
	let user = world.users.get(&user_name).unwrap().clone();
	let market_id = world.generate_market_id(MarketType::Spot);

	let forecast = ForecastSchema {
		area_uuid: format!("area-{}", user_name),
		community_uuid: "community1".to_string(),
		time_slot: world.target_delivery_time,
		creation_time: 1,
		energy_kwh: -energy,
		confidence: 1.0,
	};

	let market = MarketTopologySchema {
		market_id: h256_to_string(market_id),
		community_uuid: "community1".to_string(),
		community_name: "Test Community".to_string(),
		time_slot: world.target_delivery_time as u32,
		creation_time: 1,
		area_uuids: vec![AreaTopologySchema {
			area_uuid: format!("area-{}", user_name),
			name: user_name.clone(),
			area_hash: h256_to_string(H256::random()),
		}],
	};

	let node_url =
		std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());

	publish_orders(node_url, vec![forecast], market, &user)
		.await
		.expect("Failed to publish offer");
	println!("Submitted offer for {}", user_name);
}

#[when(regex = r#"measurements for "([^"]*)" and "([^"]*)" assets are submitted"#)]
async fn submit_measurements(world: &mut MyWorld, _user1: String, _user2: String) {
	let orderbook_url = std::env::var("ORDERBOOK_SERVICE_URL")
		.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url));

	let measurements = vec![
		MeasurementSchema {
			area_uuid: "area-alice".to_string(),
			community_uuid: "community1".to_string(),
			energy_kwh: 12.0,
			time_slot: world.target_delivery_time,
			creation_time: 1,
		},
		MeasurementSchema {
			area_uuid: "area-bob".to_string(),
			community_uuid: "community1".to_string(),
			energy_kwh: -8.0,
			time_slot: world.target_delivery_time,
			creation_time: 1,
		},
	];
	adapter.forward_measurement(measurements).await.unwrap();
	println!("Submitted measurements");
}

#[then("the matching engine matches the bid and offer and a trade is settled on-chain")]
async fn verify_trade_on_chain(world: &mut MyWorld) {
	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..30 {
		println!("Waiting for match, block {}/30...", i + 1);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();

		let order_executed_event =
			events.find_first::<gsy_node::orderbook_registry::events::OrderExecuted>();

		if let Ok(Some(event)) = order_executed_event {
			println!("OrderExecuted event found: {:?}", event.0);
			let trade = event.0;
			let alice_pubkey = world.users.get("alice").unwrap().public_key();
			let bob_pubkey = world.users.get("bob").unwrap().public_key();
			let alice_account_id: subxt::utils::AccountId32 = alice_pubkey.into();
			let bob_account_id: subxt::utils::AccountId32 = bob_pubkey.into();

			assert_eq!(trade.buyer, alice_account_id);
			assert_eq!(trade.seller, bob_account_id);
			assert_eq!(trade.parameters.selected_energy, 100000);
			let expected_rate = 30000;
			assert_eq!(trade.parameters.energy_rate, expected_rate);
			return;
		}
	}
	panic!("Timeout: Did not find OrderExecuted event after 30 blocks");
}

#[then("the execution engine submits penalties for the trade")]
async fn verify_penalties_on_chain(world: &mut MyWorld) {
	info!("Waiting for execution engine to calculate and submit penalties...");

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		info!("Waiting for penalty submission, block check {}/40...", i + 1);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block for penalty check")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();

		let penalty_event =
			events.find_first::<gsy_node::trades_settlement::events::PenaltiesSubmitted>();

		if let Ok(Some(event)) = penalty_event {
			info!("✅ PenaltiesSubmitted event found: {:?}", event.0);
			// For now, we just confirm the event was emitted.
			return;
		}
	}

	panic!("Timeout: Did not find PenaltiesSubmitted event after 40 blocks");
}

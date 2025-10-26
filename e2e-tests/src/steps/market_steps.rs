use crate::world::{gsy_node, MyWorld};
use chrono::{prelude::DateTime, prelude::NaiveDateTime, Duration as ChronoDuration, Utc};
use cucumber::{given, when};
use gsy_community_client::external_api::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::db_api_schema::market::AreaTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::h256_to_string;
use gsy_offchain_primitives::{constants::GlobalConstants, utils::timestamp_to_datetime_string, MarketType};
use std::time::Duration as Duration;
use subxt::utils::H256;
use tokio::time::sleep;
use tracing::info;

#[when(regex = r#"the community topology and forecasts of (\d+) energy are submitted"#)]
async fn submit_topology_forecasts(world: &mut MyWorld, energy: f64) {
	let bid_forecast = ForecastSchema {
		area_uuid: world.buyer_id.clone(),
		community_uuid: "community1".to_string(),
		time_slot: world.target_delivery_time,
		creation_time: 1,
		energy_kwh: energy,
		confidence: 1.0,
	};

	let offer_forecast = ForecastSchema {
		area_uuid: world.seller_id.clone(),
		community_uuid: "community1".to_string(),
		time_slot: world.target_delivery_time,
		creation_time: 1,
		energy_kwh: -energy,
		confidence: 1.0,
	};

	let market_id = world.generate_market_id(MarketType::Spot);
	world.last_market_id = Some(market_id);

	let orderbook_url = std::env::var("ORDERBOOK_SERVICE_URL")
		.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url));
	adapter.forward_forecast(
		vec![bid_forecast.clone(), offer_forecast.clone()]
	).await.expect("Forecast forwarding failed.");
	let market = adapter.get_or_create_market_topology(
		ExternalCommunityTopology {
			community_uuid: "community1".to_string(),
			community_name: "Test Community".to_string(),
			areas: vec![
				ExternalAreaTopology {
					area_uuid: world.buyer_id.clone(),
					area_name: "buyer".to_string(),
				},
				ExternalAreaTopology {
					area_uuid: world.seller_id.clone(),
					area_name: "seller".to_string(),
				},
			]
		}, world.target_delivery_time
	).await.expect("Topology forwarding failed.");

	world.bid_forecast = Some(bid_forecast.clone());
	world.offer_forecast = Some(offer_forecast.clone());
	world.topology_schema = Some(market.clone());

}


#[when("the Market Orchestrator opens the Spot market for the next delivery slot")]
async fn wait_for_market_to_open(world: &mut MyWorld) {
	info!("Waiting for the Market Orchestrator to open the Spot market...");
	let now = Utc::now();
	let target_timeslot_start = (
		(now + ChronoDuration::hours(2)).timestamp() as u64 / GlobalConstants.TIME_SLOT_SEC) * GlobalConstants.TIME_SLOT_SEC;

	world.target_delivery_time = target_timeslot_start;
	let market_id = world.generate_market_id(MarketType::Spot);

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..40 {
		info!("Waiting for MarketStatusUpdated event for market {}... Check {}/40",
			market_id, i + 1);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block from node")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();

		let event = events
			.find_first::<gsy_node::orderbook_registry::events::MarketStatusUpdated>()
			.unwrap();

		if let Some(e) = event {
			info!("-> Found event: MarketStatusUpdated({:?}, {})", e.0, e.1);
			if e.0 == market_id && e.1 == true {
				info!("✅ MarketStatusUpdated(true) event found for market {:?}", market_id);
				sleep(Duration::from_secs(6)).await;
				return;
			}
		}
	}
	panic!("Timeout: Did not find MarketStatusUpdated(true) event for the target market.");
}

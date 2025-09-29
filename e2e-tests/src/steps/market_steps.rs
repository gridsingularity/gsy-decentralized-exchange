use crate::world::{gsy_node, MyWorld};
use cucumber::when;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[when("the Market Orchestrator opens the Spot market for the next delivery slot")]
async fn wait_for_market_to_open(world: &mut MyWorld) {
	info!("Waiting for the Market Orchestrator to open the Spot market...");
	let market_id = world.generate_market_id("Spot");

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..20 {
		info!("Waiting for MarketStatusUpdated event... Check {}/20", i + 1);

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

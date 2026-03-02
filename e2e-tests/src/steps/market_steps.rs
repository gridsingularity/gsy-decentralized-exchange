use crate::world::{gsy_node, MyWorld};
use chrono::{Duration as ChronoDuration, Utc};
use cucumber::when;
use gsy_community_client::external_api::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::string_to_h256;
use gsy_offchain_primitives::{constants::GLOBAL_CONSTANTS, MarketType};
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[when(
    regex = r#"^the community topology and forecasts of (\d+) energy are submitted by "([^"]*)", "([^"]*)", and "([^"]*)"$"#
)]
async fn submit_topology_forecasts_three_users(
    world: &mut MyWorld,
    energy: f64,
    user1: String,
    user2: String,
    user3: String,
) {
    let orderbook_url = std::env::var("ORDERBOOK_SERVICE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url));

    let areas = vec![
        ExternalAreaTopology {
            area_uuid: format!("area{}", user1),
            area_name: user1.clone(),
        },
        ExternalAreaTopology {
            area_uuid: format!("area{}", user2),
            area_name: user2.clone(),
        },
        ExternalAreaTopology {
            area_uuid: format!("area{}", user3),
            area_name: user3.clone(),
        },
    ];

    let market = adapter
        .get_or_create_market_topology(
            ExternalCommunityTopology {
                community_uuid: "community1".to_string(),
                community_name: "Test Community".to_string(),
                areas: areas.clone(),
            },
            world.target_delivery_time,
        )
        .await
        .expect("Topology forwarding failed.");

    world.topology_schema = Some(market.clone());
    world.last_market_id = Some(string_to_h256(market.market_id.clone()));

    for area in market.community_areas {
        if area.area_uuid == format!("area{}", user1) {
            world.buyer_hash = Some(area.area_hash.clone());
        }
        if area.area_uuid == format!("area{}", user2) {
            world.seller_hash = Some(area.area_hash.clone());
        }
    }

    let mut forecasts = Vec::new();
    for (i, area) in areas.iter().enumerate() {
        let energy_val = if i == 0 { energy } else { -energy };
        forecasts.push(ForecastSchema {
            area_uuid: area.area_uuid.clone(),
            area_hash: world
                .topology_schema
                .as_ref()
                .unwrap()
                .community_areas
                .iter()
                .find(|a| a.area_uuid == area.area_uuid)
                .unwrap()
                .area_hash
                .clone(),
            community_uuid: "community1".to_string(),
            time_slot: world.target_delivery_time,
            creation_time: 1,
            energy_kwh: energy_val,
            confidence: 1.0,
        });
    }

    adapter
        .forward_forecast(forecasts.clone())
        .await
        .expect("Forecast forwarding failed.");

    world.bid_forecast = Some(forecasts[0].clone());
    world.offer_forecast = Some(forecasts[1].clone());
}

#[when(regex = r#"^the community topology and forecasts of (\d+) energy are submitted$"#)]
async fn submit_topology_forecasts(world: &mut MyWorld, energy: f64) {
    submit_topology_forecasts_three_users(
        world,
        energy,
        "alice".to_string(),
        "bob".to_string(),
        "charlie".to_string(),
    )
    .await;
}

#[when("the Market Orchestrator opens the Spot market for the next delivery slot")]
async fn wait_for_market_to_open(world: &mut MyWorld) {
    info!("Waiting for the Market Orchestrator to open the Spot market...");
    let now = Utc::now();
    let target_timeslot_start = ((now + ChronoDuration::hours(2)).timestamp() as u64
        / GLOBAL_CONSTANTS.time_slot_sec)
        * GLOBAL_CONSTANTS.time_slot_sec;

    world.target_delivery_time = target_timeslot_start;
    let market_id = world.generate_market_id(MarketType::Spot);

    let mut block_sub = world
        .subxt_client
        .blocks()
        .subscribe_finalized()
        .await
        .expect("Failed to subscribe to finalized blocks");

    for i in 0..40 {
        info!(
            "Waiting for MarketStatusUpdated event for market {}... Check {}/40",
            market_id,
            i + 1
        );

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
                info!(
                    "✅ MarketStatusUpdated(true) event found for market {:?}",
                    market_id
                );
                sleep(Duration::from_secs(6)).await;
                return;
            }
        }
    }
    panic!("Timeout: Did not find MarketStatusUpdated(true) event for the target market.");
}

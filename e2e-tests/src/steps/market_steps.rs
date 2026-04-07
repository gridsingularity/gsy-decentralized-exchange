use crate::world::MyWorld;
use cucumber::when;
use ethers::prelude::*;
use gsy_community_client::external_api::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::MarketType;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

abigen!(
    MarketControllerContract,
    r#"[
        function isMarketOpen(bytes32 marketId) external view returns (bool)
    ]"#
);

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
    let adapter = AreaMarketInfoAdapter::new(Some(world.orderbook_service_url.clone()));

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
        .expect("Topology forwarding failed");

    let market_id = H256::from_str(market.market_id.as_str())
        .expect("Invalid market id in topology")
        .to_fixed_bytes();

    world.last_market_id = Some(market_id);
    world.topology_schema = Some(market.clone());

    let mut forecasts = Vec::new();
    for (index, area) in areas.iter().enumerate() {
        let energy_value = if index == 0 { energy } else { -energy };
        forecasts.push(ForecastSchema {
            area_uuid: area.area_uuid.clone(),
            community_uuid: "community1".to_string(),
            time_slot: world.target_delivery_time,
            creation_time: 1,
            energy_kwh: energy_value,
            confidence: 1.0,
        });
    }

    adapter
        .forward_forecast(forecasts.clone())
        .await
        .expect("Forecast forwarding failed");

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
    let (_, next_timeslot) = get_last_and_next_timeslot();
    world.target_delivery_time = next_timeslot;

    let market_id = world.generate_market_id(MarketType::Spot);
    world.last_market_id = Some(market_id);

    info!(
        "Waiting for MarketController to open market {:?} for timeslot {}",
        H256::from(market_id),
        world.target_delivery_time
    );

    let market_controller =
        MarketControllerContract::new(world.market_controller_address, world.provider.clone());

    for attempt in 0..60 {
        let is_open = market_controller
            .is_market_open(market_id)
            .call()
            .await
            .expect("Failed to read market status from MarketController");

        if is_open {
            info!("Spot market opened after {} checks", attempt + 1);
            return;
        }

        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: Spot market {:?} was not opened by orchestrator",
        H256::from(market_id)
    );
}

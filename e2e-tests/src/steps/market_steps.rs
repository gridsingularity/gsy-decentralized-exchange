use crate::world::MyWorld;
use cucumber::when;
use ethers::prelude::*;
use gsy_community_client::external_api::ExternalFacilityTopology;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use primitives::db_api_schema::{
    profiles::ForecastSchema,
    // ids::IdType
};
// use primitives::utils::{parse_uuid_or_hex_bytes16,bytes16_to_string, generate_market_id};
use primitives::MarketType;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;
// use uuid::Uuid;
// use primitives::ewds::get_onchain_id_via_ewds;
use primitives::utils::create_encrypted_bytes16_from_string;

abigen!(
    MarketControllerContract,
    r#"[
        function isMarketOpen(bytes16 marketId) external view returns (bool)
    ]"#
);

#[when(
    expr = "the community market and forecasts of {float} energy are submitted by {string}, {string}, and {string}"
)]
async fn submit_market_forecasts_three_users(
    world: &mut MyWorld,
    energy: f64,
    user1: String,
    user2: String,
    user3: String,
) {
    let adapter = AreaMarketInfoAdapter::new(Some(world.offchain_storage_url.clone()));

    let facilities = vec![
        ExternalFacilityTopology {
            facility_id: format!("area{}", user1),
            facility_name: user1.clone(),
        },
        ExternalFacilityTopology {
            facility_id: format!("area{}", user2),
            facility_name: user2.clone(),
        },
        ExternalFacilityTopology {
            facility_id: format!("area{}", user3),
            facility_name: user3.clone(),
        },
    ];
    let market_offchain_id = world
        .last_market_offchain_id
        .clone()
        .expect("last_market_offchain_id must be set before create_market");
    let market = adapter
        .create_market(market_offchain_id, "community1".to_string(), world.target_delivery_time)
        .await
        .unwrap_or_else(|| {
            panic!(
                "market_creation_failed community=community1 time_slot={} offchain_storage_url={}",
                world.target_delivery_time, world.offchain_storage_url
            )
        });

    world.market_schema = Some(market.clone());

    let mut forecasts = Vec::new();
    for (index, facility) in facilities.iter().enumerate() {
        let energy_value = if index == 0 { energy } else { -energy };
        forecasts.push(ForecastSchema {
            facility_id: facility.facility_id.clone(),
            community_uuid: "community1".to_string(),
            time_slot: world.target_delivery_time,
            creation_time: 1,
            energy_kwh: energy_value,
            confidence: 1.0,
        });
    }

    // send forecasts to offchain storage
    adapter
        .forward_forecast(forecasts.clone())
        .await
        .expect("Forecast forwarding failed");

    world.bid_forecast = Some(forecasts[0].clone());
    world.offer_forecast = Some(forecasts[1].clone());
    world.facilities_topology = facilities;
}

#[when(expr = "the community market and forecasts of {float} energy are submitted")]
async fn submit_market_forecasts(world: &mut MyWorld, energy: f64) {
    submit_market_forecasts_three_users(
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

    let offchain_market_id = format!("{} {}", MarketType::Spot.as_str(), world.target_delivery_time);

    let onchain_market_id = create_encrypted_bytes16_from_string(&offchain_market_id);


    // let offchain_market_id_bytes16 = generate_market_id(
    //     MarketType::Spot, world.target_delivery_time);
    // let offchain_market_id = bytes16_to_string(
    //     &offchain_market_id_bytes16).expect("failed to convert market id to string");
    // let onchain_market_id = get_onchain_id_via_ewds(
    //     offchain_market_id.clone(),
    //     IdType::MarketId
    // ).await.ok().expect("market did not open");
    // let last_market_id_byte16 = parse_uuid_or_hex_bytes16(
    //     &onchain_market_id).expect("Invalid market id in topology");
    world.last_market_id = Some(onchain_market_id);
    world.last_market_offchain_id = Some(offchain_market_id.clone());

    info!(
        "Waiting for MarketController to open market {:?} for timeslot {}",
        offchain_market_id,
        world.target_delivery_time
    );

    let market_controller =
        MarketControllerContract::new(world.market_controller_address, world.provider.clone());

    for attempt in 0..60 {
        let is_open = market_controller
            .is_market_open(onchain_market_id)
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
        world.last_market_offchain_id
    );
}

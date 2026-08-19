use crate::world::MyWorld;
use cucumber::when;
use ethers::prelude::*;
use gsy_community_client::external_api::ExternalFacilityTopology;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::ewds::dto::EwdsCommunityDto;
use primitives::ewds::{EwdsClient, EwdsOperation};
use primitives::utils::{generate_market_id, parse_uuid_or_hex_bytes16};
use primitives::{MarketType, MatchingAlgorithm};
use std::env;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

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

    let market = adapter
        .create_market(
            world.community_id.clone(),
            world.target_delivery_time,
            matching_algorithm_from_env(),
        )
        .await
        .unwrap_or_else(|| {
            panic!(
                "market_creation_failed community={} time_slot={} offchain_storage_url={}",
                world.community_id, world.target_delivery_time, world.offchain_storage_url
            )
        });

    let market_id = parse_uuid_or_hex_bytes16(market.market_id.as_str())
        .expect("Invalid market id in topology");

    world.last_market_id = Some(market_id);
    world.market_schema = Some(market.clone());

    let mut forecasts = Vec::new();
    for (index, facility) in facilities.iter().enumerate() {
        let energy_value = if index == 0 { energy } else { -energy };
        forecasts.push(ForecastSchema {
            facility_id: facility.facility_id.clone(),
            community_uuid: world.community_id.clone(),
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

fn matching_algorithm_from_env() -> MatchingAlgorithm {
    let configured_value =
        env::var("MATCHING_ALGORITHM").unwrap_or_else(|_| MatchingAlgorithm::default().to_string());
    MatchingAlgorithm::from_str(configured_value.as_str())
        .unwrap_or_else(|error| panic!("Invalid MATCHING_ALGORITHM: {}", error))
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
    upsert_community(world).await;

    let (_, next_timeslot) = get_last_and_next_timeslot();
    world.target_delivery_time = next_timeslot;

    let market_id = generate_market_id(
        world.community_id.as_str(),
        MarketType::Spot,
        world.target_delivery_time,
    );
    world.last_market_id = Some(market_id);

    info!(
        "Waiting for MarketController to open market {:?} for timeslot {}",
        hex::encode(market_id),
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
        hex::encode(market_id)
    );
}

async fn upsert_community(world: &MyWorld) {
    let community = EnergyCommunitySchema {
        community_id: world.community_id.clone(),
        community_name: "E2E Community".to_string(),
        sites: vec!["E2E Site".to_string()],
    };

    let transport = env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .unwrap_or_else(|_| "http".to_string())
        .to_ascii_lowercase();
    match transport.as_str() {
        "http" => upsert_community_via_http(world, &community).await,
        "ewds" => upsert_community_via_ewds(&community).await,
        _ => panic!(
            "Unsupported OFFCHAIN_STORAGE_TRANSPORT '{}'; expected http or ewds",
            transport
        ),
    }
}

async fn upsert_community_via_http(world: &MyWorld, community: &EnergyCommunitySchema) {
    let response = world
        .http_client
        .post(format!("{}/communities", world.offchain_storage_url))
        .json(community)
        .send()
        .await
        .expect("Failed to upsert E2E community");

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        panic!("Community upsert failed with status {}: {}", status, body);
    }
}

async fn upsert_community_via_ewds(community: &EnergyCommunitySchema) {
    let client = EwdsClient::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000);
    let payload = serde_json::to_value(EwdsCommunityDto::from(community.clone()))
        .expect("Failed to serialize E2E community");
    let saved = client
        .query::<EwdsCommunityDto>(EwdsOperation::CommunityUpsert, payload)
        .await
        .expect("Failed to upsert E2E community through EWDS");

    assert!(
        saved
            .iter()
            .any(|item| item.community_id == community.community_id),
        "EWDS community upsert response did not contain {}",
        community.community_id
    );
}

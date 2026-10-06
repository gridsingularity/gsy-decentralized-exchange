use crate::world::MyWorld;
use cucumber::{then, when};
use ethers::prelude::*;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::market::{MarketChainRecord, MarketSchema};
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::ewds::dto::EwdsCommunityDto;
use primitives::ewds::{EwdsClient, EwdsOperation};
use primitives::offchain_storage::OffchainStorageClient;
use primitives::utils::{bytes16_to_hex, generate_market_id};
use primitives::{MarketType, MatchingAlgorithm};
use std::env;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

abigen!(
    MarketControllerContract,
    r#"[
        function isMarketOpen(bytes16 marketId) external view returns (bool)
        struct Market { bytes16 communityId; uint64 openingTime; uint64 closingTime; uint64 deliveryStartTime; uint64 deliveryEndTime; uint64 createdAt; uint8 marketType; uint8 matchingAlgorithm; }
        function getMarket(bytes16 marketId) external view returns (Market)
    ]"#
);

const MARKET_POLL_ATTEMPTS: usize = 60;
const MARKET_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[when(expr = "forecasts of {float} energy are submitted by {string}, {string}, and {string}")]
async fn submit_market_forecasts_three_users(
    world: &mut MyWorld,
    energy: f64,
    user1: String,
    user2: String,
    user3: String,
) {
    let adapter = AreaMarketInfoAdapter::new(Some(world.offchain_storage_url.clone()));

    let facilities = vec![
        FacilitySchema {
            facility_id: format!("area{}", user1.clone()),
            facility_name: format!("area{}", user1.clone()),
            site_id: "12345".to_string(),
            owner_id: user1.clone(),
        },
        FacilitySchema {
            facility_id: format!("area{}", user2.clone()),
            facility_name: format!("area{}", user2.clone()),
            site_id: "12345".to_string(),
            owner_id: user2.clone(),
        },
        FacilitySchema {
            facility_id: format!("area{}", user3.clone()),
            facility_name: format!("area{}", user3.clone()),
            site_id: "12345".to_string(),
            owner_id: user3.clone(),
        },
    ];
    world.create_facilities(facilities.clone()).await;

    // The market comes from the orchestrator via the chain; see wait_for_market.
    assert!(
        world.market_schema.is_some(),
        "No market: the market step must run before forecasts are submitted"
    );

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

#[when(expr = "forecasts of {float} energy are submitted")]
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

#[when("the Market Orchestrator opens the Spot market for the current delivery slot")]
async fn wait_for_market_to_open(world: &mut MyWorld) {
    world.community_id = unique_community_id();
    upsert_default_community(world).await;

    // The orchestrator creates the current delivery slot's markets for a new
    // community; markets of later slots that have already opened are not created.
    let (current_timeslot, _) = get_last_and_next_timeslot();
    world.target_delivery_time = current_timeslot;

    let market_id = generate_market_id(
        world.community_id.as_str(),
        MarketType::Spot,
        world.target_delivery_time,
    );
    let market = wait_for_market(world, market_id).await;

    world.last_market_id = Some(market_id);
    world.market_schema = Some(market);
}

/// Waits until the market is open on-chain and stored off-chain, and checks
/// that the stored market mirrors the on-chain record.
async fn wait_for_market(world: &MyWorld, market_id: [u8; 16]) -> MarketSchema {
    let market_id_hex = bytes16_to_hex(market_id);
    info!(
        "Waiting for market {} (timeslot {}) on-chain and in off-chain storage",
        market_id_hex, world.target_delivery_time
    );

    let market_controller =
        MarketControllerContract::new(world.market_controller_address, world.provider.clone());
    let storage = OffchainStorageClient::from_env("E2E_TESTS_CLIENT_ID", "e2e_tests");

    for attempt in 0..MARKET_POLL_ATTEMPTS {
        let is_open = market_controller
            .is_market_open(market_id)
            .call()
            .await
            .expect("Failed to read market status from MarketController");
        let stored = if is_open {
            storage
                .fetch_market(market_id_hex.as_str())
                .await
                .expect("Failed to read market from off-chain storage")
        } else {
            None
        };

        if let Some(stored) = stored {
            info!(
                "Market {} open and stored after {} checks",
                market_id_hex,
                attempt + 1
            );
            let (
                community_id,
                opening_time,
                closing_time,
                delivery_start_time,
                delivery_end_time,
                created_at,
                market_type,
                matching_algorithm,
            ) = market_controller
                .get_market(market_id)
                .call()
                .await
                .expect("Failed to read market from MarketController");
            let expected = MarketSchema::try_from(MarketChainRecord {
                market_id,
                community_id,
                opening_time,
                closing_time,
                delivery_start_time,
                delivery_end_time,
                market_type,
                matching_algorithm,
                created_at,
            })
            .expect("Invalid on-chain market record");
            assert_eq!(
                stored, expected,
                "Stored market differs from the on-chain record"
            );
            assert_eq!(stored.matching_algorithm, matching_algorithm_from_env());
            return stored;
        }

        sleep(MARKET_POLL_INTERVAL).await;
    }

    panic!(
        "Timeout: market {} was not created by the orchestrator",
        market_id_hex
    );
}

#[when("two communities are submitted to off-chain storage")]
async fn submit_two_communities(world: &mut MyWorld) {
    world.community_id = unique_community_id();
    world.secondary_community_id = unique_community_id();

    let communities = [
        EnergyCommunitySchema {
            community_id: world.community_id.clone(),
            community_name: format!("E2E Community {}", world.community_id),
            sites: vec![format!("E2E Site {}", world.community_id)],
        },
        EnergyCommunitySchema {
            community_id: world.secondary_community_id.clone(),
            community_name: format!("E2E Secondary Community {}", world.secondary_community_id),
            sites: vec![format!(
                "E2E Secondary Site {}",
                world.secondary_community_id
            )],
        },
    ];

    for community in &communities {
        upsert_community(world, community).await;
    }

    let (current_timeslot, _) = get_last_and_next_timeslot();
    world.target_delivery_time = current_timeslot;
}

fn unique_community_id() -> String {
    Uuid::new_v4().to_string()
}

#[then("the Market Orchestrator opens a distinct Spot market for each community")]
async fn wait_for_two_community_markets(world: &mut MyWorld) {
    let market_ids = [
        generate_market_id(
            world.community_id.as_str(),
            MarketType::Spot,
            world.target_delivery_time,
        ),
        generate_market_id(
            world.secondary_community_id.as_str(),
            MarketType::Spot,
            world.target_delivery_time,
        ),
    ];

    assert_ne!(
        market_ids[0], market_ids[1],
        "Different communities generated the same Spot market id"
    );

    for market_id in market_ids {
        wait_for_market(world, market_id).await;
    }
    info!("Distinct Spot markets for both communities are open and stored");
    world.community_market_ids = Some(market_ids);
}

async fn upsert_default_community(world: &MyWorld) {
    let community = EnergyCommunitySchema {
        community_id: world.community_id.clone(),
        community_name: format!("E2E Community {}", world.community_id),
        sites: vec!["E2E Site".to_string()],
    };

    upsert_community(world, &community).await;
}

async fn upsert_community(world: &MyWorld, community: &EnergyCommunitySchema) {
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

use crate::utils::indexed_bytes16_topic;
use crate::world::MyWorld;
use cucumber::{then, when};
use ethers::prelude::*;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use gsy_market_orchestrator::chain_connector::{
    GsyMarketOrchestratorNodeClient, MarketChainClient,
};
use gsy_market_orchestrator::config::{get_config, MARKET_RULES};
use gsy_market_orchestrator::orchestrator::new_market;
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::market::{MarketChainRecord, MarketSchema};
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::ewds::dto::{EwdsCommunityDto, EwdsEventEnvelope};
use primitives::ewds::{EwdsClient, EwdsEventType, EwdsOperation};
use primitives::offchain_storage::OffchainStorageClient;
use primitives::utils::{
    bytes16_to_hex, epoch_to_rfc3339, generate_market_id, parse_uuid_or_hex_bytes16,
};
use primitives::MarketType;
use serde_json::json;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

abigen!(
    MarketControllerContract,
    r#"[
        function isMarketOpen(bytes16 marketId) external view returns (bool)
        event NewMarketCreated(bytes16 indexed marketId, bytes16 indexed communityId, uint64 openingTime, uint64 closingTime, uint64 deliveryStartTime, uint64 deliveryEndTime, uint8 marketType, uint8 matchingAlgorithm, uint64 createdAt)
    ]"#
);

const MARKET_POLL_ATTEMPTS: usize = 60;
const MARKET_POLL_INTERVAL: Duration = Duration::from_secs(2);
const EWDS_COMMUNITY_POLL_ATTEMPTS: usize = 30;

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
            site_id: "site-12345".to_string(),
            owner_id: user1.clone(),
        },
        FacilitySchema {
            facility_id: format!("area{}", user2.clone()),
            facility_name: format!("area{}", user2.clone()),
            site_id: "site-12345".to_string(),
            owner_id: user2.clone(),
        },
        FacilitySchema {
            facility_id: format!("area{}", user3.clone()),
            facility_name: format!("area{}", user3.clone()),
            site_id: "site-12345".to_string(),
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

    // The orchestrator creates every market of a new community that has not
    // closed. The e2e Spot market closes 30 min after delivery start, so the
    // current delivery slot's market is still open.
    let (current_timeslot, _) = get_last_and_next_timeslot();
    world.target_delivery_time = current_timeslot;
    create_spot_market_with_scenario_algorithm(world, world.community_id.as_str()).await;
    upsert_default_community(world).await;

    let market_id = generate_market_id(
        world.community_id.as_str(),
        MarketType::Spot,
        world.target_delivery_time,
    )
    .unwrap();
    let market = wait_for_market(world, market_id).await;
    wait_for_orchestrated_markets(world, world.community_id.as_str(), market_id).await;

    world.last_market_id = Some(market_id);
    world.market_schema = Some(market);
}

/// Waits until the market is open on-chain and stored off-chain, and checks
/// that the stored market mirrors its `NewMarketCreated` event.
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
            let events = market_controller
                .event::<NewMarketCreatedFilter>()
                .from_block(0u64)
                .topic1(indexed_bytes16_topic(market_id))
                .query()
                .await
                .expect("Failed to query NewMarketCreated events");
            assert_eq!(events.len(), 1, "Expected one NewMarketCreated event");
            let event = &events[0];
            let expected = MarketSchema::try_from(MarketChainRecord {
                market_id: event.market_id,
                community_id: event.community_id,
                opening_time: event.opening_time,
                closing_time: event.closing_time,
                delivery_start_time: event.delivery_start_time,
                delivery_end_time: event.delivery_end_time,
                market_type: event.market_type,
                matching_algorithm: event.matching_algorithm,
                created_at: event.created_at,
            })
            .expect("Invalid NewMarketCreated event");
            assert_eq!(
                stored, expected,
                "Stored market differs from the NewMarketCreated event"
            );
            assert_eq!(
                stored.matching_algorithm, world.matching_algorithm,
                "Market {} does not use the scenario's matching algorithm",
                market_id_hex
            );
            return stored;
        }

        sleep(MARKET_POLL_INTERVAL).await;
    }

    panic!(
        "Timeout: market {} was not opened and stored",
        market_id_hex
    );
}

/// Waits until the orchestrator has created a market of the community other
/// than `spot_market_id`. The orchestrator creates a new community's markets in
/// one transaction; once that block is mined, it cannot fall into a scenario's
/// order window and move orders past a matching boundary. When the
/// orchestrator created the Spot market itself, it is already mined.
async fn wait_for_orchestrated_markets(
    world: &MyWorld,
    community_id: &str,
    spot_market_id: [u8; 16],
) {
    let community = parse_uuid_or_hex_bytes16(community_id).expect("E2E community IDs are UUIDs");
    let market_controller =
        MarketControllerContract::new(world.market_controller_address, world.provider.clone());
    for _ in 0..MARKET_POLL_ATTEMPTS {
        let events = market_controller
            .event::<NewMarketCreatedFilter>()
            .from_block(0u64)
            .topic2(indexed_bytes16_topic(community))
            .query()
            .await
            .expect("Failed to query NewMarketCreated events");
        if events.iter().any(|event| event.market_id != spot_market_id) {
            return;
        }
        sleep(MARKET_POLL_INTERVAL).await;
    }

    panic!(
        "Timeout: the orchestrator created no other market for community {}",
        community_id
    );
}

#[when("two communities are submitted to off-chain storage")]
async fn submit_two_communities(world: &mut MyWorld) {
    world.community_id = unique_community_id();
    world.secondary_community_id = unique_community_id();
    let (current_timeslot, _) = get_last_and_next_timeslot();
    world.target_delivery_time = current_timeslot;
    for community_id in [&world.community_id, &world.secondary_community_id] {
        create_spot_market_with_scenario_algorithm(world, community_id).await;
    }

    let communities = [
        EnergyCommunitySchema {
            community_id: world.community_id.clone(),
            community_name: format!("E2E Community {}", world.community_id),
            sites: vec!["site-12345".to_string()],
        },
        EnergyCommunitySchema {
            community_id: world.secondary_community_id.clone(),
            community_name: format!("E2E Secondary Community {}", world.secondary_community_id),
            sites: vec!["site-12346".to_string()],
        },
    ];

    for community in &communities {
        upsert_community(world, community).await;
    }
    let facilities = ["alice", "bob", "charlie"]
        .into_iter()
        .flat_map(|user| {
            [("", "site-12345"), ("secondary-", "site-12346")]
                .into_iter()
                .map(move |(prefix, site)| FacilitySchema {
                    facility_id: format!("{prefix}area{user}"),
                    facility_name: format!("{prefix}area{user}"),
                    site_id: site.to_string(),
                    owner_id: format!("{prefix}{user}"),
                })
        })
        .collect();
    world.create_facilities(facilities).await;
}

fn unique_community_id() -> String {
    Uuid::new_v4().to_string()
}

/// The orchestrator creates every Spot market with the algorithm of its Spot
/// rule (`pay_as_clear`). For a scenario with another algorithm, this creates
/// the community's Spot market for the target slot itself, from the same rule
/// but with the scenario's algorithm. It must run before the community is
/// registered, so the orchestrator finds the market and skips it.
async fn create_spot_market_with_scenario_algorithm(world: &MyWorld, community_id: &str) {
    let spot_rule = MARKET_RULES
        .iter()
        .find(|rule| rule.market_type == MarketType::Spot)
        .expect("The market orchestrator has no Spot market rule");
    if spot_rule.matching_algorithm == world.matching_algorithm {
        return;
    }

    let market_id =
        generate_market_id(community_id, MarketType::Spot, world.target_delivery_time).unwrap();
    let community = parse_uuid_or_hex_bytes16(community_id).expect("E2E community IDs are UUIDs");
    let mut market = new_market(market_id, community, spot_rule, world.target_delivery_time);
    market.matching_algorithm = world.matching_algorithm.to_evm();

    let config = get_config().expect("Invalid market orchestrator configuration");
    GsyMarketOrchestratorNodeClient::new(&config)
        .await
        .expect("Failed to connect with the market orchestrator's key")
        .create_markets(vec![market])
        .await
        .expect("Failed to create the Spot market");
    info!(
        "Created {} Spot market {} for community {}",
        world.matching_algorithm,
        bytes16_to_hex(market_id),
        community_id
    );
}

#[then("the Market Orchestrator opens a distinct Spot market for each community")]
async fn wait_for_two_community_markets(world: &mut MyWorld) {
    let market_ids = [
        generate_market_id(
            world.community_id.as_str(),
            MarketType::Spot,
            world.target_delivery_time,
        )
        .unwrap(),
        generate_market_id(
            world.secondary_community_id.as_str(),
            MarketType::Spot,
            world.target_delivery_time,
        )
        .unwrap(),
    ];

    assert_ne!(
        market_ids[0], market_ids[1],
        "Different communities generated the same Spot market id"
    );

    let community_ids = [&world.community_id, &world.secondary_community_id];
    for (community_id, market_id) in community_ids.into_iter().zip(market_ids) {
        wait_for_market(world, market_id).await;
        wait_for_orchestrated_markets(world, community_id, market_id).await;
    }
    info!("Distinct Spot markets for both communities are open and stored");
    world.community_market_ids = Some(market_ids);
}

async fn upsert_default_community(world: &MyWorld) {
    let community = EnergyCommunitySchema {
        community_id: world.community_id.clone(),
        community_name: format!("E2E Community {}", world.community_id),
        sites: vec!["site-12345".to_string()],
    };

    upsert_community(world, &community).await;
}

pub(crate) async fn upsert_community(world: &MyWorld, community: &EnergyCommunitySchema) {
    let transport = env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .unwrap_or_else(|_| "http".to_string())
        .to_ascii_lowercase();
    match transport.as_str() {
        "http" => upsert_community_via_http(world, &community).await,
        "ewds" => publish_community_via_ewds(&community).await,
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

async fn publish_community_via_ewds(community: &EnergyCommunitySchema) {
    let client = EwdsClient::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000);
    let community = EwdsCommunityDto::from(community.clone());
    let event = EwdsEventEnvelope {
        event_id: Uuid::new_v4().to_string(),
        event_type: EwdsEventType::Community,
        occurred_at: epoch_to_rfc3339(get_current_timestamp_in_secs()),
        data: vec![&community],
    };
    client
        .publish_event(&event)
        .await
        .expect("Failed to publish E2E community through EWDS");

    // Events get no reply, so wait until the off-chain storage has stored the community.
    for _ in 0..EWDS_COMMUNITY_POLL_ATTEMPTS {
        let communities = client
            .query::<EwdsCommunityDto>(EwdsOperation::CommunitiesQuery, json!({}))
            .await
            .expect("Failed to query E2E communities through EWDS");
        if communities.contains(&community) {
            return;
        }
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: community {} was not stored after publishing it through EWDS",
        community.community_id
    );
}

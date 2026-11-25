use primitives::utils::EPOCH_RFC3339;
use crate::utils::indexed_bytes16_topic;
use crate::world::{MyWorld, PRIMARY_SITE_ID};
use cucumber::{then, when};
use ethers::prelude::*;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::market::{MarketChainRecord, MarketSchema};
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::ewds::dto::{EwdsCommunityDto, EwdsEventEnvelope};
use primitives::ewds::{EwdsClient, EwdsEventType, EwdsOperation};
use primitives::offchain_storage::OffchainStorageClient;
use primitives::utils::{
    bytes16_to_uuid_string, derive_onchain_id, epoch_to_rfc3339, hex_to_bytes16,
};
use primitives::{MarketType, MatchingAlgorithm};
use serde_json::json;
use std::env;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

abigen!(
    MarketControllerContract,
    r#"[
        struct MarketKey { bytes16 communityId; uint8 marketType; uint64 deliveryStartTime; }
        function getMarketIds(MarketKey[] keys) external view returns (bytes16[])
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
            facility_id: world.facility_id_for_user(&user1),
            facility_name: format!("area{}", user1.clone()),
            site_id: PRIMARY_SITE_ID.to_string(),
            owner_id: user1.clone(),
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
        FacilitySchema {
            facility_id: world.facility_id_for_user(&user2),
            facility_name: format!("area{}", user2.clone()),
            site_id: PRIMARY_SITE_ID.to_string(),
            owner_id: user2.clone(),
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
        FacilitySchema {
            facility_id: world.facility_id_for_user(&user3),
            facility_name: format!("area{}", user3.clone()),
            site_id: PRIMARY_SITE_ID.to_string(),
            owner_id: user3.clone(),
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
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

#[when(expr = "forecasts of {float} energy are submitted for facilities with uppercase UUIDs")]
async fn submit_uuid_facility_forecasts(world: &mut MyWorld, energy: f64) {
    world.facilities_topology = ["alice", "bob", "charlie"]
        .into_iter()
        .enumerate()
        .map(|(index, user)| FacilitySchema {
            valid_from: primitives::utils::EPOCH_RFC3339.to_string(),
            valid_until: None,
            facility_id: format!("ABCDEFAB-ABCD-4BCD-ABCD-{:012X}", index + 1),
            facility_name: format!("area{user}"),
            site_id: PRIMARY_SITE_ID.into(),
            owner_id: user.into(),
        })
        .collect();
    submit_market_forecasts(world, energy).await;
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

    // The orchestrator creates every market of a new community that has not
    // closed. The e2e Spot market closes 30 min after delivery start, so the
    // current delivery slot's market is still open.
    let (current_timeslot, _) = get_last_and_next_timeslot();
    world.target_delivery_time = current_timeslot;

    let (market_id, market) = wait_for_market(world, &world.community_id).await;

    world.last_market_id = Some(market_id);
    world.market_schema = Some(market);
}

fn market_key(community_id: &str, delivery_start_time: u64) -> MarketKey {
    MarketKey {
        community_id: hex_to_bytes16(&derive_onchain_id(community_id))
            .expect("Derived community identity must fit bytes16"),
        market_type: MarketType::Spot.to_evm(),
        delivery_start_time,
    }
}

async fn lookup_market_id<M: Middleware>(
    market_controller: &MarketControllerContract<M>,
    key: &MarketKey,
) -> Option<[u8; 16]> {
    let market_ids = market_controller
        .get_market_ids(vec![key.clone()])
        .call()
        .await
        .expect("Failed to look up market from MarketController");
    assert_eq!(market_ids.len(), 1, "Expected one getMarketIds result");
    (market_ids[0] != [0; 16]).then_some(market_ids[0])
}

/// Resolves the orchestrator's random ID by tuple, then waits for the market to
/// be open and indexed, checking storage against its `NewMarketCreated` event.
async fn wait_for_market(world: &MyWorld, community_id: &str) -> ([u8; 16], MarketSchema) {
    let key = market_key(community_id, world.target_delivery_time);
    info!(
        "Waiting for Spot market for community {} (timeslot {}) on-chain and in off-chain storage",
        community_id, world.target_delivery_time
    );

    let market_controller =
        MarketControllerContract::new(world.market_controller_address, world.provider.clone());
    let offchain_storage_client =
        OffchainStorageClient::from_env("E2E_TESTS_CLIENT_ID", "e2e_tests");

    for attempt in 0..MARKET_POLL_ATTEMPTS {
        let Some(market_id) = lookup_market_id(&market_controller, &key).await else {
            sleep(MARKET_POLL_INTERVAL).await;
            continue;
        };
        let market_id_string = bytes16_to_uuid_string(market_id);
        let is_open = market_controller
            .is_market_open(market_id)
            .call()
            .await
            .expect("Failed to read market status from MarketController");
        let stored = if is_open {
            offchain_storage_client
                .fetch_market(market_id_string.as_str())
                .await
                .expect("Failed to read market from off-chain storage")
        } else {
            None
        };

        if let Some(stored) = stored {
            info!(
                "Market {} open and stored after {} checks",
                market_id_string,
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
            assert_eq!(event.market_id, market_id);
            assert_eq!(event.community_id, key.community_id);
            assert_eq!(event.market_type, key.market_type);
            assert_eq!(event.delivery_start_time, key.delivery_start_time);
            assert_eq!(Uuid::from_bytes(market_id).get_version_num(), 4);
            assert_eq!(
                Uuid::from_bytes(market_id).get_variant(),
                uuid::Variant::RFC4122
            );
            let expected = MarketChainRecord {
                market_id: event.market_id,
                community_id: event.community_id,
                opening_time: event.opening_time,
                closing_time: event.closing_time,
                delivery_start_time: event.delivery_start_time,
                delivery_end_time: event.delivery_end_time,
                market_type: event.market_type,
                matching_algorithm: event.matching_algorithm,
                created_at: event.created_at,
            }
            .into_schema(community_id.to_string())
            .expect("Invalid NewMarketCreated event");
            assert_eq!(
                stored, expected,
                "Stored market differs from the NewMarketCreated event"
            );
            assert_eq!(stored.market_id, market_id_string);
            assert_eq!(stored.matching_algorithm, matching_algorithm_from_env());
            return (market_id, stored);
        }

        sleep(MARKET_POLL_INTERVAL).await;
    }

    panic!(
        "Timeout: Spot market for community {} (timeslot {}) was not open and indexed",
        community_id, world.target_delivery_time
    );
}

#[when("two communities are submitted to off-chain storage")]
async fn submit_two_communities(world: &mut MyWorld) {
    world.community_id = unique_community_id();
    // These opaque identities differ only in case and must remain distinct.
    world.secondary_community_id = world.community_id.to_ascii_lowercase();

    let communities = [
        EnergyCommunitySchema {
            community_id: world.community_id.clone(),
            community_name: format!("E2E Community {}", world.community_id),
            sites: vec![PRIMARY_SITE_ID.to_string()],
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
        EnergyCommunitySchema {
            community_id: world.secondary_community_id.clone(),
            community_name: format!("E2E Secondary Community {}", world.secondary_community_id),
            sites: vec!["site-12346".to_string()],
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
    ];

    for community in &communities {
        upsert_community(world, community).await;
    }
    let facilities = ["alice", "bob", "charlie"]
        .into_iter()
        .flat_map(|user| {
            [("", PRIMARY_SITE_ID), ("secondary-", "site-12346")]
                .into_iter()
                .map(move |(prefix, site)| FacilitySchema {
                    facility_id: format!("{prefix}area{user}"),
                    facility_name: format!("{prefix}area{user}"),
                    site_id: site.to_string(),
                    owner_id: format!("{prefix}{user}"),
                    valid_from: EPOCH_RFC3339.to_string(),
                    valid_until: None,
                })
        })
        .collect();
    world.create_facilities(facilities).await;

    let (current_timeslot, _) = get_last_and_next_timeslot();
    world.target_delivery_time = current_timeslot;
}

fn unique_community_id() -> String {
    format!("E2E-Community-{}", Uuid::new_v4())
}

#[then("the Market Orchestrator opens a distinct Spot market for each community")]
async fn wait_for_two_community_markets(world: &mut MyWorld) {
    let market_ids = [
        wait_for_market(world, &world.community_id).await.0,
        wait_for_market(world, &world.secondary_community_id)
            .await
            .0,
    ];

    assert_ne!(
        market_ids[0], market_ids[1],
        "Different communities resolved to the same Spot market id"
    );

    info!("Distinct Spot markets for both communities are open and stored");
    world.community_market_ids = Some(market_ids);
}

async fn upsert_default_community(world: &MyWorld) {
    let community = EnergyCommunitySchema {
        community_id: world.community_id.clone(),
        community_name: format!("E2E Community {}", world.community_id),
        sites: vec![PRIMARY_SITE_ID.to_string()],
        valid_from: EPOCH_RFC3339.to_string(),
        valid_until: None,
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
        .json(&EwdsCommunityDto::from(community.clone()))
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
    // valid_from is set by the off-chain storage.
    let community = EwdsCommunityDto {
        valid_from: None,
        ..EwdsCommunityDto::from(community.clone())
    };
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
        if communities.iter().any(|stored| {
            stored.community_id == community.community_id
                && stored.community_name == community.community_name
                && stored.sites == community.sites
        }) {
            return;
        }
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: community {} was not stored after publishing it through EWDS",
        community.community_id
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::abi::{encode, Token};
    use std::sync::Arc;

    fn lookup_fixture() -> (
        MarketControllerContract<Provider<MockProvider>>,
        MockProvider,
        MarketKey,
    ) {
        let (provider, mock) = Provider::mocked();
        let controller = MarketControllerContract::new(Address::zero(), Arc::new(provider));
        let key = MarketKey {
            community_id: [0x11; 16],
            market_type: MarketType::Spot.to_evm(),
            delivery_start_time: 1_700_000_100,
        };
        (controller, mock, key)
    }

    fn push_market_ids(mock: &MockProvider, ids: Vec<[u8; 16]>) {
        let encoded = encode(&[Token::Array(
            ids.into_iter()
                .map(|id| Token::FixedBytes(id.to_vec()))
                .collect(),
        )]);
        mock.push::<Bytes, _>(Bytes::from(encoded)).unwrap();
    }

    #[test]
    fn lookup_key_hashes_exact_opaque_community_spelling() {
        for (first, second) in [
            ("Community-A", "community-a"),
            (
                "ABCDEFAB-1234-4234-8234-ABCDEFABCDEF",
                "abcdefab-1234-4234-8234-abcdefabcdef",
            ),
        ] {
            let first_key = market_key(first, 1_700_000_100);
            let second_key = market_key(second, 1_700_000_100);
            assert_ne!(first_key.community_id, second_key.community_id);
            for (identity, key) in [(first, first_key), (second, second_key)] {
                assert_eq!(
                    key.community_id,
                    hex_to_bytes16(&derive_onchain_id(identity)).unwrap()
                );
                assert_eq!(key.market_type, MarketType::Spot.to_evm());
                assert_eq!(key.delivery_start_time, 1_700_000_100);
            }
        }
    }

    #[tokio::test]
    async fn lookup_binding_encodes_tuple_and_resolves_zero_then_existing_uuid() {
        let (controller, mock, key) = lookup_fixture();
        let call = controller.get_market_ids(vec![key.clone()]);
        let mut expected_calldata =
            ethers::utils::id("getMarketIds((bytes16,uint8,uint64)[])").to_vec();
        expected_calldata.extend(encode(&[Token::Array(vec![Token::Tuple(vec![
            Token::FixedBytes(key.community_id.to_vec()),
            Token::Uint(key.market_type.into()),
            Token::Uint(key.delivery_start_time.into()),
        ])])]));
        assert_eq!(call.calldata().unwrap().as_ref(), expected_calldata);

        let market_id = *Uuid::new_v4().as_bytes();
        for expected in [None, Some(market_id), Some(market_id)] {
            push_market_ids(&mock, vec![expected.unwrap_or([0; 16])]);
            assert_eq!(lookup_market_id(&controller, &key).await, expected);
            mock.assert_request(
                "eth_call",
                (call.tx.clone(), BlockId::Number(BlockNumber::Latest)),
            )
            .unwrap();
        }
    }

    #[tokio::test]
    #[should_panic(expected = "Expected one getMarketIds result")]
    async fn lookup_rejects_missing_result() {
        let (controller, mock, key) = lookup_fixture();
        push_market_ids(&mock, vec![]);
        lookup_market_id(&controller, &key).await;
    }

    #[tokio::test]
    #[should_panic(expected = "Expected one getMarketIds result")]
    async fn lookup_rejects_extra_results() {
        let (controller, mock, key) = lookup_fixture();
        push_market_ids(&mock, vec![[0; 16], [0; 16]]);
        lookup_market_id(&controller, &key).await;
    }
}

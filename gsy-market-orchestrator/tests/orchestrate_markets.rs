use chrono::DateTime;
use async_trait::async_trait;
use ethers::types::Address;
use gsy_market_orchestrator::chain_connector::{MarketChainClient, MarketKey, NewMarket};
use gsy_market_orchestrator::config::{Config, MarketRule, OffchainStorageTransport, MARKET_RULES};
use gsy_market_orchestrator::orchestrator::{
    orchestrate_markets, orchestrate_markets_at, MARKET_ID_LOOKUP_BATCH_SIZE,
};
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::offchain_storage::CommunityProvider;
use primitives::utils::{hex_to_bytes16, derive_onchain_id, uuid_v4_to_bytes16};
use primitives::{MarketType, MatchingAlgorithm};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use uuid::{Uuid, Variant, Version};

type MarketTuple = ([u8; 16], u8, u64);

fn market_tuple(market: &NewMarket) -> MarketTuple {
    (
        market.community_id,
        market.market_type,
        market.delivery_start_time,
    )
}

fn market_key(market: &NewMarket) -> MarketKey {
    MarketKey {
        community_id: market.community_id,
        market_type: market.market_type,
        delivery_start_time: market.delivery_start_time,
    }
}

#[derive(Default, Clone)]
struct MockChainClient {
    existing_markets: Arc<Mutex<HashMap<MarketTuple, [u8; 16]>>>,
    lookups: Arc<Mutex<Vec<Vec<MarketKey>>>>,
    batches: Arc<Mutex<Vec<Vec<NewMarket>>>>,
    /// Zero-based index of the create_markets call that fails.
    failing_call: Option<usize>,
    calls: Arc<Mutex<usize>>,
    /// Simulates a malformed response on the selected zero-based lookup call.
    malformed_lookup: Option<(usize, isize)>,
    lookup_error: bool,
    /// Coordinates two orchestrators after they have both observed missing keys.
    lookup_barrier: Option<Arc<tokio::sync::Barrier>>,
}

impl MockChainClient {
    fn with_existing(markets: impl IntoIterator<Item = NewMarket>) -> Self {
        Self {
            existing_markets: Arc::new(Mutex::new(
                markets
                    .into_iter()
                    .map(|market| (market_tuple(&market), market.market_id))
                    .collect(),
            )),
            ..Self::default()
        }
    }

    fn lookup_sizes(&self) -> Vec<usize> {
        self.lookups.lock().unwrap().iter().map(Vec::len).collect()
    }

    fn batches(&self) -> Vec<Vec<NewMarket>> {
        self.batches.lock().expect("batches lock poisoned").clone()
    }

    fn created(&self) -> Vec<NewMarket> {
        self.batches().into_iter().flatten().collect()
    }

    fn created_market(&self, key: MarketTuple) -> Option<NewMarket> {
        self.created()
            .into_iter()
            .find(|market| market_tuple(market) == key)
    }
}

#[async_trait]
impl MarketChainClient for MockChainClient {
    async fn is_operator_registered(&self) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn get_market_ids(&self, keys: Vec<MarketKey>) -> anyhow::Result<Vec<[u8; 16]>> {
        let call = {
            let mut lookups = self.lookups.lock().unwrap();
            lookups.push(keys.clone());
            lookups.len() - 1
        };
        if self.lookup_error {
            anyhow::bail!("lookup unavailable");
        }
        let mut ids = {
            let existing = self.existing_markets.lock().unwrap();
            keys.iter()
                .map(|key| {
                    existing
                        .get(&(key.community_id, key.market_type, key.delivery_start_time))
                        .copied()
                        .unwrap_or([0; 16])
                })
                .collect::<Vec<_>>()
        };
        if let Some((malformed_call, delta)) = self.malformed_lookup {
            if call == malformed_call {
                ids.resize((ids.len() as isize + delta) as usize, [0; 16]);
            }
        }
        if let Some(barrier) = &self.lookup_barrier {
            barrier.wait().await;
        }
        Ok(ids)
    }

    async fn create_markets(&self, new_markets: Vec<NewMarket>) -> anyhow::Result<()> {
        let call = {
            let mut calls = self.calls.lock().expect("calls lock poisoned");
            *calls += 1;
            *calls - 1
        };
        if self.failing_call == Some(call) {
            anyhow::bail!("transaction reverted");
        }
        // The contract keeps the first UUID for a tuple, even if a racing
        // orchestrator submits a different UUID after its lookup.
        let mut existing = self.existing_markets.lock().unwrap();
        for market in &new_markets {
            existing
                .entry(market_tuple(market))
                .or_insert(market.market_id);
        }
        self.batches
            .lock()
            .expect("batches lock poisoned")
            .push(new_markets);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct MockCommunityProvider {
    communities: Arc<Mutex<Vec<EnergyCommunitySchema>>>,
    error: Arc<Mutex<Option<String>>>,
    fetch_count: Arc<Mutex<u32>>,
}

impl MockCommunityProvider {
    fn with_error(message: &str) -> Self {
        Self {
            error: Arc::new(Mutex::new(Some(message.to_string()))),
            ..Self::default()
        }
    }

    fn set_communities(&self, communities: Vec<EnergyCommunitySchema>) {
        *self.communities.lock().expect("communities lock poisoned") = communities;
    }

    fn fetch_count(&self) -> u32 {
        *self.fetch_count.lock().expect("fetch_count lock poisoned")
    }
}

#[async_trait]
impl CommunityProvider for MockCommunityProvider {
    async fn fetch_communities(&self) -> anyhow::Result<Vec<EnergyCommunitySchema>> {
        *self.fetch_count.lock().expect("fetch_count lock poisoned") += 1;
        if let Some(message) = self.error.lock().expect("error lock poisoned").clone() {
            return Err(anyhow::anyhow!(message));
        }

        Ok(self
            .communities
            .lock()
            .expect("communities lock poisoned")
            .clone())
    }
}

fn community_with_id(community_id: &str) -> EnergyCommunitySchema {
    EnergyCommunitySchema {
        community_id: community_id.to_string(),
        community_name: "Community One".to_string(),
        sites: vec!["site-one".to_string()],
        valid_from: DateTime::UNIX_EPOCH,
        valid_until: None,
    }
}

fn community() -> EnergyCommunitySchema {
    community_with_id("11111111-1111-4111-8111-111111111111")
}

fn community_bytes() -> [u8; 16] {
    hashed_community_bytes(&community().community_id)
}

fn hashed_community_bytes(id: &str) -> [u8; 16] {
    hex_to_bytes16(&derive_onchain_id(id)).unwrap()
}

fn test_config(look_ahead_hours: u64) -> Config {
    Config {
        evm_node_url: "ws://localhost:8545".to_string(),
        market_controller_address: Address::zero(),
        orchestrator_signer_private_key:
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
        tick_interval_seconds: 1,
        look_ahead_hours,
        offchain_storage_transport: OffchainStorageTransport::Http,
        offchain_storage_url: "http://localhost:8080".to_string(),
        market_creation_batch_size: 1000,
    }
}

const NOW: u64 = 1_700_000_000;

fn current_slot() -> u64 {
    (NOW / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

/// Expected record apart from its randomly allocated market ID.
fn expected_market(community_id: [u8; 16], rule: &MarketRule, delivery_start: u64) -> NewMarket {
    NewMarket {
        market_id: [0; 16],
        community_id,
        opening_time: (delivery_start as i64 + rule.open_offset_mins * 60) as u64,
        closing_time: (delivery_start as i64 + rule.close_offset_mins * 60) as u64,
        delivery_start_time: delivery_start,
        delivery_end_time: delivery_start + GLOBAL_CONSTANTS.time_slot_sec,
        market_type: rule.market_type.to_evm(),
        matching_algorithm: rule.matching_algorithm.to_evm(),
    }
}

/// Every market the tick at NOW should create, in creation order: per
/// rule, the slots whose market has not closed at NOW and opens by
/// NOW + look-ahead, each for every community.
fn expected_markets(
    communities: &[EnergyCommunitySchema],
    look_ahead_hours: u64,
) -> Vec<NewMarket> {
    let horizon = NOW + look_ahead_hours * 3600;
    let mut markets = Vec::new();
    for rule in MARKET_RULES.iter() {
        let opening = |slot: u64| (slot as i64 + rule.open_offset_mins * 60) as u64;
        let closing = |slot: u64| (slot as i64 + rule.close_offset_mins * 60) as u64;
        let mut slots = Vec::new();
        let mut slot = current_slot() - 86_400;
        while slot <= horizon + 86_400 {
            if NOW < closing(slot) && opening(slot) <= horizon {
                slots.push(slot);
            }
            slot += GLOBAL_CONSTANTS.time_slot_sec;
        }
        for slot in slots {
            for community in communities {
                markets.push(expected_market(
                    hashed_community_bytes(&community.community_id),
                    rule,
                    slot,
                ));
            }
        }
    }
    markets
}

fn assert_market_records(actual: &[NewMarket], expected: &[NewMarket]) {
    let ids = actual
        .iter()
        .map(|market| market.market_id)
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), actual.len(), "market UUIDs must be unique");
    for id in ids {
        let uuid = Uuid::from_bytes(id);
        assert_eq!(uuid.get_version(), Some(Version::Random));
        assert_eq!(uuid.get_variant(), Variant::RFC4122);
    }
    let without_ids = actual
        .iter()
        .cloned()
        .map(|mut market| {
            market.market_id = [0; 16];
            market
        })
        .collect::<Vec<_>>();
    assert_eq!(without_ids, expected);
}

#[tokio::test]
async fn orchestration_tick_skips_when_no_communities_exist() {
    let config = test_config(0);
    let client = MockChainClient::default();
    let source = MockCommunityProvider::default();

    orchestrate_markets(&config, &client, &source)
        .await
        .expect("empty community result should not fail");

    assert_eq!(source.fetch_count(), 1);
    assert!(client.batches().is_empty());
}

#[tokio::test]
async fn orchestration_tick_propagates_community_fetch_failures() {
    let config = test_config(0);
    let client = MockChainClient::default();
    let source = MockCommunityProvider::with_error("community source unavailable");

    let error = orchestrate_markets(&config, &client, &source)
        .await
        .expect_err("community source error should propagate");

    assert!(error.to_string().contains("community source unavailable"));
    assert!(client.batches().is_empty());
}

#[tokio::test]
async fn orchestration_fetches_communities_on_every_tick() {
    let config = test_config(0);
    let client = MockChainClient::default();
    let source = MockCommunityProvider::default();

    orchestrate_markets(&config, &client, &source)
        .await
        .unwrap();
    source.set_communities(vec![community()]);
    orchestrate_markets(&config, &client, &source)
        .await
        .unwrap();

    assert_eq!(source.fetch_count(), 2);
}

#[tokio::test]
async fn creates_every_market_of_the_look_ahead_window() {
    let config = test_config(2);
    let client = MockChainClient::default();
    let communities = vec![
        community(),
        community_with_id("22222222-2222-4222-8222-222222222222"),
    ];

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    let expected = expected_markets(&communities, config.look_ahead_hours);
    assert!(expected.len() > communities.len() * MARKET_RULES.len());
    assert_eq!(client.batches().len(), 1);
    assert_market_records(&client.created(), &expected);
}

#[tokio::test]
async fn spot_markets_clear_pay_as_clear_and_the_others_pay_as_bid() {
    let config = test_config(2);
    let client = MockChainClient::default();

    orchestrate_markets_at(&config, &client, &[community()], NOW)
        .await
        .expect("orchestration should succeed");

    let mut algorithms: HashMap<u8, HashSet<u8>> = HashMap::new();
    for market in client.created() {
        algorithms
            .entry(market.market_type)
            .or_default()
            .insert(market.matching_algorithm);
    }
    assert_eq!(
        algorithms,
        HashMap::from([
            (
                MarketType::Spot.to_evm(),
                HashSet::from([MatchingAlgorithm::PayAsClear.to_evm()])
            ),
            (
                MarketType::Flex.to_evm(),
                HashSet::from([MatchingAlgorithm::PayAsBid.to_evm()])
            ),
            (
                MarketType::Settlement.to_evm(),
                HashSet::from([MatchingAlgorithm::PayAsBid.to_evm()])
            ),
        ])
    );
}

#[tokio::test]
async fn new_market_carries_the_full_market_record() {
    let config = test_config(3);
    let client = MockChainClient::default();
    let communities = vec![community()];
    // Spot opens 3 h before delivery by default, so this one opens in 1–2 h.
    let delivery_slot = current_slot() + 4 * 3600;
    let rule = &MARKET_RULES[0];
    let key = (community_bytes(), rule.market_type.to_evm(), delivery_slot);

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_market_records(
        &[client.created_market(key).expect("market created")],
        &[NewMarket {
            market_id: [0; 16],
            community_id: community_bytes(),
            opening_time: (delivery_slot as i64 + rule.open_offset_mins * 60) as u64,
            closing_time: (delivery_slot as i64 + rule.close_offset_mins * 60) as u64,
            delivery_start_time: delivery_slot,
            delivery_end_time: delivery_slot + GLOBAL_CONSTANTS.time_slot_sec,
            market_type: MarketType::Spot.to_evm(),
            matching_algorithm: rule.matching_algorithm.to_evm(),
        }],
    );
}

#[tokio::test]
async fn creates_missing_markets_that_have_already_opened() {
    let config = test_config(3);
    let client = MockChainClient::default();
    let communities = vec![community()];
    // Spot opens 3 h and closes 1 h before delivery: this one is open now.
    let open_slot = current_slot() + 2 * 3600;
    let rule = &MARKET_RULES[0];
    let open_market = expected_market(community_bytes(), rule, open_slot);
    assert!(open_market.opening_time <= NOW && NOW < open_market.closing_time);

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_market_records(
        &[client.created_market(market_tuple(&open_market)).unwrap()],
        &[open_market],
    );
}

#[tokio::test]
async fn never_creates_a_market_that_has_closed() {
    let config = test_config(3);
    let client = MockChainClient::default();
    let communities = vec![community()];

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    let created = client.created();
    assert!(!created.is_empty());
    assert!(created.iter().all(|market| NOW < market.closing_time));
    // Every market type closes at or before delivery start by default, so
    // the markets of the current delivery slot have closed.
    assert!(created
        .iter()
        .all(|market| market.delivery_start_time != current_slot()));
}

#[tokio::test]
async fn creates_markets_until_they_close() {
    let config = test_config(1);
    let communities = vec![community()];
    let rule = &MARKET_RULES[0];
    let slot = current_slot() + 4 * 3600;
    let market = expected_market(community_bytes(), rule, slot);
    let key = market_tuple(&market);
    let closing = market.closing_time;

    // One second before closing: still open, created.
    let client = MockChainClient::default();
    orchestrate_markets_at(&config, &client, communities.as_slice(), closing - 1)
        .await
        .expect("orchestration should succeed");
    assert!(client.created_market(key).is_some());

    // Closing exactly now: closed, not created.
    let client = MockChainClient::default();
    orchestrate_markets_at(&config, &client, communities.as_slice(), closing)
        .await
        .expect("orchestration should succeed");
    assert!(client.created_market(key).is_none());
}

#[tokio::test]
async fn creates_markets_opening_up_to_and_including_the_window_end() {
    let config = test_config(1);
    let communities = vec![community()];
    let rule = &MARKET_RULES[0];
    let slot = current_slot() + 4 * 3600;
    let market = expected_market(community_bytes(), rule, slot);
    let key = market_tuple(&market);
    let opening = market.opening_time;

    // Opening exactly at the end of the window: created.
    let client = MockChainClient::default();
    orchestrate_markets_at(&config, &client, communities.as_slice(), opening - 3600)
        .await
        .expect("orchestration should succeed");
    assert!(client.created_market(key).is_some());

    // Opening one second after the end of the window: not created yet.
    let client = MockChainClient::default();
    orchestrate_markets_at(&config, &client, communities.as_slice(), opening - 3600 - 1)
        .await
        .expect("orchestration should succeed");
    assert!(client.created_market(key).is_none());
}

#[tokio::test]
async fn skips_empty_and_padded_communities_without_blocking_valid_markets() {
    let config = test_config(1);
    let client = MockChainClient::default();
    let communities = vec![
        community_with_id(""),
        community_with_id(" "),
        community_with_id(" community"),
        community_with_id("community "),
        community_with_id("\tcommunity"),
        community_with_id("community\n"),
        community(),
    ];

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_market_records(
        &client.created(),
        &expected_markets(&[community()], config.look_ahead_hours),
    );
}

#[tokio::test]
async fn skips_markets_that_exist_on_chain() {
    let config = test_config(1);
    let communities = vec![community()];
    let all_markets = expected_markets(&communities, config.look_ahead_hours);
    let (existing, missing) = all_markets.split_at(all_markets.len() / 2);
    let client = MockChainClient::with_existing(existing.iter().cloned().map(|market| NewMarket {
        market_id: *Uuid::new_v4().as_bytes(),
        ..market
    }));
    let original_ids = client.existing_markets.lock().unwrap().clone();

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_market_records(&client.created(), missing);
    let stored = client.existing_markets.lock().unwrap();
    for (key, id) in original_ids {
        assert_eq!(stored.get(&key), Some(&id));
    }
}

#[tokio::test]
async fn looks_up_all_candidate_keys_in_one_call() {
    let config = test_config(1);
    let communities = vec![
        community(),
        community_with_id("22222222-2222-4222-8222-222222222222"),
    ];
    let client = MockChainClient::default();

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_eq!(
        client.lookup_sizes(),
        vec![expected_markets(&communities, config.look_ahead_hours).len()]
    );
    assert_eq!(
        *client.lookups.lock().unwrap(),
        vec![expected_markets(&communities, config.look_ahead_hours)
            .iter()
            .map(market_key)
            .collect::<Vec<_>>()]
    );
}

#[tokio::test]
async fn splits_key_lookups_into_chunks() {
    let config = test_config(1);
    let communities = (0..60)
        .map(|index| community_with_id(&format!("{:032x}", index + 1)))
        .collect::<Vec<_>>();
    let client = MockChainClient::default();
    let candidate_count = expected_markets(&communities, config.look_ahead_hours).len();
    assert!(candidate_count > MARKET_ID_LOOKUP_BATCH_SIZE);

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    let checks = client.lookup_sizes();
    assert_eq!(
        checks.len(),
        candidate_count.div_ceil(MARKET_ID_LOOKUP_BATCH_SIZE)
    );
    assert!(checks
        .iter()
        .all(|count| *count <= MARKET_ID_LOOKUP_BATCH_SIZE));
    assert_eq!(checks.iter().sum::<usize>(), candidate_count);
    assert_eq!(client.created().len(), candidate_count);
}

#[tokio::test]
async fn does_not_resend_markets_created_in_an_earlier_tick() {
    let config = test_config(1);
    let communities = vec![community()];
    let client = MockChainClient::default();

    for _ in 0..2 {
        orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
            .await
            .expect("tick should succeed");
    }

    assert_eq!(client.batches().len(), 1);
    assert_market_records(
        &client.created(),
        &expected_markets(&communities, config.look_ahead_hours),
    );
    assert_eq!(
        client.lookup_sizes().len(),
        2,
        "each tick consults the chain"
    );
}

#[tokio::test]
async fn sends_only_markets_new_to_the_window() {
    let config = test_config(1);
    let communities = vec![community()];
    let client = MockChainClient::default();
    let later = NOW + GLOBAL_CONSTANTS.time_slot_sec;

    for now in [NOW, later] {
        orchestrate_markets_at(&config, &client, communities.as_slice(), now)
            .await
            .expect("tick should succeed");
    }

    let first = client.batches()[0].clone();
    let second = client.batches()[1].clone();
    assert!(!second.is_empty());
    assert!(second.iter().all(|market| !first
        .iter()
        .any(|sent| market_tuple(sent) == market_tuple(market))));
    assert_market_records(
        &first,
        &expected_markets(&communities, config.look_ahead_hours),
    );
    // New: only markets that entered the opening window.
    assert!(second
        .iter()
        .all(|market| { NOW + 3600 < market.opening_time && market.opening_time <= later + 3600 }));
}

#[tokio::test]
async fn retries_markets_whose_transaction_failed() {
    let config = Config {
        market_creation_batch_size: 4,
        ..test_config(1)
    };
    let communities = vec![community()];
    let client = MockChainClient {
        failing_call: Some(1),
        ..MockChainClient::default()
    };
    let expected = expected_markets(&communities, config.look_ahead_hours);

    let error = orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect_err("second batch should fail");
    assert!(error.to_string().contains("transaction reverted"));
    assert_market_records(&client.created(), &expected[..4]);
    let first_batch = client.created();

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("retry should succeed");

    assert_market_records(&client.created(), &expected);
    assert_eq!(&client.created()[..4], first_batch.as_slice());
}

#[tokio::test]
async fn splits_creation_into_batches() {
    let config = Config {
        market_creation_batch_size: 4,
        ..test_config(1)
    };
    let client = MockChainClient::default();
    let communities = vec![community()];
    let expected = expected_markets(&communities, config.look_ahead_hours);

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    let batches = client.batches();
    assert_eq!(batches.len(), expected.len().div_ceil(4));
    assert!(batches
        .iter()
        .all(|batch| !batch.is_empty() && batch.len() <= 4));
    assert_market_records(&client.created(), &expected);
}

#[tokio::test]
async fn hashes_exact_opaque_community_ids_without_uuid_or_case_normalization() {
    let config = test_config(0);
    let ids = [
        "Community-A",
        "community-a",
        "community with internal spaces",
        "abcdefab-1234-4123-8123-abcdefabcdef",
        "ABCDEFAB-1234-4123-8123-ABCDEFABCDEF",
        "abcdefab123441238123abcdefabcdef",
        "0xabcdefab123441238123abcdefabcdef",
        "not-a-uuid",
        "00000000-0000-0000-0000-000000000000",
        "0x00000000000000000000000000000000",
        "00000000000000000000000000000000",
    ];
    let communities = ids
        .iter()
        .map(|id| community_with_id(id))
        .collect::<Vec<_>>();
    let original = communities
        .iter()
        .map(|c| c.community_id.clone())
        .collect::<Vec<_>>();
    let client = MockChainClient::default();

    orchestrate_markets_at(&config, &client, &communities, NOW)
        .await
        .unwrap();

    assert_market_records(&client.created(), &expected_markets(&communities, 0));
    let hashed = ids
        .iter()
        .map(|id| hashed_community_bytes(id))
        .collect::<HashSet<_>>();
    assert_eq!(
        hashed.len(),
        ids.len(),
        "actor spellings must remain distinct"
    );
    assert_eq!(
        client
            .created()
            .iter()
            .map(|market| market.community_id)
            .collect::<HashSet<_>>(),
        hashed
    );
    for id in ids {
        if let Ok(raw_bytes) = uuid_v4_to_bytes16(id) {
            assert_ne!(
                hashed_community_bytes(id),
                raw_bytes,
                "actor IDs must be hashed, not decoded"
            );
        }
    }
    assert_eq!(
        communities
            .iter()
            .map(|c| c.community_id.clone())
            .collect::<Vec<_>>(),
        original
    );
}

#[tokio::test]
async fn all_invalid_communities_skip_lookup_and_creation() {
    let communities = ["", " ", "\t", " padded", "padded\n"].map(community_with_id);
    let client = MockChainClient::default();

    orchestrate_markets_at(&test_config(0), &client, &communities, NOW)
        .await
        .unwrap();

    assert!(client.lookup_sizes().is_empty());
    assert!(client.batches().is_empty());
}

#[tokio::test]
async fn fresh_orchestrator_reuses_onchain_tuple_ids_after_restart() {
    let config = test_config(1);
    let communities = [community()];
    let first = MockChainClient::default();
    orchestrate_markets_at(&config, &first, &communities, NOW)
        .await
        .unwrap();
    let original = first.existing_markets.lock().unwrap().clone();
    assert!(!original.is_empty());

    // Only chain state survives the restart, not any orchestrator state.
    let restarted = MockChainClient::with_existing(first.created());
    orchestrate_markets_at(&config, &restarted, &communities, NOW)
        .await
        .unwrap();

    assert_eq!(restarted.lookup_sizes(), first.lookup_sizes());
    assert!(restarted.batches().is_empty());
    assert_eq!(*restarted.existing_markets.lock().unwrap(), original);
}

#[tokio::test]
async fn uuid_ids_are_random_across_independent_chains_for_the_same_keys() {
    let config = test_config(0);
    let communities = [community()];
    let first = MockChainClient::default();
    let second = MockChainClient::default();
    orchestrate_markets_at(&config, &first, &communities, NOW)
        .await
        .unwrap();
    orchestrate_markets_at(&config, &second, &communities, NOW)
        .await
        .unwrap();

    let expected = expected_markets(&communities, 0);
    assert_market_records(&first.created(), &expected);
    assert_market_records(&second.created(), &expected);
    let first_ids = first
        .created()
        .iter()
        .map(|market| market.market_id)
        .collect::<HashSet<_>>();
    assert!(second
        .created()
        .iter()
        .all(|market| !first_ids.contains(&market.market_id)));
}

#[tokio::test]
async fn concurrent_orchestrators_leave_one_uuid_per_tuple_without_reconciliation() {
    let config = test_config(0);
    let communities = [community()];
    let first = MockChainClient {
        lookup_barrier: Some(Arc::new(tokio::sync::Barrier::new(2))),
        ..MockChainClient::default()
    };
    let second = MockChainClient {
        existing_markets: first.existing_markets.clone(),
        lookup_barrier: first.lookup_barrier.clone(),
        ..MockChainClient::default()
    };

    let (a, b) = tokio::join!(
        orchestrate_markets_at(&config, &first, &communities, NOW),
        orchestrate_markets_at(&config, &second, &communities, NOW),
    );
    a.unwrap();
    b.unwrap();

    let expected = expected_markets(&communities, 0);
    assert_market_records(&first.created(), &expected);
    assert_market_records(&second.created(), &expected);
    let stored = first.existing_markets.lock().unwrap().clone();
    assert_eq!(stored.len(), expected.len());
    for (a, b) in first.created().iter().zip(second.created()) {
        assert_ne!(a.market_id, b.market_id);
        assert!([a.market_id, b.market_id].contains(&stored[&market_tuple(a)]));
    }
    // Neither tick needs another lookup or a local UUID-to-winner mapping.
    assert_eq!(first.lookup_sizes().len(), 1);
    assert_eq!(second.lookup_sizes().len(), 1);
    let next = MockChainClient {
        existing_markets: first.existing_markets.clone(),
        ..MockChainClient::default()
    };
    orchestrate_markets_at(&config, &next, &communities, NOW)
        .await
        .unwrap();
    assert!(next.batches().is_empty());
    assert_eq!(*next.existing_markets.lock().unwrap(), stored);
}

#[tokio::test]
async fn lookup_cardinality_mismatch_aborts_before_any_creation() {
    let communities = (0..60)
        .map(|index| community_with_id(&format!("Community-{index}")))
        .collect::<Vec<_>>();
    assert!(expected_markets(&communities, 1).len() > MARKET_ID_LOOKUP_BATCH_SIZE);
    // A malformed later chunk must not cause even earlier valid chunks to be submitted.
    for call in [0, 1] {
        for delta in [-1, 1] {
            let client = MockChainClient {
                malformed_lookup: Some((call, delta)),
                ..MockChainClient::default()
            };
            let error = orchestrate_markets_at(&test_config(1), &client, &communities, NOW)
                .await
                .expect_err("incorrect lookup cardinality must fail");
            assert!(error.to_string().contains("getMarketIds returned"));
            assert!(client.batches().is_empty());
        }
    }
}

#[tokio::test]
async fn lookup_failure_does_not_create_unchecked_markets() {
    let client = MockChainClient {
        lookup_error: true,
        ..MockChainClient::default()
    };
    let error = orchestrate_markets_at(&test_config(0), &client, &[community()], NOW)
        .await
        .expect_err("lookup failure must propagate");
    assert!(error.to_string().contains("lookup unavailable"));
    assert!(client.batches().is_empty());
}

use async_trait::async_trait;
use ethers::types::Address;
use gsy_market_orchestrator::chain_connector::{MarketChainClient, NewMarket};
use gsy_market_orchestrator::config::{Config, MarketRule, OffchainStorageTransport, MARKET_RULES};
use gsy_market_orchestrator::orchestrator::{
    orchestrate_markets, orchestrate_markets_at, MARKET_EXISTENCE_CHECK_BATCH_SIZE,
};
use primitives::constants::GLOBAL_CONSTANTS;
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::offchain_storage::CommunityProvider;
use primitives::utils::{generate_market_id, parse_uuid_or_hex_bytes16};
use primitives::MarketType;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
struct MockChainClient {
    existing_markets: Arc<Mutex<HashSet<[u8; 16]>>>,
    /// Number of market IDs per markets_exist call.
    existence_checks: Arc<Mutex<Vec<usize>>>,
    batches: Arc<Mutex<Vec<Vec<NewMarket>>>>,
    /// Zero-based index of the create_markets call that fails.
    failing_call: Option<usize>,
    calls: Arc<Mutex<usize>>,
}

impl MockChainClient {
    fn with_existing(market_ids: impl IntoIterator<Item = [u8; 16]>) -> Self {
        Self {
            existing_markets: Arc::new(Mutex::new(market_ids.into_iter().collect())),
            ..Self::default()
        }
    }

    fn existence_checks(&self) -> Vec<usize> {
        self.existence_checks
            .lock()
            .expect("existence_checks lock poisoned")
            .clone()
    }

    fn batches(&self) -> Vec<Vec<NewMarket>> {
        self.batches.lock().expect("batches lock poisoned").clone()
    }

    fn created(&self) -> Vec<NewMarket> {
        self.batches().into_iter().flatten().collect()
    }

    fn created_market(&self, market_id: [u8; 16]) -> Option<NewMarket> {
        self.created()
            .into_iter()
            .find(|market| market.market_id == market_id)
    }
}

#[async_trait]
impl MarketChainClient for MockChainClient {
    async fn is_operator_registered(&self) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn markets_exist(&self, market_ids: Vec<[u8; 16]>) -> anyhow::Result<Vec<bool>> {
        self.existence_checks
            .lock()
            .expect("existence_checks lock poisoned")
            .push(market_ids.len());
        let existing = self
            .existing_markets
            .lock()
            .expect("existing_markets lock poisoned");
        Ok(market_ids
            .iter()
            .map(|market_id| existing.contains(market_id))
            .collect())
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
        self.existing_markets
            .lock()
            .expect("existing_markets lock poisoned")
            .extend(new_markets.iter().map(|market| market.market_id));
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
    }
}

fn community() -> EnergyCommunitySchema {
    community_with_id("11111111-1111-4111-8111-111111111111")
}

fn community_bytes() -> [u8; 16] {
    parse_uuid_or_hex_bytes16(community().community_id.as_str()).unwrap()
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

/// The record the orchestrator should create for one market.
fn expected_market(
    market_id: [u8; 16],
    community_id: [u8; 16],
    rule: &MarketRule,
    delivery_start: u64,
) -> NewMarket {
    NewMarket {
        market_id,
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
/// rule, the current delivery slot (unless it also opens in the window)
/// followed by the slots opening in (NOW, NOW + look-ahead], each for
/// every community.
fn expected_markets(
    communities: &[EnergyCommunitySchema],
    look_ahead_hours: u64,
) -> Vec<NewMarket> {
    let horizon = NOW + look_ahead_hours * 3600;
    let mut markets = Vec::new();
    for rule in MARKET_RULES.iter() {
        let opening = |slot: u64| (slot as i64 + rule.open_offset_mins * 60) as u64;
        let mut slots = Vec::new();
        let mut slot = current_slot() - 86_400;
        while slot <= horizon + 86_400 {
            if NOW < opening(slot) && opening(slot) <= horizon {
                slots.push(slot);
            }
            slot += GLOBAL_CONSTANTS.time_slot_sec;
        }
        if !slots.contains(&current_slot()) {
            slots.insert(0, current_slot());
        }
        for slot in slots {
            for community in communities {
                markets.push(expected_market(
                    generate_market_id(
                        community.community_id.as_str(),
                        rule.market_type.clone(),
                        slot,
                    ),
                    parse_uuid_or_hex_bytes16(community.community_id.as_str()).unwrap(),
                    rule,
                    slot,
                ));
            }
        }
    }
    markets
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
    assert_eq!(client.batches(), vec![expected]);
}

#[tokio::test]
async fn new_market_carries_the_full_market_record() {
    let config = test_config(3);
    let client = MockChainClient::default();
    let communities = vec![community()];
    // Spot opens 3 h before delivery by default, so this one opens in 1–2 h.
    let delivery_slot = current_slot() + 4 * 3600;
    let rule = &MARKET_RULES[0];
    let market_id = generate_market_id(
        communities[0].community_id.as_str(),
        rule.market_type.clone(),
        delivery_slot,
    );

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_eq!(
        client.created_market(market_id).expect("market created"),
        NewMarket {
            market_id,
            community_id: community_bytes(),
            opening_time: (delivery_slot as i64 + rule.open_offset_mins * 60) as u64,
            closing_time: (delivery_slot as i64 + rule.close_offset_mins * 60) as u64,
            delivery_start_time: delivery_slot,
            delivery_end_time: delivery_slot + GLOBAL_CONSTANTS.time_slot_sec,
            market_type: MarketType::Spot.to_evm(),
            matching_algorithm: rule.matching_algorithm.to_evm(),
        }
    );
}

#[tokio::test]
async fn never_creates_a_market_that_has_opened() {
    let config = test_config(3);
    let client = MockChainClient::default();
    let communities = vec![community()];
    // Spot opens 3 h and closes 1 h before delivery: this one is open now.
    let open_slot = current_slot() + 2 * 3600;
    let rule = &MARKET_RULES[0];
    let open_market = expected_market(
        generate_market_id(
            communities[0].community_id.as_str(),
            MarketType::Spot,
            open_slot,
        ),
        community_bytes(),
        rule,
        open_slot,
    );
    assert!(open_market.opening_time <= NOW && NOW < open_market.closing_time);

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert!(client.created_market(open_market.market_id).is_none());
    assert!(client.created().iter().all(|market| {
        NOW < market.opening_time || market.delivery_start_time == current_slot()
    }));
}

#[tokio::test]
async fn creates_the_markets_of_the_current_delivery_slot() {
    let config = test_config(1);
    let communities = vec![community()];
    let client = MockChainClient::default();
    let current_market_ids = MARKET_RULES
        .iter()
        .map(|rule| {
            generate_market_id(
                communities[0].community_id.as_str(),
                rule.market_type.clone(),
                current_slot(),
            )
        })
        .collect::<Vec<_>>();

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("tick should succeed");

    for market_id in &current_market_ids {
        let market = client.created_market(*market_id).expect("market created");
        assert!(market.delivery_start_time <= NOW && NOW < market.delivery_end_time);
    }

    // Still in the same delivery slot: they exist on-chain and are not resent.
    let delivery_end = current_slot() + GLOBAL_CONSTANTS.time_slot_sec;
    orchestrate_markets_at(&config, &client, communities.as_slice(), delivery_end - 1)
        .await
        .expect("tick should succeed");
    let resent = client.batches().get(1).cloned().unwrap_or_default();
    assert!(resent
        .iter()
        .all(|market| !current_market_ids.contains(&market.market_id)));
}

#[tokio::test]
async fn creates_markets_opening_up_to_and_including_the_window_end() {
    let config = test_config(1);
    let communities = vec![community()];
    let rule = &MARKET_RULES[0];
    let slot = current_slot() + 4 * 3600;
    let market_id = generate_market_id(
        communities[0].community_id.as_str(),
        rule.market_type.clone(),
        slot,
    );
    let opening = expected_market(market_id, community_bytes(), rule, slot).opening_time;

    // Opening exactly at the end of the window: created.
    let client = MockChainClient::default();
    orchestrate_markets_at(&config, &client, communities.as_slice(), opening - 3600)
        .await
        .expect("orchestration should succeed");
    assert!(client.created_market(market_id).is_some());

    // Opening exactly now: already open, not created.
    let client = MockChainClient::default();
    orchestrate_markets_at(&config, &client, communities.as_slice(), opening)
        .await
        .expect("orchestration should succeed");
    assert!(client.created_market(market_id).is_none());
}

#[tokio::test]
async fn skips_communities_with_unparsable_ids() {
    let config = test_config(1);
    let client = MockChainClient::default();
    let communities = vec![community_with_id("not-a-uuid"), community()];

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_eq!(
        client.created(),
        expected_markets(&[community()], config.look_ahead_hours)
    );
}

#[tokio::test]
async fn skips_markets_that_exist_on_chain() {
    let config = test_config(1);
    let communities = vec![community()];
    let all_markets = expected_markets(&communities, config.look_ahead_hours);
    let (existing, missing) = all_markets.split_at(all_markets.len() / 2);
    let client = MockChainClient::with_existing(existing.iter().map(|market| market.market_id));

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    assert_eq!(client.created(), missing.to_vec());
}

#[tokio::test]
async fn checks_existence_of_all_candidates_in_one_call() {
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
        client.existence_checks(),
        vec![expected_markets(&communities, config.look_ahead_hours).len()]
    );
}

#[tokio::test]
async fn splits_existence_checks_into_chunks() {
    let config = test_config(1);
    let communities = (0..60)
        .map(|index| community_with_id(&format!("{:032x}", index + 1)))
        .collect::<Vec<_>>();
    let client = MockChainClient::default();
    let candidate_count = expected_markets(&communities, config.look_ahead_hours).len();
    assert!(candidate_count > MARKET_EXISTENCE_CHECK_BATCH_SIZE);

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("orchestration should succeed");

    let checks = client.existence_checks();
    assert_eq!(
        checks.len(),
        candidate_count.div_ceil(MARKET_EXISTENCE_CHECK_BATCH_SIZE)
    );
    assert!(checks
        .iter()
        .all(|count| *count <= MARKET_EXISTENCE_CHECK_BATCH_SIZE));
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

    assert_eq!(
        client.batches(),
        vec![expected_markets(&communities, config.look_ahead_hours)]
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

    let first = expected_markets(&communities, config.look_ahead_hours);
    let second = client.batches()[1].clone();
    assert!(!second.is_empty());
    assert!(second
        .iter()
        .all(|market| !first.iter().any(|sent| sent.market_id == market.market_id)));
    // New: markets that entered the opening window, and those of the
    // delivery slot that became current.
    let later_slot = (later / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;
    assert!(second.iter().all(|market| {
        (NOW + 3600 < market.opening_time && market.opening_time <= later + 3600)
            || market.delivery_start_time == later_slot
    }));
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
    assert_eq!(client.created(), expected[..4].to_vec());

    orchestrate_markets_at(&config, &client, communities.as_slice(), NOW)
        .await
        .expect("retry should succeed");

    assert_eq!(client.created(), expected);
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
    assert_eq!(client.created(), expected);
}

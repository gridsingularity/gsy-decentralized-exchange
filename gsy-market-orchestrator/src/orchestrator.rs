use crate::chain_connector::{MarketChainClient, NewMarket};
use crate::config::{Config, MarketRule, MARKET_RULES};
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::offchain_storage::CommunityProvider;
use primitives::{
    constants::GLOBAL_CONSTANTS,
    utils::{generate_market_id, parse_uuid_or_hex_bytes16, timestamp_to_datetime_string},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub async fn run<C, S>(config: Config, client: C, community_source: S) -> anyhow::Result<()>
where
    C: MarketChainClient,
    S: CommunityProvider,
{
    info!("Configuration: {:?}", config);

    info!("Waiting for orchestrator account to be registered as an operator...");
    loop {
        match client.is_operator_registered().await {
            Ok(true) => {
                info!("✅ Orchestrator account is registered. Starting main loop.");
                break;
            }
            Ok(false) => {
                warn!("Orchestrator account not yet registered. Retrying in 10 seconds...");
            }
            Err(e) => {
                error!(
                    "Error checking registration status: {:?}. Retrying in 10 seconds...",
                    e
                );
            }
        }
        sleep(Duration::from_secs(10)).await;
    }

    let interval = Duration::from_secs(config.tick_interval_seconds);

    loop {
        info!("-- Orchestrator Tick --");
        if let Err(e) = orchestrate_markets(&config, &client, &community_source).await {
            error!("An error occurred during orchestration tick: {:?}", e);
        }
        sleep(interval).await;
    }
}

async fn orchestrate_markets<C, S>(
    config: &Config,
    client: &C,
    community_source: &S,
) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
    S: CommunityProvider + ?Sized,
{
    let communities = community_source.fetch_communities().await?;
    if communities.is_empty() {
        warn!("No communities found; skipping market orchestration tick");
        return Ok(());
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    orchestrate_markets_at(config, client, communities.as_slice(), now).await
}

async fn orchestrate_markets_at<C>(
    config: &Config,
    client: &C,
    communities: &[EnergyCommunitySchema],
    now: u64,
) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
{
    let look_ahead_horizon = now + (config.look_ahead_hours * 3600);

    info!(
        "Orchestrator Check at {}. Creating markets opening until {} for {} communities",
        now,
        look_ahead_horizon,
        communities.len()
    );

    let communities = communities
        .iter()
        .filter_map(
            |community| match parse_uuid_or_hex_bytes16(community.community_id.as_str()) {
                Some(community_bytes) => Some((community, community_bytes)),
                None => {
                    warn!(
                        "Skipping community '{}': its ID is not a UUID or 16-byte hex value",
                        community.community_id
                    );
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    let mut new_markets = Vec::new();

    // Markets are created before they open: those opening in
    // (now, look_ahead_horizon]. The markets of the current delivery slot are
    // added too, so they exist after a (re)start even if they have opened.
    let current_delivery_slot =
        (now / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;
    for rule in MARKET_RULES.iter() {
        let mut delivery_slots = delivery_slots_opening_in(rule, now, look_ahead_horizon);
        if !delivery_slots.contains(&current_delivery_slot) {
            delivery_slots.insert(0, current_delivery_slot);
        }
        for delivery_start in delivery_slots {
            for (community, community_bytes) in &communities {
                let market_id = generate_market_id(
                    community.community_id.as_str(),
                    rule.market_type.clone(),
                    delivery_start,
                );
                // The chain is the single source of truth for which markets exist.
                if client.market_exists(market_id).await? {
                    continue;
                }

                let new_market = new_market(market_id, *community_bytes, rule, delivery_start);
                debug!(
                    "Market community '{}' type '{:?}' for delivery at {}. Opens {}, closes {}.",
                    community.community_id,
                    rule.market_type,
                    timestamp_to_datetime_string(delivery_start),
                    timestamp_to_datetime_string(new_market.opening_time),
                    timestamp_to_datetime_string(new_market.closing_time)
                );
                new_markets.push(new_market);
            }
        }
    }

    if !new_markets.is_empty() {
        info!(
            "Sending {} markets in batches of {}",
            new_markets.len(),
            config.market_creation_batch_size
        );
    }
    for batch in new_markets.chunks(config.market_creation_batch_size) {
        client.create_markets(batch.to_vec()).await?;
    }

    Ok(())
}

/// Delivery slots whose market for `rule` opens in `(after, until]`.
fn delivery_slots_opening_in(rule: &MarketRule, after: u64, until: u64) -> Vec<u64> {
    let time_slot = GLOBAL_CONSTANTS.time_slot_sec as i64;
    let open_offset = rule.open_offset_mins * 60;
    // opening = delivery + open_offset, so the delivery range is shifted by -open_offset.
    let first_slot = ((after as i64 - open_offset).div_euclid(time_slot) + 1) * time_slot;
    let last_delivery = until as i64 - open_offset;
    (first_slot..=last_delivery)
        .step_by(time_slot as usize)
        .map(|slot| slot as u64)
        .collect()
}

/// Market record for one community, rule and delivery slot.
fn new_market(
    market_id: [u8; 16],
    community_id: [u8; 16],
    rule: &MarketRule,
    delivery_start_secs: u64,
) -> NewMarket {
    NewMarket {
        market_id,
        community_id,
        opening_time: (delivery_start_secs as i64 + rule.open_offset_mins * 60) as u64,
        closing_time: (delivery_start_secs as i64 + rule.close_offset_mins * 60) as u64,
        delivery_start_time: delivery_start_secs,
        delivery_end_time: delivery_start_secs + GLOBAL_CONSTANTS.time_slot_sec,
        market_type: rule.market_type.to_evm(),
        matching_algorithm: rule.matching_algorithm.to_evm(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ethers::types::Address;
    use primitives::MarketType;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct MockChainClient {
        existing_markets: Arc<Mutex<HashSet<[u8; 16]>>>,
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

        async fn market_exists(&self, market_id: [u8; 16]) -> anyhow::Result<bool> {
            Ok(self
                .existing_markets
                .lock()
                .expect("existing_markets lock poisoned")
                .contains(&market_id))
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
            offchain_storage_transport: crate::config::OffchainStorageTransport::Http,
            offchain_storage_url: "http://localhost:8080".to_string(),
            market_creation_batch_size: 1000,
        }
    }

    const NOW: u64 = 1_700_000_000;

    fn current_slot() -> u64 {
        (NOW / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
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
                    markets.push(new_market(
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
        let open_market = new_market(
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

    #[test]
    fn delivery_slots_include_only_openings_inside_the_window() {
        let rule = &MARKET_RULES[0];
        let open_offset = rule.open_offset_mins * 60;
        let slot = current_slot() + 4 * 3600;
        let opening = (slot as i64 + open_offset) as u64;

        // Opening exactly at `after` is excluded, exactly at `until` included.
        assert!(!delivery_slots_opening_in(rule, opening, opening + 3600).contains(&slot));
        assert!(delivery_slots_opening_in(rule, opening - 1, opening).contains(&slot));
        assert_eq!(
            delivery_slots_opening_in(rule, opening - 1, opening + 3600),
            (0..=4)
                .map(|index| slot + index * GLOBAL_CONSTANTS.time_slot_sec)
                .collect::<Vec<_>>()
        );
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
}

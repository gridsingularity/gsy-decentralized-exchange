use crate::chain_connector::MarketChainClient;
use crate::community_source::CommunityProvider;
use crate::config::{Config, MARKET_RULES};
use blake2_rfc::blake2b::blake2b;
use primitives::MarketType;
use primitives::{constants::GLOBAL_CONSTANTS, utils::timestamp_to_datetime_string};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{error, info, warn};

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
    orchestrate_markets_at(config, client, now).await
}

async fn orchestrate_markets_at<C>(config: &Config, client: &C, now: u64) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
{
    let look_ahead_horizon = now + (config.look_ahead_hours * 3600);

    let mut current_delivery_secs =
        (now / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;

    info!(
        "Orchestrator Check at {}. Looking ahead to {}",
        now, look_ahead_horizon
    );

    while current_delivery_secs <= look_ahead_horizon {
        for rule in MARKET_RULES.iter() {
            let market_id = generate_market_id(rule.market_type.clone(), current_delivery_secs);
            let open_time = (current_delivery_secs as i64 + rule.open_offset_mins * 60) as u64;
            let close_time = (current_delivery_secs as i64 + rule.close_offset_mins * 60) as u64;

            let on_chain_status = client.get_market_status(market_id).await?;
            let should_be_open = market_should_be_open(
                now,
                current_delivery_secs,
                rule.open_offset_mins,
                rule.close_offset_mins,
            );

            if should_be_open && !on_chain_status {
                error!(
                    "OPENING market '{:?}' for delivery at {}. Opening time {}.",
                    rule.market_type,
                    timestamp_to_datetime_string(current_delivery_secs),
                    timestamp_to_datetime_string(open_time)
                );
                client.update_market_status(market_id, true).await?;
            } else if !should_be_open && on_chain_status {
                error!(
                    "CLOSING market '{:?}' for delivery at {}. Closing time {}.",
                    rule.market_type,
                    timestamp_to_datetime_string(current_delivery_secs),
                    timestamp_to_datetime_string(close_time)
                );
                client.update_market_status(market_id, false).await?;
            }
        }
        current_delivery_secs += GLOBAL_CONSTANTS.time_slot_sec;
    }
    Ok(())
}

pub fn generate_market_id(market_type: MarketType, delivery_timestamp: u64) -> [u8; 16] {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(market_type.as_str().as_bytes());
    buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
    blake2b(16, &[], &buffer)
        .as_bytes()
        .try_into()
        .expect("hash is 16 bytes")
}

fn market_should_be_open(
    now: u64,
    delivery_time: u64,
    open_offset_mins: i64,
    close_offset_mins: i64,
) -> bool {
    let open_time = (delivery_time as i64 + open_offset_mins * 60) as u64;
    let close_time = (delivery_time as i64 + close_offset_mins * 60) as u64;
    now >= open_time && now < close_time
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ethers::types::Address;
    use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct MockChainClient {
        market_statuses: Arc<Mutex<HashMap<[u8; 16], bool>>>,
        updates: Arc<Mutex<Vec<([u8; 16], bool)>>>,
    }

    impl MockChainClient {
        fn with_statuses(statuses: HashMap<[u8; 16], bool>) -> Self {
            Self {
                market_statuses: Arc::new(Mutex::new(statuses)),
                updates: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn updates(&self) -> Vec<([u8; 16], bool)> {
            self.updates.lock().expect("updates lock poisoned").clone()
        }
    }

    #[async_trait]
    impl MarketChainClient for MockChainClient {
        async fn is_operator_registered(&self) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn get_market_status(&self, market_id: [u8; 16]) -> anyhow::Result<bool> {
            Ok(*self
                .market_statuses
                .lock()
                .expect("market_statuses lock poisoned")
                .get(&market_id)
                .unwrap_or(&false))
        }

        async fn update_market_statuses(
            &self,
            market_ids: Vec<[u8; 16]>,
            is_open: bool,
        ) -> anyhow::Result<()> {
            for market_id in market_ids {
                self.market_statuses
                    .lock()
                    .expect("market_statuses lock poisoned")
                    .insert(market_id, is_open);
                self.updates
                    .lock()
                    .expect("updates lock poisoned")
                    .push((market_id, is_open));
            }
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

    fn community() -> EnergyCommunitySchema {
        EnergyCommunitySchema {
            community_id: "11111111-1111-4111-8111-111111111111".to_string(),
            community_name: "Community One".to_string(),
            sites: vec!["site-one".to_string()],
        }
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
        }
    }

    fn find_delivery_slot_for_state(
        now: u64,
        look_ahead_hours: u64,
        should_be_open: bool,
    ) -> (MarketType, u64) {
        let look_ahead_horizon = now + (look_ahead_hours * 3600);
        let mut current_delivery_secs =
            (now / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;

        while current_delivery_secs <= look_ahead_horizon {
            for rule in MARKET_RULES.iter() {
                if market_should_be_open(
                    now,
                    current_delivery_secs,
                    rule.open_offset_mins,
                    rule.close_offset_mins,
                ) == should_be_open
                {
                    return (rule.market_type.clone(), current_delivery_secs);
                }
            }
            current_delivery_secs += GLOBAL_CONSTANTS.time_slot_sec;
        }

        panic!(
            "No delivery slot found for should_be_open={} in look_ahead={}h",
            should_be_open, look_ahead_hours
        );
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
        assert!(client.updates().is_empty());
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
        assert!(client.updates().is_empty());
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
    async fn orchestrate_markets_opens_market_when_it_should_be_open() {
        let now = 1_700_000_000;
        let config = test_config(4);
        let client = MockChainClient::default();
        let (market_type, delivery_slot) =
            find_delivery_slot_for_state(now, config.look_ahead_hours, true);
        let expected_market_id = generate_market_id(market_type, delivery_slot);

        orchestrate_markets_at(&config, &client, now)
            .await
            .expect("orchestration should succeed");

        assert!(
            client.updates().contains(&(expected_market_id, true)),
            "expected market to be opened"
        );
    }

    #[tokio::test]
    async fn orchestrate_markets_closes_market_when_it_should_be_closed() {
        let now = 1_700_000_000;
        let config = test_config(0);
        let (market_type, delivery_slot) =
            find_delivery_slot_for_state(now, config.look_ahead_hours, false);
        let expected_market_id = generate_market_id(market_type, delivery_slot);

        let mut statuses = HashMap::new();
        statuses.insert(expected_market_id, true);
        let client = MockChainClient::with_statuses(statuses);

        orchestrate_markets_at(&config, &client, now)
            .await
            .expect("orchestration should succeed");

        assert!(
            client.updates().contains(&(expected_market_id, false)),
            "expected market to be closed"
        );
    }

    #[test]
    fn generate_market_id_is_deterministic() {
        let delivery = 1_700_000_000;

        let first = generate_market_id(MarketType::Spot, delivery);
        let second = generate_market_id(MarketType::Spot, delivery);
        let different = generate_market_id(MarketType::Flex, delivery);

        assert_eq!(first, second);
        assert_ne!(first, different);
    }
}

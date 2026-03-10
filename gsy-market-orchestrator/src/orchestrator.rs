use crate::chain_connector::MarketChainClient;
use crate::config::{Config, MARKET_RULES};
use blake2_rfc::blake2b::blake2b;
use gsy_offchain_primitives::{
    constants::GLOBAL_CONSTANTS, utils::timestamp_to_datetime_string, MarketType,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{error, info, warn};

pub async fn run<C>(config: Config, client: C) -> anyhow::Result<()>
where
    C: MarketChainClient,
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
        if let Err(e) = orchestrate_markets(&config, &client).await {
            error!("An error occurred during orchestration tick: {:?}", e);
        }
        sleep(interval).await;
    }
}

async fn orchestrate_markets<C>(config: &Config, client: &C) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
{
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
            let market_id = generate_market_id(rule.market_type, current_delivery_secs);
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

pub fn generate_market_id(market_type: MarketType, delivery_timestamp: u64) -> [u8; 32] {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(market_type.as_str().as_bytes());
    buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
    blake2b(32, &[], &buffer)
        .as_bytes()
        .try_into()
        .expect("hash is 32 bytes")
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
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct MockChainClient {
        market_statuses: Arc<Mutex<HashMap<[u8; 32], bool>>>,
        updates: Arc<Mutex<Vec<([u8; 32], bool)>>>,
    }

    impl MockChainClient {
        fn with_statuses(statuses: HashMap<[u8; 32], bool>) -> Self {
            Self {
                market_statuses: Arc::new(Mutex::new(statuses)),
                updates: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn updates(&self) -> Vec<([u8; 32], bool)> {
            self.updates.lock().expect("updates lock poisoned").clone()
        }
    }

    #[async_trait]
    impl MarketChainClient for MockChainClient {
        async fn is_operator_registered(&self) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn get_market_status(&self, market_id: [u8; 32]) -> anyhow::Result<bool> {
            Ok(*self
                .market_statuses
                .lock()
                .expect("market_statuses lock poisoned")
                .get(&market_id)
                .unwrap_or(&false))
        }

        async fn update_market_status(
            &self,
            market_id: [u8; 32],
            is_open: bool,
        ) -> anyhow::Result<()> {
            self.market_statuses
                .lock()
                .expect("market_statuses lock poisoned")
                .insert(market_id, is_open);
            self.updates
                .lock()
                .expect("updates lock poisoned")
                .push((market_id, is_open));
            Ok(())
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
                    return (rule.market_type, current_delivery_secs);
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
        let different = generate_market_id(MarketType::Flexibility, delivery);

        assert_eq!(first, second);
        assert_ne!(first, different);
    }
}

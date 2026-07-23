use crate::chain_connector::MarketChainClient;
use crate::config::{Config, MARKET_RULES, MarketRule};
use crate::ewds_connector::{fetch_list_of_community_ids_via_ewds, fetch_list_of_markets_via_ewds};
use blake2_rfc::blake2b::blake2b;
use primitives::{
    constants::GLOBAL_CONSTANTS, utils::timestamp_to_datetime_string,
    utils::{string_to_timestamp, convert_uuid_string_to_bytes},
    db_api_schema::market::MarketSchema,
            MatchingAlgorithm
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

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

    let tick_interval = Duration::from_secs(config.tick_interval_seconds);
    let community_ids = fetch_list_of_community_ids_via_ewds().await?; // todo

    loop {
        info!("-- Orchestrator Tick --");
        if let Err(e) = orchestrate_markets(&config, &client, community_ids.clone()).await {
            error!("An error occurred during orchestration tick: {:?}", e);
        }
        sleep(tick_interval).await;
    }
}

async fn orchestrate_markets<C>(config: &Config, client: &C, community_ids: Vec<[u8; 16]>
) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
{
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    orchestrate_markets_at(config, client, now, community_ids).await;
    Ok(())
}

async fn orchestrate_markets_at<C>(config: &Config, client: &C, now: u64, community_ids: Vec<[u8; 16]>
) -> anyhow::Result<()>
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

    for community_id in community_ids.iter() {
        for rule in MARKET_RULES.iter() {
            let markets = fetch_list_of_markets_via_ewds(
                rule.market_type.clone() as u8, current_delivery_secs, community_id.clone()).await?;
            let mut existing_market_slots = Vec::new();
            for market in markets.iter() {
                open_close_market(client, now, current_delivery_secs, market, rule).await?;
                let market_slot = string_to_timestamp(&market.delivery_start_time)?;
                existing_market_slots.push(market_slot);
            }

            create_markets(
                client,
                now,
                current_delivery_secs,
                community_id,
                rule,
                existing_market_slots).await?;
            current_delivery_secs += GLOBAL_CONSTANTS.time_slot_sec;
        }
    }
    Ok(())
}

pub async fn create_markets<C>(
    client: &C,
    now: u64,
    current_delivery_secs: u64,
    community_id: &[u8; 16],
    rule: &MarketRule,
    existing_market_slots: Vec<u64>,
) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
{
    let start_current_market_slots = current_delivery_secs as u64;
    let end_current_market_slots = current_delivery_secs + (rule.open_offset_mins.abs() * 60) as u64;
    let market_slot_start_times: Vec<u64> = (
        start_current_market_slots..=end_current_market_slots).step_by(
        GLOBAL_CONSTANTS.time_slot_sec as usize).collect();
    for market_slot_start_time in market_slot_start_times {
        if !existing_market_slots.contains(&market_slot_start_time) {
            let open_time = (
                current_delivery_secs as i64 + rule.open_offset_mins * 60) as u64;
            let close_time = (
                current_delivery_secs as i64 + rule.close_offset_mins * 60) as u64;
            let delivery_start_time = market_slot_start_time as u64;
            let market_id = generate_market_id();
            client.create_market(
                market_id,
                *community_id,
                open_time,
                close_time,
                delivery_start_time,
                (market_slot_start_time + GLOBAL_CONSTANTS.time_slot_sec) as u64,
                now,
                MatchingAlgorithm::PayAsBid as u8,
                rule.market_type.clone() as u8,
                true
            );
        }
    }
    Ok(())
}

pub async fn open_close_market<C>(
    client: &C, now: u64, current_delivery_secs: u64, market: &MarketSchema,
    rule: &MarketRule
) -> anyhow::Result<()>
where
    C: MarketChainClient + ?Sized,
{
    let opening_time = string_to_timestamp(&market.opening_time)?;
    let closing_time = string_to_timestamp(&market.closing_time)?;
    let should_be_open = market_should_be_open(
        now,
        opening_time,
        closing_time,
    );
    let market_id = convert_uuid_string_to_bytes(&market.market_id)?;
    if should_be_open && !market.is_open {
        info!(
            "OPENING market '{:?}' for delivery at {}. Opening time {}.",
            rule.market_type,
            timestamp_to_datetime_string(current_delivery_secs),
            timestamp_to_datetime_string(opening_time)
            );
    } else if !should_be_open && market.is_open {
        info!(
            "CLOSING market '{:?}' for delivery at {}. Closing time {}.",
            rule.market_type,
            timestamp_to_datetime_string(current_delivery_secs),
            timestamp_to_datetime_string(closing_time)
            );
        client.update_market_status(market_id, false).await?;
    }
    Ok(())
}

pub fn generate_market_id() -> [u8; 16] {
    let mut buffer = Vec::new();
    let id = Uuid::new_v4();
    buffer.extend_from_slice(id.as_bytes());
    blake2b(16, &[], &buffer)
        .as_bytes()
        .try_into()
}

fn market_should_be_open(
    now: u64,
    opening_time: u64,
    closing_time: u64,
) -> bool {
    now >= opening_time && now < closing_time
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ethers::types::Address;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use crate::MarketType;

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

        async fn update_market_status(
            &self,
            market_id: [u8; 16],
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

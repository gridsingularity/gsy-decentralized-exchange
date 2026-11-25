use crate::chain_connector::{MarketChainClient, MarketKey, NewMarket};
use crate::config::{Config, MarketRule, MARKET_RULES};
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::offchain_storage::CommunityProvider;
use primitives::{
    constants::GLOBAL_CONSTANTS,
    utils::{
        bytes16_to_uuid_string, derive_onchain_id, validate_offchain_actor_id, hex_to_bytes16,
        timestamp_to_datetime_string,
    },
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Market keys per `getMarketIds` call, bounded to stay below `eth_call` gas caps.
pub const MARKET_ID_LOOKUP_BATCH_SIZE: usize = 500;

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

/// One orchestration tick at the current time: fetches the communities and
/// creates their missing markets.
pub async fn orchestrate_markets<C, S>(
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

/// One orchestration tick at `now` (epoch seconds) for the given communities:
/// creates every missing market that has not closed at `now` and opens by
/// `now + LOOK_AHEAD_HOURS`.
pub async fn orchestrate_markets_at<C>(
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
            |community| match validate_offchain_actor_id(&community.community_id) {
                Ok(id) => Some(
                    hex_to_bytes16(&derive_onchain_id(&id))
                        .expect("actor ID hashing produces bytes16"),
                ),
                Err(error) => {
                    warn!("Skipping community '{}': {}", community.community_id, error);
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    let mut candidates = Vec::new();

    // Markets are normally created before they open. Markets that have opened
    // but not closed are included too, so a new community or a (re)start
    // after downtime still gets every market it can trade in.
    for rule in MARKET_RULES.iter() {
        for delivery_start in delivery_slots_to_create(rule, now, look_ahead_horizon) {
            for community_id in &communities {
                candidates.push((
                    MarketKey {
                        community_id: *community_id,
                        market_type: rule.market_type.to_evm(),
                        delivery_start_time: delivery_start,
                    },
                    rule,
                ));
            }
        }
    }

    // Tuple lookup is the source of truth across ticks, restarts and competing
    // orchestrators. Allocate a new UUID only for tuples reported as missing.
    let mut new_markets = Vec::new();
    for chunk in candidates.chunks(MARKET_ID_LOOKUP_BATCH_SIZE) {
        let keys = chunk.iter().map(|(key, _)| key.clone()).collect();
        let market_ids = client.get_market_ids(keys).await?;
        anyhow::ensure!(
            market_ids.len() == chunk.len(),
            "getMarketIds returned {} results for {} keys",
            market_ids.len(),
            chunk.len()
        );
        for ((key, rule), market_id) in chunk.iter().zip(market_ids) {
            if market_id != [0; 16] {
                continue;
            }
            let market = new_market(
                *Uuid::new_v4().as_bytes(),
                key.community_id,
                rule,
                key.delivery_start_time,
            );
            debug!(
                "Market {} for delivery at {} is missing. Opens {}, closes {}.",
                bytes16_to_uuid_string(market.market_id),
                timestamp_to_datetime_string(market.delivery_start_time),
                timestamp_to_datetime_string(market.opening_time),
                timestamp_to_datetime_string(market.closing_time)
            );
            new_markets.push(market);
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

/// Delivery slots whose market for `rule` has not closed at `now` and opens by
/// `until`.
fn delivery_slots_to_create(rule: &MarketRule, now: u64, until: u64) -> Vec<u64> {
    let time_slot = GLOBAL_CONSTANTS.time_slot_sec as i64;
    // closing = delivery + close_offset and opening = delivery + open_offset,
    // so both bounds are shifted by the offsets to get delivery times.
    let first_slot =
        ((now as i64 - rule.close_offset_mins * 60).div_euclid(time_slot) + 1) * time_slot;
    let last_delivery = until as i64 - rule.open_offset_mins * 60;
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

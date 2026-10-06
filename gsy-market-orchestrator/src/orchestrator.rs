use crate::chain_connector::{MarketChainClient, NewMarket};
use crate::config::{Config, MarketRule, MARKET_RULES};
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::offchain_storage::CommunityProvider;
use primitives::{
    constants::GLOBAL_CONSTANTS,
    utils::{
        bytes16_to_hex, generate_market_id, parse_uuid_or_hex_bytes16, timestamp_to_datetime_string,
    },
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Market IDs per `marketsExist` call. Each lookup costs about one cold
/// SLOAD (2.1k gas), so 500 IDs stay far below common `eth_call` gas caps.
pub const MARKET_EXISTENCE_CHECK_BATCH_SIZE: usize = 500;

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
/// creates every missing market that opens in `(now, now + LOOK_AHEAD_HOURS]`
/// and every missing market of the current delivery slot.
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

    let mut candidates = Vec::new();

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
                candidates.push(new_market(
                    market_id,
                    *community_bytes,
                    rule,
                    delivery_start,
                ));
            }
        }
    }

    // The chain is the single source of truth for which markets exist; ask it
    // for all candidates at once instead of one RPC call per market.
    let mut new_markets = Vec::new();
    for chunk in candidates.chunks(MARKET_EXISTENCE_CHECK_BATCH_SIZE) {
        let market_ids = chunk.iter().map(|market| market.market_id).collect();
        let exists = client.markets_exist(market_ids).await?;
        for (market, exists) in chunk.iter().zip(exists) {
            if exists {
                continue;
            }
            debug!(
                "Market {} for delivery at {} is missing. Opens {}, closes {}.",
                bytes16_to_hex(market.market_id),
                timestamp_to_datetime_string(market.delivery_start_time),
                timestamp_to_datetime_string(market.opening_time),
                timestamp_to_datetime_string(market.closing_time)
            );
            new_markets.push(market.clone());
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

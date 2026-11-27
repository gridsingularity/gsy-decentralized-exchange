use crate::algorithms::{MatchOrders, PayAsClearPricing};
use crate::models::{Attributes, BidOfferMatch, MatchType, MatchingData, Order, Requirements};
use anyhow::{anyhow, Context, Error, Result};
use chrono::{DateTime, Utc};
use ethers::prelude::*;
use ethers::utils::keccak256;
use primitives::db_api_schema::orders::{
    order_metadata_to_contract, DbOrderSchema, OrderEnum, OrderStatus,
};
use primitives::db_api_schema::trades::ClearingStatus;
use primitives::ewds::dto::{order_status_from_ewds, EwdsOrderDto};
use primitives::ewds::{EwdsClient, EwdsOperation};
use primitives::matching::{current_unix_time, matching_block_interval, ClearingSchedule};
use primitives::offchain_storage::OffchainStorageClient;
use primitives::utils::{
    bytes16_to_uuid_string, datetime_to_epoch, derive_onchain_id, hex_to_bytes16, normalize_uuid,
    rfc3339_to_datetime, uuid_v4_to_bytes16, validate_offchain_actor_id,
    NODE_FLOAT_SCALING_FACTOR,
};
use primitives::MatchingAlgorithm;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::str::FromStr;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

abigen!(
    SubmitMatchesContract,
    "src/connectors/abi/submit_matches.json"
);

// The contract's ClearingResult struct collides with the local ClearingResult model.
use submit_matches_contract::ClearingResult as EvmClearingResult;

struct PreparedOrders {
    open_bids: Vec<Order>,
    open_offers: Vec<Order>,
    by_order_id: HashMap<String, DbOrderSchema>,
}

#[derive(Default)]
pub struct ClearingResult {
    pub market_id: Option<String>,
    pub clearing_status: ClearingStatus,
    pub clearing_price: Option<u64>,
    pub total_supply: Option<u64>,
    pub total_demand: Option<u64>,
    pub traded_quantity: Option<u64>,
    pub num_trades: Option<u32>,
}

pub struct MarketMatches {
    pub bid_offer_matches: Vec<BidOfferMatch>,
    pub clearing_result: ClearingResult,
}

pub async fn evm_subscribe(
    orderbook_url: String,
    node_url: String,
    trade_matching_address: String,
    matching_engine_private_key: String,
    matching_algorithm: MatchingAlgorithm,
    pay_as_clear_pricing: PayAsClearPricing,
    clearing_schedule: ClearingSchedule,
) -> Result<(), Error> {
    info!("Connecting to EVM node {}", node_url);
    let provider = Provider::<Ws>::connect(node_url.as_str()).await?;
    if matches!(matching_algorithm, MatchingAlgorithm::PayAsClear) {
        // Pay-as-clear markets are cleared at times derived from their own
        // opening and closing time, not on block boundaries.
        drop(provider);
        return run_clearing_schedule(
            orderbook_url.as_str(),
            node_url.as_str(),
            trade_matching_address.as_str(),
            matching_engine_private_key.as_str(),
            pay_as_clear_pricing,
            clearing_schedule,
        )
        .await;
    }
    let matching_block_interval = matching_block_interval();
    info!(
        "Using {}-block matching interval for {}",
        matching_block_interval, matching_algorithm
    );
    let mut last_processed_block = provider.get_block_number().await?;
    // Keep track of which trigger "bucket" was already processed, so we do not
    // miss matches when multiple blocks are mined between polling iterations.
    let mut last_processed_trigger_bucket =
        last_processed_block.as_u64().saturating_sub(1) / matching_block_interval;

    loop {
        let block_number = provider.get_block_number().await?;
        if block_number > last_processed_block {
            info!("Block {} observed", block_number);

            let current_trigger_bucket = block_number.as_u64() / matching_block_interval;
            if current_trigger_bucket > last_processed_trigger_bucket {
                info!(
                    "Matching trigger reached (bucket {} -> {}) at block {}",
                    last_processed_trigger_bucket, current_trigger_bucket, block_number
                );

                if let Err(error) = run_matching_cycle(
                    orderbook_url.as_str(),
                    node_url.as_str(),
                    trade_matching_address.as_str(),
                    matching_engine_private_key.as_str(),
                    &matching_algorithm,
                    pay_as_clear_pricing,
                )
                .await
                {
                    error!("Matching cycle failed: {:?}", error);
                }

                last_processed_trigger_bucket = current_trigger_bucket;
            }

            last_processed_block = block_number;
        }

        sleep(Duration::from_secs(2)).await;
    }
}

async fn run_matching_cycle(
    orderbook_url: &str,
    evm_node_url: &str,
    trade_matching_address: &str,
    matching_engine_private_key: &str,
    matching_algorithm: &MatchingAlgorithm,
    pay_as_clear_pricing: PayAsClearPricing,
) -> Result<()> {
    info!("Starting matching cycle");
    info!("Fetching open orders from {}", orderbook_url);

    let prepared_orders =
        fetch_open_orders_from_orderbook_service(orderbook_url.to_string()).await?;
    match_and_submit(
        prepared_orders,
        evm_node_url,
        trade_matching_address,
        matching_engine_private_key,
        matching_algorithm,
        pay_as_clear_pricing,
    )
    .await
}

async fn match_and_submit(
    prepared_orders: PreparedOrders,
    evm_node_url: &str,
    trade_matching_address: &str,
    matching_engine_private_key: &str,
    matching_algorithm: &MatchingAlgorithm,
    pay_as_clear_pricing: PayAsClearPricing,
) -> Result<()> {
    info!(
        "Prepared open orders: bids={}, offers={}",
        prepared_orders.open_bids.len(),
        prepared_orders.open_offers.len()
    );
    if prepared_orders.open_bids.is_empty() || prepared_orders.open_offers.is_empty() {
        info!("No open bid/offer pairs to match");
        return Ok(());
    }

    let PreparedOrders {
        open_bids,
        open_offers,
        by_order_id,
    } = prepared_orders;
    let order_books = partition_orders_by_market_slot(open_bids, open_offers)?;
    info!(
        "Partitioned open orders into {} market/time-slot order books",
        order_books.len()
    );

    let matches_per_market =
        match_order_books(order_books, matching_algorithm, pay_as_clear_pricing)?;
    if matches_per_market.is_empty() {
        info!("No matches generated by {} algorithm", matching_algorithm);
        return Ok(());
    }

    info!(
        "Generated matches for {} markets using {}",
        matches_per_market.len(),
        matching_algorithm
    );
    send_submit_matches_transaction(
        evm_node_url,
        trade_matching_address,
        matching_engine_private_key,
        matches_per_market,
        by_order_id,
    )
    .await?;
    Ok(())
}

/// How often the open orders are read to find markets that are not known yet.
const MARKET_DISCOVERY_INTERVAL_SECONDS: u64 = 5;

/// The open window of a market with open orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketWindow {
    pub opening_time: u64,
    pub closing_time: u64,
    /// Clearing times up to this time were already handled.
    pub checked_until: u64,
}

/// The markets that have a clearing time in `(checked_until, now]` and are
/// still open at `now`.
pub fn markets_due_for_clearing(
    markets: &HashMap<String, MarketWindow>,
    clearing_schedule: &ClearingSchedule,
    now: u64,
) -> HashSet<String> {
    markets
        .iter()
        .filter(|(_, market)| {
            now < market.closing_time
                && clearing_schedule.is_clearing_due(
                    market.opening_time,
                    market.closing_time,
                    market.checked_until,
                    now,
                )
        })
        .map(|(market_id, _)| market_id.clone())
        .collect()
}

/// Clears every pay-as-clear market at the times `clearing_schedule` derives
/// from the opening and closing time of the market.
///
/// The clock is checked every second. Markets are found through their open
/// orders, so a market without orders is not cleared. A clearing whose orders
/// cannot be read is retried on the next second; a clearing whose
/// `submitMatches` transaction fails is not repeated.
async fn run_clearing_schedule(
    orderbook_url: &str,
    evm_node_url: &str,
    trade_matching_address: &str,
    matching_engine_private_key: &str,
    pay_as_clear_pricing: PayAsClearPricing,
    clearing_schedule: ClearingSchedule,
) -> Result<()> {
    let offchain_storage_client =
        OffchainStorageClient::from_env("EWDS_MATCHING_ENGINE_CLIENT_ID", "gsymatchingengine");
    let mut markets: HashMap<String, MarketWindow> = HashMap::new();
    // Clearing times before the start are not made up for.
    let mut last_discovery = current_unix_time();
    let mut next_discovery = last_discovery;

    loop {
        let now = current_unix_time();
        let mut prepared_orders = None;

        if now >= next_discovery {
            next_discovery = now + MARKET_DISCOVERY_INTERVAL_SECONDS;
            match fetch_open_orders_from_orderbook_service(orderbook_url.to_string()).await {
                Ok(orders) => {
                    // The first orders of a new market arrived after the
                    // previous discovery, so its clearing times since then
                    // are still due.
                    discover_markets(
                        &offchain_storage_client,
                        &orders,
                        &mut markets,
                        last_discovery,
                    )
                    .await;
                    last_discovery = now;
                    prepared_orders = Some(orders);
                }
                Err(error) => error!("Failed to fetch open orders: {:?}", error),
            }
        }

        markets.retain(|_, market| now < market.closing_time);
        let due_markets = markets_due_for_clearing(&markets, &clearing_schedule, now);
        for (market_id, market) in markets.iter_mut() {
            if !due_markets.contains(market_id) {
                market.checked_until = now;
            }
        }

        if !due_markets.is_empty() {
            let prepared_orders = match prepared_orders {
                Some(orders) => Ok(orders),
                None => fetch_open_orders_from_orderbook_service(orderbook_url.to_string()).await,
            };
            match prepared_orders {
                Ok(mut orders) => {
                    for market_id in &due_markets {
                        if let Some(market) = markets.get_mut(market_id) {
                            market.checked_until = now;
                        }
                    }
                    info!("Clearing {} market(s): {:?}", due_markets.len(), due_markets);
                    orders
                        .open_bids
                        .retain(|order| due_markets.contains(&order.market_id));
                    orders
                        .open_offers
                        .retain(|order| due_markets.contains(&order.market_id));
                    if let Err(error) = match_and_submit(
                        orders,
                        evm_node_url,
                        trade_matching_address,
                        matching_engine_private_key,
                        &MatchingAlgorithm::PayAsClear,
                        pay_as_clear_pricing,
                    )
                    .await
                    {
                        error!("Clearing failed: {:?}", error);
                    }
                }
                // The markets stay due and are retried on the next second.
                Err(error) => error!("Failed to fetch the orders to clear: {:?}", error),
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

/// Adds the markets of `orders` that are not in `markets` yet.
async fn discover_markets(
    offchain_storage_client: &OffchainStorageClient,
    orders: &PreparedOrders,
    markets: &mut HashMap<String, MarketWindow>,
    checked_until: u64,
) {
    let unknown_market_ids: HashSet<&String> = orders
        .open_bids
        .iter()
        .chain(orders.open_offers.iter())
        .map(|order| &order.market_id)
        .filter(|market_id| !markets.contains_key(*market_id))
        .collect();

    for market_id in unknown_market_ids {
        let market = match offchain_storage_client.fetch_market(market_id).await {
            Ok(Some(market)) => market,
            Ok(None) => {
                warn!("Market {} of an open order is not in the storage", market_id);
                continue;
            }
            Err(error) => {
                error!("Failed to fetch market {}: {:?}", market_id, error);
                continue;
            }
        };
        match (
            datetime_to_epoch(&market.opening_time),
            datetime_to_epoch(&market.closing_time),
        ) {
            (Ok(opening_time), Ok(closing_time)) => {
                info!(
                    "Market {} is open from {} to {}",
                    market_id, opening_time, closing_time
                );
                markets.insert(
                    market_id.clone(),
                    MarketWindow {
                        opening_time,
                        closing_time,
                        checked_until,
                    },
                );
            }
            _ => error!(
                "Market {} has an invalid opening or closing time: {} / {}",
                market_id, market.opening_time, market.closing_time
            ),
        }
    }
}

pub fn match_order_books(
    order_books: Vec<MatchingData>,
    matching_algorithm: &MatchingAlgorithm,
    pay_as_clear_pricing: PayAsClearPricing,
) -> Result<Vec<MarketMatches>> {
    let mut all_matches = Vec::new();
    for mut market_matching_data in order_books {
        if market_matching_data.bids().is_empty() || market_matching_data.offers().is_empty() {
            info!(
                "Skipping market {} timeslot {} without an open bid/offer pair",
                market_matching_data.market_id(),
                market_matching_data.time_slot()
            );
            continue;
        }
        let market_matches: Vec<BidOfferMatch> = matching_algorithm
            .match_orders_with_pricing(&mut market_matching_data, pay_as_clear_pricing)
            .map_err(|error| anyhow!(error))?;

        let clearing_result = compute_clearing_result(&market_matching_data, &market_matches);

        all_matches.push(MarketMatches {
            bid_offer_matches: market_matches,
            clearing_result,
        });
    }
    Ok(all_matches)
}

fn compute_clearing_result(
    market_matching_data: &MatchingData,
    matches: &[BidOfferMatch],
) -> ClearingResult {
    let total_supply: u64 = market_matching_data
        .offers()
        .iter()
        .map(|offer| offer.energy)
        .sum();
    let total_demand: u64 = market_matching_data
        .bids()
        .iter()
        .map(|bid| bid.energy)
        .sum();
    let traded_quantity: u64 = matches.iter().map(|m| m.selected_energy).sum();

    // * FINAL: the whole bid energy or the whole offer energy was matched
    // * PARTIAL: some bids and offers were matched, others were still not matched
    // * REJECTED: no bid and no offer were matched, obviously due to unmatched prices
    // * NO_BID: there is neither a bid nor an offer in the order book todo in DD-434
    let clearing_status = if matches.is_empty() {
        ClearingStatus::Rejected
    } else if traded_quantity == total_demand || traded_quantity == total_supply {
        ClearingStatus::Final
    } else {
        ClearingStatus::Partial
    };

    let mut clearing_result = ClearingResult {
        market_id: Some(market_matching_data.market_id().to_string()),
        clearing_status,
        total_supply: Some(total_supply),
        total_demand: Some(total_demand),
        ..Default::default()
    };

    if !matches.is_empty() {
        let num_trades = matches.len() as u32;
        let clearing_price = matches.iter().map(|m| m.energy_rate).sum::<u64>() / num_trades as u64;

        clearing_result.clearing_price = Some(clearing_price);
        clearing_result.traded_quantity = Some(traded_quantity);
        clearing_result.num_trades = Some(num_trades);
    }

    clearing_result
}

pub fn partition_orders_by_market_slot(
    bids: Vec<Order>,
    offers: Vec<Order>,
) -> Result<Vec<MatchingData>> {
    let mut order_books: BTreeMap<(String, u64), (Vec<Order>, Vec<Order>)> = BTreeMap::new();

    for bid in bids {
        order_books
            .entry((bid.market_id.clone(), bid.time_slot))
            .or_default()
            .0
            .push(bid);
    }
    for offer in offers {
        order_books
            .entry((offer.market_id.clone(), offer.time_slot))
            .or_default()
            .1
            .push(offer);
    }

    order_books
        .into_iter()
        .map(|((market_id, time_slot), (bids, offers))| {
            MatchingData::new(market_id, time_slot, bids, offers).map_err(|error| anyhow!(error))
        })
        .collect()
}

pub async fn send_submit_matches_transaction(
    evm_node_url: &str,
    trade_matching_address: &str,
    matching_engine_private_key: &str,
    matches: Vec<MarketMatches>,
    order_lookup: HashMap<String, DbOrderSchema>,
) -> Result<()> {
    if matches.is_empty() {
        info!("No matches to submit");
        return Ok(());
    }

    let trade_matching_address =
        Address::from_str(trade_matching_address).map_err(|e| {
            anyhow!(
                "Invalid trade matching address '{}': {}",
                trade_matching_address,
                e
            )
        })?;
    let evm_matches = to_evm_matches(matches, &order_lookup)?;

    let provider = Provider::<Ws>::connect(evm_node_url).await?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = matching_engine_private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow!("Invalid matching engine private key: {}", e))?
        .with_chain_id(chain_id);
    let signer_address = wallet.address();
    let client = std::sync::Arc::new(SignerMiddleware::new(provider, wallet));
    let trade_matching =
        SubmitMatchesContract::new(trade_matching_address, client.clone());

    let operator_role = keccak256("OPERATOR_ROLE");
    let has_role = trade_matching
        .has_role(operator_role, signer_address)
        .call()
        .await?;
    if !has_role {
        warn!(
            "Signer {:?} does not currently have OPERATOR_ROLE in TradeMatching",
            signer_address
        );
    }

    info!("Submitting {} matches to submitMatches", evm_matches.len());
    let submit_matches_call = trade_matching.submit_matches(evm_matches);
    let pending_tx = submit_matches_call.send().await?;
    let tx_hash = pending_tx.tx_hash();
    let receipt = pending_tx.await?;

    match receipt {
        Some(receipt) => {
            if receipt
                .status
                .map(|status| status.as_u64())
                .unwrap_or_default()
                != 1
            {
                return Err(anyhow!(
                    "submitMatches transaction {:?} reverted with status {:?}",
                    tx_hash,
                    receipt.status
                ));
            }
            info!("submitMatches successful. tx={:?}", tx_hash);
            Ok(())
        }
        None => Err(anyhow!(
            "submitMatches transaction {:?} dropped without receipt",
            tx_hash
        )),
    }
}

async fn fetch_market_orders(body: Vec<EwdsOrderDto>) -> Result<PreparedOrders> {
    prepare_market_orders(body).await
}

async fn prepare_market_orders(body: Vec<EwdsOrderDto>) -> Result<PreparedOrders> {
    let mut open_bids: Vec<Order> = Vec::new();
    let mut open_offers: Vec<Order> = Vec::new();
    let mut books: BTreeMap<(String, DateTime<Utc>), Vec<DbOrderSchema>> = BTreeMap::new();
    let mut excluded = HashSet::new();
    for dto in body {
        if matches!(order_status_from_ewds(&dto.order_status), Ok(status) if status != OrderStatus::Submitted)
        {
            continue;
        }
        // An unknown market cannot safely be isolated from the other books.
        let market_id = uuid_v4_to_bytes16(&dto.market_id)
            .map(bytes16_to_uuid_string)
            .map_err(|_| {
                anyhow!(
                    "Cannot identify market for order {}: {}",
                    dto.order_id,
                    dto.market_id
                )
            })?;
        let time_slot = rfc3339_to_datetime(&dto.time_slot).ok();
        let order_id = dto.order_id.clone();
        match DbOrderSchema::try_from(dto) {
            Ok(mut order) => {
                order.market_id = market_id.clone();
                books
                    .entry((market_id, order.time_slot))
                    .or_default()
                    .push(order);
            }
            Err(error) => {
                error!(%order_id, %market_id, ?time_slot, %error, "Excluding invalid order's book; unreadable slot excludes the entire market");
                excluded.insert((market_id, time_slot));
            }
        }
    }
    let mut by_order_id: HashMap<String, DbOrderSchema> = HashMap::new();

    for ((market_id, time_slot), orders) in books {
        if excluded.contains(&(market_id.clone(), None))
            || excluded.contains(&(market_id.clone(), Some(time_slot)))
        {
            continue;
        }
        // Publish a book only after every active order has converted successfully.
        let mut converted = Vec::with_capacity(orders.len());
        for db_order in orders {
            match convert_db_order_to_canonical(&db_order) {
                Ok(order) => converted.push((order, db_order)),
                Err(error) => {
                    error!(order_id = %db_order.order_id, %market_id, %time_slot, %error, "Excluding book with invalid canonical order");
                    converted.clear();
                    break;
                }
            }
        }
        for (order, db_order) in converted {
            by_order_id.insert(order.order_id.clone(), db_order);
            match order.order_type {
                OrderEnum::Bid => open_bids.push(order),
                OrderEnum::Offer => open_offers.push(order),
            }
        }
    }

    Ok(PreparedOrders {
        open_bids,
        open_offers,
        by_order_id,
    })
}

async fn fetch_open_orders_from_orderbook_service(url: String) -> Result<PreparedOrders, Error> {
    if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching orders via EWDS transport");
        return fetch_open_orders_via_ewds(url).await;
    }

    let res = reqwest::get(url).await?.error_for_status()?;
    debug!("Response: {:?} {}", res.version(), res.status());
    debug!("Headers: {:#?}\n", res.headers());

    let body = res.json::<Vec<EwdsOrderDto>>().await?;
    info!("Fetched {} total orders from orderbook", body.len());
    fetch_market_orders(body).await
}

async fn fetch_open_orders_via_ewds(fallback_url: String) -> Result<PreparedOrders, Error> {
    fetch_open_orders_via_ewds_query(fallback_url).await
}

async fn fetch_open_orders_via_ewds_query(fallback_url: String) -> Result<PreparedOrders, Error> {
    let query_payload = parse_query_params_from_url(&fallback_url);
    info!("Publishing EWDS orders.query request");
    let ewds_client = EwdsClient::from_env(
        "EWDS_MATCHING_ENGINE_CLIENT_ID",
        "gsymatchingengine",
        60_000,
    );
    let orders: Vec<EwdsOrderDto> = ewds_client
        .query(EwdsOperation::OrdersQuery, query_payload)
        .await?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()?;
    info!("Fetched {} total orders from EWDS", orders.len());
    fetch_market_orders(orders).await
}

fn parse_query_params_from_url(url: &str) -> serde_json::Value {
    if let Ok(parsed_url) = reqwest::Url::parse(url) {
        let mut map = serde_json::Map::new();
        for (key, value) in parsed_url.query_pairs() {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        return serde_json::Value::Object(map);
    }
    serde_json::json!({})
}

fn parse_bytes16_field(field_name: &str, value: &str) -> Result<[u8; 16]> {
    uuid_v4_to_bytes16(value).with_context(|| format!("{} must be a UUIDv4", field_name))
}

fn normalize_entity_id(field_name: &str, value: &str) -> Result<String> {
    normalize_uuid(value).with_context(|| format!("Invalid {}", field_name))
}

fn convert_requirements(
    requirements: Option<&primitives::db_api_schema::orders::DbRequirements>,
) -> Result<Option<Requirements>> {
    requirements
        .map(|requirements| -> Result<Requirements> {
            Ok(Requirements {
                trading_partner_id: requirements
                    .trading_partner_id
                    .as_deref()
                    .map(validate_offchain_actor_id)
                    .transpose()?,
                energy_type: requirements.energy_type.clone(),
                preferred_energy_rate: requirements
                    .preferred_energy_rate
                    .map(|rate| (rate * NODE_FLOAT_SCALING_FACTOR).round() as u64),
            })
        })
        .transpose()
}

fn convert_attributes(
    attributes: Option<&primitives::db_api_schema::orders::DbAttributes>,
) -> Result<Option<Attributes>> {
    attributes
        .map(|attributes| -> Result<Attributes> {
            Ok(Attributes {
                energy_type: attributes.energy_type.clone(),
            })
        })
        .transpose()
}

fn convert_db_order_to_canonical(order: &DbOrderSchema) -> Result<Order> {
    let order_id = normalize_entity_id("order_id", &order.order_id)?;
    let market_id = normalize_entity_id("market_id", &order.market_id)?;
    let created_by = validate_offchain_actor_id(&order.created_by)?;
    let time_slot = datetime_to_epoch(&order.time_slot)?;
    let creation_time = datetime_to_epoch(&order.creation_time)?;
    Ok(match order.order_type {
        OrderEnum::Bid => Order {
            created_by,
            order_id,
            order_type: OrderEnum::Bid,
            status: order.status.clone(),
            market_id,
            time_slot,
            creation_time,
            energy: (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            energy_rate: (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            requirements: convert_requirements(order.requirements.as_ref())?,
            attributes: None,
        },
        OrderEnum::Offer => Order {
            order_id,
            order_type: order.order_type.clone(),
            status: order.status.clone(),
            created_by,
            market_id,
            time_slot,
            creation_time,
            energy: (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            energy_rate: (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            requirements: convert_requirements(order.requirements.as_ref())?,
            attributes: convert_attributes(order.attributes.as_ref())?,
        },
    })
}

fn to_evm_order_data(order: &DbOrderSchema, expected_type: OrderEnum) -> Result<OrderParams> {
    if order.order_type != expected_type {
        return Err(anyhow!(
            "Order {} type mismatch. Expected {:?}, got {:?}",
            order.order_id,
            expected_type,
            order.order_type
        ));
    }

    // Actor IDs remain off-chain throughout matching; derive pseudonyms only here.
    let mut requirements = order.requirements.clone();
    if let Some(partner) = requirements
        .as_mut()
        .and_then(|r| r.trading_partner_id.as_mut())
    {
        *partner = derive_onchain_id(&validate_offchain_actor_id(partner)?);
    }
    let metadata = order_metadata_to_contract(requirements.as_ref(), order.attributes.as_ref())?;

    Ok(OrderParams {
        order_id: parse_bytes16_field("order_id", order.order_id.as_str())?,
        created_by: hex_to_bytes16(&derive_onchain_id(&validate_offchain_actor_id(&order.created_by)?)).expect("actor hash is bytes16"),
        market_id: parse_bytes16_field("market_id", order.market_id.as_str())?,
        time_slot: datetime_to_epoch(&order.time_slot)?,
        creation_time: datetime_to_epoch(&order.creation_time)?,
        energy: (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        energy_rate: (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        energy_source_preference: metadata.energy_source_preference,
        energy_type: metadata.energy_type,
        is_bid: order.order_type == OrderEnum::Bid,
        preferred_trading_partner: metadata.preferred_trading_partner,
        preferred_energy_rate: metadata.preferred_energy_rate,
    })
}

fn optional_order_id_to_bytes16(order: Option<&Order>) -> Result<[u8; 16]> {
    order
        .map(|order| parse_bytes16_field("residual_order_id", &order.order_id))
        .transpose()
        .map(|value| value.unwrap_or([0u8; 16]))
}

fn derive_trade_id() -> [u8; 16] {
    *Uuid::new_v4().as_bytes()
}

fn to_evm_clearing_result(clearing_result: &ClearingResult) -> Result<EvmClearingResult> {
    let market_id = match clearing_result.market_id.as_deref() {
        Some(market_id) => parse_bytes16_field("clearing_result.market_id", market_id)?,
        None => [0u8; 16],
    };
    Ok(EvmClearingResult {
        market_id,
        clearing_status: clearing_result.clearing_status.to_evm(),
        clearing_price: U256::from(clearing_result.clearing_price.unwrap_or_default()),
        total_supply: U256::from(clearing_result.total_supply.unwrap_or_default()),
        total_demand: U256::from(clearing_result.total_demand.unwrap_or_default()),
        traded_quantity: U256::from(clearing_result.traded_quantity.unwrap_or_default()),
        num_trades: clearing_result.num_trades.unwrap_or_default(),
    })
}

fn to_evm_match(
    item: &BidOfferMatch,
    available_orders: &mut HashMap<[u8; 16], OrderParams>,
    known_ids: &mut HashSet<[u8; 16]>,
) -> Result<Match> {
    if item.bid.market_id != item.offer.market_id
        || item.bid.time_slot != item.offer.time_slot
        || item.market_id != item.bid.market_id
        || item.time_slot != item.bid.time_slot
    {
        return Err(anyhow!(
            "Refusing cross-market/time-slot match between bid {}/{} and offer {}/{}",
            item.bid.market_id,
            item.bid.time_slot,
            item.offer.market_id,
            item.offer.time_slot
        ));
    }

    let bid_id = parse_bytes16_field("bid.order_id", &item.bid.order_id)?;
    let offer_id = parse_bytes16_field("offer.order_id", &item.offer.order_id)?;
    let bid = available_orders
        .remove(&bid_id)
        .ok_or_else(|| anyhow!("Bid {} is unknown or already consumed", item.bid.order_id))?;
    let offer = available_orders.remove(&offer_id).ok_or_else(|| {
        anyhow!(
            "Offer {} is unknown or already consumed",
            item.offer.order_id
        )
    })?;
    if !bid.is_bid
        || offer.is_bid
        || bid.market_id != offer.market_id
        || bid.time_slot != offer.time_slot
        || bid.market_id != parse_bytes16_field("match.market_id", &item.market_id)?
        || bid.time_slot != item.time_slot
    {
        return Err(anyhow!(
            "Invalid order types or market/time-slot boundaries"
        ));
    }
    if item.selected_energy == 0
        || item.selected_energy > bid.energy
        || item.selected_energy > offer.energy
    {
        return Err(anyhow!("Invalid selected energy for match"));
    }
    let residual_bid_id = register_residual(
        &bid,
        item.residual_bid.as_ref(),
        item.selected_energy,
        available_orders,
        known_ids,
    )?;
    let residual_offer_id = register_residual(
        &offer,
        item.residual_offer.as_ref(),
        item.selected_energy,
        available_orders,
        known_ids,
    )?;

    Ok(Match {
        trade_id: derive_trade_id(),
        bid,
        offer,
        residual_bid_id,
        residual_offer_id,
        selected_energy: U256::from(item.selected_energy),
        clearing_price: U256::from(item.energy_rate),
        match_type: match item.match_type {
            MatchType::Standard => 0,
            MatchType::Preferred => 1,
        },
    })
}

fn to_evm_matches(
    matches: Vec<MarketMatches>,
    order_lookup: &HashMap<String, DbOrderSchema>,
) -> Result<Vec<MarketMatchBatch>> {
    let mut available_orders = order_lookup
        .values()
        .map(|order| {
            to_evm_order_data(order, order.order_type.clone())
                .map(|params| (params.order_id, params))
        })
        .collect::<Result<HashMap<_, _>>>()?;
    let mut known_ids = available_orders.keys().copied().collect::<HashSet<_>>();
    matches
        .into_iter()
        .map(|market| {
            let bid_offer_matches = market
                .bid_offer_matches
                .iter()
                .map(|item| to_evm_match(item, &mut available_orders, &mut known_ids))
                .collect::<Result<Vec<_>>>()?;
            Ok(MarketMatchBatch {
                matches: bid_offer_matches,
                clearing_result: to_evm_clearing_result(&market.clearing_result)?,
            })
        })
        .collect()
}

fn register_residual(
    parent: &OrderParams,
    residual: Option<&Order>,
    selected_energy: u64,
    available_orders: &mut HashMap<[u8; 16], OrderParams>,
    known_ids: &mut HashSet<[u8; 16]>,
) -> Result<[u8; 16]> {
    let remaining_energy = parent.energy - selected_energy;
    let residual_id = optional_order_id_to_bytes16(residual)?;
    if (remaining_energy == 0) != residual.is_none()
        || residual.is_some_and(|order| order.energy != remaining_energy || residual_id == [0; 16])
    {
        return Err(anyhow!("Residual must contain exactly the unfilled energy"));
    }
    if residual.is_some() {
        if !known_ids.insert(residual_id) {
            return Err(anyhow!("Residual order ID has already been used"));
        }
        // Mirror OrderRegistry::fillOrder without a floating-point round trip.
        let mut params = parent.clone();
        params.order_id = residual_id;
        params.energy = remaining_energy;
        available_orders.insert(residual_id, params);
    }
    Ok(residual_id)
}

#[cfg(test)]
#[path = "../../tests/connectors/residual_conversion.rs"]
mod residual_conversion_tests;

#[cfg(test)]
#[path = "../../tests/connectors/order_books.rs"]
mod order_book_tests;

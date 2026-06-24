use crate::algorithms::PayAsBid;
use anyhow::{anyhow, Error, Result};
use ethers::prelude::*;
use ethers::utils::keccak256;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbAttributes, DbOrderSchema, DbRequirements, IntelligentEnergyType, OrderEnum, OrderStatus,
};
use gsy_offchain_primitives::types::{BidOfferMatch, MatchingData, Order};
use gsy_offchain_primitives::utils::{
    actor_id_to_account_id, bytes16_to_h256, h256_to_bytes16_hex, parse_uuid_or_hex_bytes16,
    string_to_account_id, NODE_FLOAT_SCALING_FACTOR,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

const MATCH_PER_NR_BLOCKS: u64 = 4;
const ENERGY_TYPE_UNSPECIFIED: u8 = 0;

abigen!(
    TradeSettlementContract,
    r#"[
        {
            "type": "function",
            "name": "hasRole",
            "stateMutability": "view",
            "inputs": [
                {"name": "role", "type": "bytes32"},
                {"name": "account", "type": "address"}
            ],
            "outputs": [{"name": "", "type": "bool"}]
        },
        {
            "type": "function",
            "name": "settleBatch",
            "stateMutability": "nonpayable",
            "inputs": [
                {
                    "name": "matches",
                    "type": "tuple[]",
                    "components": [
                        {
                            "name": "tradeId",
                            "type": "bytes16"
                        },
                        {
                            "name": "bid",
                            "type": "tuple",
                            "components": [
                                {"name": "orderId", "type": "bytes16"},
                                {"name": "createdBy", "type": "bytes16"},
                                {"name": "marketId", "type": "bytes16"},
                                {"name": "timeSlot", "type": "uint64"},
                                {"name": "creationTime", "type": "uint64"},
                                {"name": "energy", "type": "uint64"},
                                {"name": "energyRate", "type": "uint64"},
                                {"name": "energySourcePreference", "type": "uint8"},
                                {"name": "energyType", "type": "uint8"}
                            ]
                        },
                        {
                            "name": "offer",
                            "type": "tuple",
                            "components": [
                                {"name": "orderId", "type": "bytes16"},
                                {"name": "createdBy", "type": "bytes16"},
                                {"name": "marketId", "type": "bytes16"},
                                {"name": "timeSlot", "type": "uint64"},
                                {"name": "creationTime", "type": "uint64"},
                                {"name": "energy", "type": "uint64"},
                                {"name": "energyRate", "type": "uint64"},
                                {"name": "energySourcePreference", "type": "uint8"},
                                {"name": "energyType", "type": "uint8"}
                            ]
                        },
                        {"name": "residualBidId", "type": "bytes16"},
                        {"name": "residualOfferId", "type": "bytes16"},
                        {"name": "selectedEnergy", "type": "uint256"},
                        {"name": "clearingPrice", "type": "uint256"}
                    ]
                }
            ],
            "outputs": []
        }
    ]"#
);

type EvmOrderDataTuple = ([u8; 16], [u8; 16], [u8; 16], u64, u64, u64, u64, u8, u8);
type EvmMatchTuple = (
    [u8; 16],
    EvmOrderDataTuple,
    EvmOrderDataTuple,
    [u8; 16],
    [u8; 16],
    U256,
    U256,
);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsRequestEnvelope {
    request_id: String,
    operation: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EwdsSendMessageDto {
    fqcn: String,
    topic_name: String,
    topic_version: String,
    topic_owner: String,
    transaction_id: String,
    payload: String,
    anonymous_recipient: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsMessageDto {
    payload: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsOrderDto {
    order_id: String,
    market_id: String,
    order_type: String,
    status: String,
    area_uuid: String,
    #[serde(default)]
    nonce: Option<u64>,
    time_slot: u64,
    creation_time: u64,
    quantity: f64,
    price_limit: f64,
    created_by: String,
    #[serde(default)]
    requirements: Option<EwdsRequirementsDto>,
    #[serde(default)]
    attributes: Option<EwdsAttributesDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsRequirementsDto {
    #[serde(default)]
    trading_partner_id: Option<String>,
    #[serde(default)]
    energy_type: Option<IntelligentEnergyType>,
    #[serde(default)]
    preferred_energy_rate: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsAttributesDto {
    #[serde(default)]
    trading_partner_id: Option<String>,
    energy_type: IntelligentEnergyType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EwdsOrdersQueryResponse {
    #[serde(alias = "request_id")]
    request_id: String,
    success: bool,
    data: Option<Vec<Value>>,
    error: Option<EwdsErrorPayload>,
}

#[derive(Deserialize)]
struct EwdsErrorPayload {
    code: String,
    message: String,
}

struct PreparedOrders {
    open_bids: Vec<Order>,
    open_offers: Vec<Order>,
    by_order_id: HashMap<String, DbOrderSchema>,
}

pub async fn evm_subscribe(
    orderbook_url: String,
    node_url: String,
    trade_settlement_address: String,
    matching_engine_private_key: String,
) -> Result<(), Error> {
    info!("Connecting to EVM node {}", node_url);
    let provider = Provider::<Ws>::connect(node_url.as_str()).await?;
    let mut last_processed_block = provider.get_block_number().await?;
    // Keep track of which trigger "bucket" was already processed, so we do not
    // miss matches when multiple blocks are mined between polling iterations.
    let mut last_processed_trigger_bucket =
        last_processed_block.as_u64().saturating_sub(1) / MATCH_PER_NR_BLOCKS;

    loop {
        let block_number = provider.get_block_number().await?;
        if block_number > last_processed_block {
            info!("Block {} observed", block_number);

            let current_trigger_bucket = block_number.as_u64() / MATCH_PER_NR_BLOCKS;
            if current_trigger_bucket > last_processed_trigger_bucket {
                info!(
                    "Matching trigger reached (bucket {} -> {}) at block {}",
                    last_processed_trigger_bucket, current_trigger_bucket, block_number
                );

                if let Err(error) = run_matching_cycle(
                    orderbook_url.as_str(),
                    node_url.as_str(),
                    trade_settlement_address.as_str(),
                    matching_engine_private_key.as_str(),
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
    trade_settlement_address: &str,
    matching_engine_private_key: &str,
) -> Result<()> {
    info!("Starting matching cycle");
    info!("Fetching open orders from {}", orderbook_url);

    let prepared_orders =
        fetch_open_orders_from_orderbook_service(orderbook_url.to_string()).await?;
    info!(
        "Prepared open orders: bids={}, offers={}",
        prepared_orders.open_bids.len(),
        prepared_orders.open_offers.len()
    );
    if prepared_orders.open_bids.is_empty() || prepared_orders.open_offers.is_empty() {
        info!("No open bid/offer pairs to match");
        return Ok(());
    }

    let market_id = prepared_orders.open_bids[0].market_id;
    let mut matching_data = MatchingData {
        bids: prepared_orders.open_bids,
        offers: prepared_orders.open_offers,
        market_id,
    };

    let bid_offer_matches = matching_data.pay_as_bid();
    if bid_offer_matches.is_empty() {
        info!("No matches generated by pay-as-bid algorithm");
        return Ok(());
    }

    info!("Generated {} matches", bid_offer_matches.len());
    send_settle_batch_transaction(
        evm_node_url,
        trade_settlement_address,
        matching_engine_private_key,
        bid_offer_matches,
        prepared_orders.by_order_id,
    )
    .await?;
    Ok(())
}

pub async fn send_settle_batch_transaction(
    evm_node_url: &str,
    trade_settlement_address: &str,
    matching_engine_private_key: &str,
    matches: Vec<BidOfferMatch>,
    order_lookup: HashMap<String, DbOrderSchema>,
) -> Result<()> {
    if matches.is_empty() {
        info!("No matches to settle");
        return Ok(());
    }

    let trade_settlement_address = Address::from_str(trade_settlement_address).map_err(|e| {
        anyhow!(
            "Invalid trade settlement address '{}': {}",
            trade_settlement_address,
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
    let trade_settlement = TradeSettlementContract::new(trade_settlement_address, client.clone());

    let operator_role = keccak256("OPERATOR_ROLE");
    let has_role = trade_settlement
        .has_role(operator_role, signer_address)
        .call()
        .await?;
    if !has_role {
        warn!(
            "Signer {:?} does not currently have OPERATOR_ROLE in TradeSettlement",
            signer_address
        );
    }

    info!("Submitting {} matches to settleBatch", evm_matches.len());
    let settle_batch_call = trade_settlement.settle_batch(evm_matches);
    let pending_tx = settle_batch_call.send().await?;
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
                    "settleBatch transaction {:?} reverted with status {:?}",
                    tx_hash,
                    receipt.status
                ));
            }
            info!("settleBatch successful. tx={:?}", tx_hash);
            Ok(())
        }
        None => Err(anyhow!(
            "settleBatch transaction {:?} dropped without receipt",
            tx_hash
        )),
    }
}

fn fetch_market_orders(body: Vec<DbOrderSchema>) -> PreparedOrders {
    let mut open_bids: Vec<Order> = Vec::new();
    let mut open_offers: Vec<Order> = Vec::new();
    let mut by_order_id: HashMap<String, DbOrderSchema> = HashMap::new();

    for db_order_schema in body
        .into_iter()
        .filter(|order| order.status == OrderStatus::Open)
    {
        let order_id = db_order_schema.order_id.to_ascii_lowercase();
        match convert_db_order_to_canonical(&db_order_schema) {
            Ok(order) => {
                by_order_id.insert(order_id, db_order_schema);
                match order.order_type {
                    OrderEnum::Bid => open_bids.push(order),
                    OrderEnum::Offer => open_offers.push(order),
                }
            }
            Err(e) => {
                error!("Failed to convert DB order to canonical: {:?}", e);
            }
        }
    }

    PreparedOrders {
        open_bids,
        open_offers,
        by_order_id,
    }
}

async fn fetch_open_orders_from_orderbook_service(url: String) -> Result<PreparedOrders, Error> {
    if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|value| value.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false)
    {
        info!("Fetching orders via EWDS transport");
        return fetch_open_orders_via_ewds(url).await;
    }

    let res = reqwest::get(url).await?;
    info!("Response: {:?} {}", res.version(), res.status());
    info!("Headers: {:#?}\n", res.headers());

    let body = res.json::<Vec<DbOrderSchema>>().await?;
    info!("Fetched {} total orders from orderbook", body.len());
    Ok(fetch_market_orders(body))
}

async fn fetch_open_orders_via_ewds(fallback_url: String) -> Result<PreparedOrders, Error> {
    fetch_open_orders_via_ewds_query(fallback_url).await
}

async fn fetch_open_orders_via_ewds_query(fallback_url: String) -> Result<PreparedOrders, Error> {
    let gateway_base =
        env::var("EWDS_GATEWAY_URL").unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string());
    let request_fqcn = env_var("EWDS_REQUEST_PUBLISH_FQCN")
        .or_else(|| env_var("EWDS_REQUEST_FQCN"))
        .unwrap_or_else(|| "gsy.intelligent.requests.pub".to_string());
    let response_fqcn = env_var("EWDS_RESPONSE_SUBSCRIBE_FQCN")
        .or_else(|| env_var("EWDS_RESPONSE_FQCN"))
        .unwrap_or_else(|| "gsy.intelligent.responses.sub".to_string());
    let topic_owner = env::var("EWDS_TOPIC_OWNER")
        .unwrap_or_else(|_| "integration.apps.intelligent.auth.ewc".to_string());
    let topic_version = env::var("EWDS_TOPIC_VERSION").unwrap_or_else(|_| "1.0.0".to_string());
    let response_client_id = env_var("EWDS_MATCHING_ENGINE_CLIENT_ID")
        .or_else(|| env_var("EWDS_RESPONSE_CLIENT_ID"))
        .unwrap_or_else(|| "gsymatchingengine".to_string());
    let request_topic =
        env::var("EWDS_ORDERS_REQUEST_TOPIC").unwrap_or_else(|_| "ordersQuery".to_string());
    let response_topic = env::var("EWDS_ORDERS_RESPONSE_TOPIC")
        .unwrap_or_else(|_| "ordersQueryResponse".to_string());

    let timeout_ms = env::var("EWDS_RESPONSE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60_000);
    let poll_interval_ms = env::var("EWDS_RESPONSE_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(400);

    let request_id = format!(
        "orders-query-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    );

    let query_payload = parse_query_params_from_url(&fallback_url);
    let envelope = EwdsRequestEnvelope {
        request_id: request_id.clone(),
        operation: "orders.query".to_string(),
        payload: query_payload,
    };

    let send_message_body = EwdsSendMessageDto {
        fqcn: request_fqcn,
        topic_name: request_topic,
        topic_version,
        topic_owner: topic_owner.clone(),
        transaction_id: request_id.clone(),
        payload: serde_json::to_string(&envelope)?,
        anonymous_recipient: Vec::new(),
    };

    let client = reqwest::Client::new();
    let post_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    info!(
        "Publishing EWDS orders.query request (request_id={}, topic={}, response_topic={})",
        request_id, send_message_body.topic_name, response_topic
    );
    let send_response = client
        .post(post_url)
        .json(&send_message_body)
        .send()
        .await?;
    let send_status = send_response.status();
    if !send_status.is_success() {
        let body = send_response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "EWDS message send failed for orders.query: HTTP {}{}",
            send_status,
            format_response_body(&body)
        ));
    }

    let started = Instant::now();
    let get_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    let poll_client_id = client_id_for_suffix(response_client_id.as_str(), response_topic.as_str());
    loop {
        if started.elapsed().as_millis() as u64 > timeout_ms {
            return Err(anyhow!(
                "EWDS timeout waiting for orders.query response (request_id={})",
                request_id
            ));
        }

        let mut poll_url = reqwest::Url::parse(get_url.as_str())?;
        {
            let mut query = poll_url.query_pairs_mut();
            query.append_pair("fqcn", response_fqcn.as_str());
            query.append_pair("amount", "100");
            query.append_pair("topicName", response_topic.as_str());
            query.append_pair("topicOwner", topic_owner.as_str());
            query.append_pair("clientId", poll_client_id.as_str());
        }

        let response = client.get(poll_url).send().await?;

        let status = response.status();
        if status.is_success() {
            let messages = response
                .json::<Vec<EwdsMessageDto>>()
                .await
                .unwrap_or_default();
            for message in messages {
                let parsed = serde_json::from_str::<EwdsOrdersQueryResponse>(&message.payload);
                if let Ok(parsed_payload) = parsed {
                    if parsed_payload.request_id == request_id {
                        if !parsed_payload.success {
                            let error_message = parsed_payload
                                .error
                                .map(|error| format!("{}: {}", error.code, error.message))
                                .unwrap_or_else(|| "Unknown EWDS error".to_string());
                            return Err(anyhow!(
                                "EWDS orders.query returned error (request_id={}): {}",
                                request_id,
                                error_message
                            ));
                        }
                        let orders = parsed_payload
                            .data
                            .and_then(parse_order_values)
                            .unwrap_or_default();
                        info!("Fetched {} total orders from EWDS", orders.len());
                        return Ok(fetch_market_orders(orders));
                    }
                }
            }
        } else {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "EWDS response poll failed for orders.query (request_id={}): HTTP {}{}",
                request_id,
                status,
                format_response_body(&body)
            ));
        }

        sleep(Duration::from_millis(poll_interval_ms)).await;
    }
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

fn format_response_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return String::new();
    }

    let max_chars = 1_024;
    let truncated = compact.chars().take(max_chars).collect::<String>();
    if compact.chars().count() > max_chars {
        format!(": {}...", truncated)
    } else {
        format!(": {}", truncated)
    }
}

fn env_var(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn client_id_for_suffix(base: &str, suffix: &str) -> String {
    let mut value = String::with_capacity(base.len() + suffix.len());
    value.extend(base.chars().filter(|ch| ch.is_ascii_alphanumeric()));
    value.extend(suffix.chars().filter(|ch| ch.is_ascii_alphanumeric()));

    if value.is_empty() {
        "gsymatchingengine".to_string()
    } else {
        value
    }
}

fn parse_order_values(values: Vec<Value>) -> Option<Vec<DbOrderSchema>> {
    let original_len = values.len();
    let orders = values
        .into_iter()
        .filter_map(|value| match parse_order_value(value) {
            Ok(order) => Some(order),
            Err(error) => {
                warn!("Skipping EWDS order payload: {}", error);
                None
            }
        })
        .collect::<Vec<_>>();

    if original_len == 0 || !orders.is_empty() {
        Some(orders)
    } else {
        None
    }
}

fn parse_order_value(value: Value) -> Result<DbOrderSchema> {
    if let Ok(order) = serde_json::from_value::<DbOrderSchema>(value.clone()) {
        return Ok(order);
    }

    let dto = serde_json::from_value::<EwdsOrderDto>(value)?;
    ewds_order_to_db(dto)
}

fn ewds_order_to_db(order: EwdsOrderDto) -> Result<DbOrderSchema> {
    let requirements = match order.requirements {
        Some(requirements) => Some(DbRequirements {
            trading_partner_id: requirements.trading_partner_id,
            energy_type: requirements.energy_type,
            preferred_energy_rate: requirements.preferred_energy_rate,
        }),
        None => None,
    };

    let attributes = match order.attributes {
        Some(attributes) => Some(DbAttributes {
            trading_partner_id: attributes.trading_partner_id,
            energy_type: attributes.energy_type,
        }),
        None => None,
    };

    Ok(DbOrderSchema {
        order_id: order.order_id,
        status: ewds_order_status_to_db(order.status.as_str())?,
        order_type: ewds_order_type_to_db(order.order_type.as_str())?,
        area_uuid: order.area_uuid,
        market_id: order.market_id,
        nonce: order.nonce,
        time_slot: order.time_slot,
        creation_time: order.creation_time,
        energy_kWh: order.quantity,
        energy_rate: order.price_limit,
        created_by: order.created_by,
        requirements,
        attributes,
    })
}

fn ewds_order_type_to_db(value: &str) -> Result<OrderEnum> {
    match value.to_ascii_lowercase().as_str() {
        "bid" => Ok(OrderEnum::Bid),
        "offer" => Ok(OrderEnum::Offer),
        _ => Err(anyhow!("unsupported EWDS order type '{}'", value)),
    }
}

fn ewds_order_status_to_db(value: &str) -> Result<OrderStatus> {
    match value.to_ascii_lowercase().as_str() {
        "open" => Ok(OrderStatus::Open),
        "executed" => Ok(OrderStatus::Executed),
        "expired" => Ok(OrderStatus::Expired),
        "deleted" => Ok(OrderStatus::Deleted),
        _ => Err(anyhow!("unsupported EWDS order status '{}'", value)),
    }
}

fn parse_bytes16_field(field_name: &str, value: &str) -> Result<[u8; 16]> {
    parse_uuid_or_hex_bytes16(value)
        .ok_or_else(|| anyhow!("{} must be a UUID or 0x-prefixed 16-byte hex", field_name))
}

fn convert_db_order_to_canonical(order: &DbOrderSchema) -> Result<Order> {
    let parse_account_or_address = |value: &str| {
        string_to_account_id(value.to_string()).or_else(|| actor_id_to_account_id(value))
    };

    let order_id = bytes16_to_h256(parse_bytes16_field("order_id", &order.order_id)?);
    let market_id = bytes16_to_h256(parse_bytes16_field("market_id", &order.market_id)?);
    let area_uuid = bytes16_to_h256(parse_bytes16_field("area_uuid", &order.area_uuid)?);

    Ok(match order.order_type {
        OrderEnum::Bid => Order {
            created_by: parse_account_or_address(order.created_by.as_str())
                .ok_or_else(|| anyhow!("Invalid buyer actor/account: {}", order.created_by))?,
            order_id,
            order_type: OrderEnum::Bid,
            status: order.status.clone(),
            area_uuid,
            market_id,
            time_slot: order.time_slot,
            creation_time: order.creation_time,
            energy: (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            energy_rate: (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            requirements: order.requirements.as_ref().map(|r| {
                gsy_offchain_primitives::types::Requirements {
                    trading_partner_id: r
                        .trading_partner_id
                        .as_deref()
                        .and_then(parse_account_or_address),
                    energy_type: r.energy_type.clone(),
                    preferred_energy_rate: r
                        .preferred_energy_rate
                        .map(|rate| (rate * NODE_FLOAT_SCALING_FACTOR).round() as u64),
                }
            }),
            attributes: None,
        },
        OrderEnum::Offer => Order {
            order_id,
            order_type: order.order_type.clone(),
            status: order.status.clone(),
            created_by: parse_account_or_address(order.created_by.as_str())
                .ok_or_else(|| anyhow!("Invalid seller actor/account: {}", order.created_by))?,
            area_uuid,
            market_id,
            time_slot: order.time_slot,
            creation_time: order.creation_time,
            energy: (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            energy_rate: (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            requirements: None,
            attributes: order.attributes.as_ref().map(|a| {
                gsy_offchain_primitives::types::Attributes {
                    trading_partner_id: a
                        .trading_partner_id
                        .as_deref()
                        .and_then(parse_account_or_address),
                    energy_type: a.energy_type.clone(),
                }
            }),
        },
    })
}

fn energy_type_to_contract(energy_type: &IntelligentEnergyType) -> u8 {
    match energy_type {
        IntelligentEnergyType::Green => 1,
        IntelligentEnergyType::Pv => 2,
        IntelligentEnergyType::Hydro => 3,
        IntelligentEnergyType::Biomass => 4,
        IntelligentEnergyType::Battery => 5,
        IntelligentEnergyType::Grey => 6,
    }
}

fn order_energy_source_preference(order: &DbOrderSchema) -> u8 {
    order
        .requirements
        .as_ref()
        .and_then(|requirements| requirements.energy_type.as_ref())
        .map(energy_type_to_contract)
        .unwrap_or(ENERGY_TYPE_UNSPECIFIED)
}

fn order_energy_type(order: &DbOrderSchema) -> u8 {
    order
        .attributes
        .as_ref()
        .map(|attributes| energy_type_to_contract(&attributes.energy_type))
        .unwrap_or(ENERGY_TYPE_UNSPECIFIED)
}

fn to_evm_order_data(order: &DbOrderSchema, expected_type: OrderEnum) -> Result<EvmOrderDataTuple> {
    if order.order_type != expected_type {
        return Err(anyhow!(
            "Order {} type mismatch. Expected {:?}, got {:?}",
            order.order_id,
            expected_type,
            order.order_type
        ));
    }

    Ok((
        parse_bytes16_field("order_id", order.order_id.as_str())?,
        parse_bytes16_field("created_by", order.created_by.as_str())?,
        parse_bytes16_field("market_id", order.market_id.as_str())?,
        order.time_slot,
        order.creation_time,
        (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        order_energy_source_preference(order),
        order_energy_type(order),
    ))
}

fn optional_order_id_to_bytes16(order: Option<&Order>) -> [u8; 16] {
    order
        .map(|order| {
            order.order_id.as_bytes()[..16]
                .try_into()
                .expect("order id prefix is 16 bytes")
        })
        .unwrap_or([0u8; 16])
}

fn derive_trade_id(
    bid_id: &str,
    offer_id: &str,
    selected_energy: u64,
    energy_rate: u64,
) -> [u8; 16] {
    let hash = keccak256(
        format!(
            "{}:{}:{}:{}",
            bid_id, offer_id, selected_energy, energy_rate
        )
        .as_bytes(),
    );
    hash[..16].try_into().expect("hash prefix is 16 bytes")
}

fn to_evm_matches(
    matches: Vec<BidOfferMatch>,
    order_lookup: &HashMap<String, DbOrderSchema>,
) -> Result<Vec<EvmMatchTuple>> {
    matches
        .into_iter()
        .map(|item| {
            let bid_id = h256_to_bytes16_hex(item.bid.order_id).to_ascii_lowercase();
            let offer_id = h256_to_bytes16_hex(item.offer.order_id).to_ascii_lowercase();

            let bid_order = order_lookup
                .get(&bid_id)
                .ok_or_else(|| anyhow!("Could not find bid order '{}' in lookup map", bid_id))?;
            let offer_order = order_lookup.get(&offer_id).ok_or_else(|| {
                anyhow!("Could not find offer order '{}' in lookup map", offer_id)
            })?;

            Ok((
                derive_trade_id(&bid_id, &offer_id, item.selected_energy, item.energy_rate),
                to_evm_order_data(bid_order, OrderEnum::Bid)?,
                to_evm_order_data(offer_order, OrderEnum::Offer)?,
                optional_order_id_to_bytes16(item.residual_bid.as_ref()),
                optional_order_id_to_bytes16(item.residual_offer.as_ref()),
                U256::from(item.selected_energy),
                U256::from(item.energy_rate),
            ))
        })
        .collect()
}

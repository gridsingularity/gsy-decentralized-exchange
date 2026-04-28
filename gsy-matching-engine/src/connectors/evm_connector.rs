use crate::algorithms::PayAsBid;
use anyhow::{anyhow, Error, Result};
use ethers::prelude::*;
use ethers::utils::keccak256;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbOrderSchema, EnergyType, OrderEnum, OrderStatus,
};
use gsy_offchain_primitives::types::{BidOfferMatch, MatchingData, Order};
use gsy_offchain_primitives::utils::{
    evm_address_to_account_id, h256_to_string, string_to_account_id, string_to_h256,
    NODE_FLOAT_SCALING_FACTOR,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

const MATCH_PER_NR_BLOCKS: u64 = 4;

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
                            "name": "bid",
                            "type": "tuple",
                            "components": [
                                {"name": "owner", "type": "address"},
                                {"name": "nonce", "type": "uint64"},
                                {"name": "areaUuid", "type": "bytes32"},
                                {"name": "marketId", "type": "bytes32"},
                                {"name": "timeSlot", "type": "uint64"},
                                {"name": "creationTime", "type": "uint64"},
                                {"name": "energy", "type": "uint64"},
                                {"name": "energyRate", "type": "uint64"}
                            ]
                        },
                        {
                            "name": "ask",
                            "type": "tuple",
                            "components": [
                                {"name": "owner", "type": "address"},
                                {"name": "nonce", "type": "uint64"},
                                {"name": "areaUuid", "type": "bytes32"},
                                {"name": "marketId", "type": "bytes32"},
                                {"name": "timeSlot", "type": "uint64"},
                                {"name": "creationTime", "type": "uint64"},
                                {"name": "energy", "type": "uint64"},
                                {"name": "energyRate", "type": "uint64"}
                            ]
                        },
                        {"name": "selectedEnergy", "type": "uint256"},
                        {"name": "clearingPrice", "type": "uint256"}
                    ]
                }
            ],
            "outputs": []
        }
    ]"#
);

type EvmOrderDataTuple = (Address, u64, [u8; 32], [u8; 32], u64, u64, u64, u64);
type EvmMatchTuple = (EvmOrderDataTuple, EvmOrderDataTuple, U256, U256);

#[derive(Serialize)]
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
struct EwdsOrdersQueryResponse {
    request_id: String,
    success: bool,
    data: Option<Vec<DbOrderSchema>>,
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
    let gateway_base =
        env::var("EWDS_GATEWAY_URL").unwrap_or_else(|_| "http://ewds-gateway-api:3333".to_string());
    let request_fqcn =
        env::var("EWDS_REQUEST_FQCN").unwrap_or_else(|_| "gsy.dex.offchain.request".to_string());
    let response_fqcn =
        env::var("EWDS_RESPONSE_FQCN").unwrap_or_else(|_| "gsy.dex.offchain.response".to_string());
    let topic_owner =
        env::var("EWDS_TOPIC_OWNER").unwrap_or_else(|_| "gsy.dex.offchain-storage".to_string());
    let request_topic =
        env::var("EWDS_ORDERS_REQUEST_TOPIC").unwrap_or_else(|_| "orders.query".to_string());
    let response_topic = env::var("EWDS_ORDERS_RESPONSE_TOPIC")
        .unwrap_or_else(|_| "orders.query.response".to_string());

    let timeout_ms = env::var("EWDS_RESPONSE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(8_000);
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
        topic_version: "1.0.0".to_string(),
        topic_owner: topic_owner.clone(),
        transaction_id: request_id.clone(),
        payload: serde_json::to_string(&envelope)?,
        anonymous_recipient: Vec::new(),
    };

    let client = reqwest::Client::new();
    let post_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
    let send_response = client
        .post(post_url)
        .json(&send_message_body)
        .send()
        .await?;
    if !send_response.status().is_success() {
        return Err(anyhow!(
            "EWDS message send failed for orders.query: HTTP {}",
            send_response.status()
        ));
    }

    let started = Instant::now();
    let get_url = format!("{}/api/v2/messages", gateway_base.trim_end_matches('/'));
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
        }

        let response = client.get(poll_url).send().await?;

        if response.status().is_success() {
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
                        let orders = parsed_payload.data.unwrap_or_default();
                        info!("Fetched {} total orders from EWDS", orders.len());
                        return Ok(fetch_market_orders(orders));
                    }
                }
            }
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

fn validate_h256_hex(field_name: &str, value: &str) -> Result<()> {
    if value.len() != 66 || !value.starts_with("0x") {
        return Err(anyhow!("{} must be a 0x-prefixed 32-byte hex", field_name));
    }
    if !value[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "{} must contain only hexadecimal characters",
            field_name
        ));
    }
    Ok(())
}

fn convert_db_order_to_canonical(order: &DbOrderSchema) -> Result<Order> {
    let parse_account_or_address = |value: &str| {
        string_to_account_id(value.to_string()).or_else(|| evm_address_to_account_id(value))
    };

    validate_h256_hex("order_id", order.order_id.as_str())?;
    validate_h256_hex("area_uuid", order.area_uuid.as_str())?;
    validate_h256_hex("market_id", order.market_id.as_str())?;

    Ok(match order.order_type {
        OrderEnum::Bid => Order {
            created_by: parse_account_or_address(order.created_by.as_str())
                .ok_or_else(|| anyhow!("Invalid buyer account/address: {}", order.created_by))?,
            order_id: string_to_h256(order.order_id.clone()),
            order_type: OrderEnum::Bid,
            status: order.status.clone(),
            area_uuid: string_to_h256(order.area_uuid.clone()),
            market_id: string_to_h256(order.market_id.clone()),
            time_slot: order.time_slot,
            creation_time: order.creation_time,
            energy: (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            energy_rate: (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
            requirements: order.requirements.as_ref().map(|r| {
                gsy_offchain_primitives::types::Requirements {
                    trading_partner_id: r
                        .trading_partner_id
                        .as_deref()
                        .and_then(|value| parse_account_or_address(value)),
                    energy_type: r.energy_type.as_ref().map(map_energy_type),
                    preferred_energy_rate: r
                        .preferred_energy_rate
                        .map(|rate| (rate * NODE_FLOAT_SCALING_FACTOR).round() as u64),
                }
            }),
            attributes: None,
        },
        OrderEnum::Offer => Order {
            order_id: string_to_h256(order.order_id.clone()),
            order_type: order.order_type.clone(),
            status: order.status.clone(),
            created_by: parse_account_or_address(order.created_by.as_str())
                .ok_or_else(|| anyhow!("Invalid seller account/address: {}", order.created_by))?,
            area_uuid: string_to_h256(order.area_uuid.clone()),
            market_id: string_to_h256(order.market_id.clone()),
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
                        .and_then(|value| parse_account_or_address(value)),
                    energy_type: map_energy_type(&a.energy_type),
                }
            }),
        },
    })
}

fn map_energy_type(energy_type: &EnergyType) -> gsy_offchain_primitives::types::EnergyType {
    match energy_type {
        EnergyType::Clean => gsy_offchain_primitives::types::EnergyType::Clean,
        EnergyType::Battery => gsy_offchain_primitives::types::EnergyType::Battery,
        EnergyType::FossilFuel => gsy_offchain_primitives::types::EnergyType::FossilFuel,
        EnergyType::Import => gsy_offchain_primitives::types::EnergyType::Import,
    }
}

fn parse_evm_bytes32(field_name: &str, value: &str) -> Result<[u8; 32]> {
    let parsed = H256::from_str(value)
        .map_err(|e| anyhow!("Invalid {} '{}' for EVM bytes32: {}", field_name, value, e))?;
    Ok(parsed.to_fixed_bytes())
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

    let owner = Address::from_str(order.created_by.as_str())
        .map_err(|e| anyhow!("Invalid owner address '{}': {}", order.created_by, e))?;

    Ok((
        owner,
        order.nonce.unwrap_or(0),
        parse_evm_bytes32("area_uuid", order.area_uuid.as_str())?,
        parse_evm_bytes32("market_id", order.market_id.as_str())?,
        order.time_slot,
        order.creation_time,
        (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
    ))
}

fn to_evm_matches(
    matches: Vec<BidOfferMatch>,
    order_lookup: &HashMap<String, DbOrderSchema>,
) -> Result<Vec<EvmMatchTuple>> {
    matches
        .into_iter()
        .map(|item| {
            let bid_id = h256_to_string(item.bid.order_id).to_ascii_lowercase();
            let ask_id = h256_to_string(item.offer.order_id).to_ascii_lowercase();

            let bid_order = order_lookup
                .get(&bid_id)
                .ok_or_else(|| anyhow!("Could not find bid order '{}' in lookup map", bid_id))?;
            let ask_order = order_lookup
                .get(&ask_id)
                .ok_or_else(|| anyhow!("Could not find ask order '{}' in lookup map", ask_id))?;

            Ok((
                to_evm_order_data(bid_order, OrderEnum::Bid)?,
                to_evm_order_data(ask_order, OrderEnum::Offer)?,
                U256::from(item.selected_energy),
                U256::from(item.energy_rate),
            ))
        })
        .collect()
}

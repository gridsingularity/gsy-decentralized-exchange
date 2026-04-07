use crate::world::MyWorld;
use cucumber::{then, when};
use ethers::abi::{encode, Token};
use ethers::prelude::*;
use ethers::utils::keccak256;
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus,
};
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use gsy_offchain_primitives::utils::NODE_FLOAT_SCALING_FACTOR;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::info;

const MATCHING_ENGINE_BLOCK_INTERVAL: u64 = 4;
const FLOAT_EPSILON: f64 = 0.000_001;

type EvmOrderParamsTuple = (Address, u64, [u8; 32], [u8; 32], u64, u64, u64, u64, bool);

abigen!(
    OrderRegistryContract,
    r#"[
        {
            "type": "function",
            "name": "getStatus",
            "stateMutability": "view",
            "inputs": [
                {"name": "orderHash", "type": "bytes32"}
            ],
            "outputs": [{"name": "", "type": "uint8"}]
        },
        {
            "type": "function",
            "name": "placeOrder",
            "stateMutability": "nonpayable",
            "inputs": [
                {
                    "name": "params",
                    "type": "tuple",
                    "components": [
                        {"name": "owner", "type": "address"},
                        {"name": "nonce", "type": "uint64"},
                        {"name": "areaUuid", "type": "bytes32"},
                        {"name": "marketId", "type": "bytes32"},
                        {"name": "timeSlot", "type": "uint64"},
                        {"name": "creationTime", "type": "uint64"},
                        {"name": "energy", "type": "uint64"},
                        {"name": "energyRate", "type": "uint64"},
                        {"name": "isBid", "type": "bool"}
                    ]
                }
            ],
            "outputs": []
        }
    ]"#
);

abigen!(
    TradeSettlementContract,
    r#"[
        function penaltyEnergyByTrade(bytes32 tradeId) external view returns (uint256)
    ]"#
);

async fn emit_activity_blocks(world: &MyWorld, count: usize) {
    let wallet = world.wallet_for_user("alice");
    let signer = Arc::new(SignerMiddleware::new(
        world.provider.clone(),
        wallet.clone(),
    ));

    for _ in 0..count {
        let pending_tx = signer
            .send_transaction(
                TransactionRequest::new()
                    .to(wallet.address())
                    .value(U256::from(1u64)),
                None,
            )
            .await
            .expect("Failed to emit synthetic activity transaction");

        pending_tx
            .await
            .expect("Failed to await synthetic activity receipt");
    }
}

async fn emit_until_matching_block(world: &MyWorld, max_blocks: usize) {
    for _ in 0..max_blocks {
        emit_activity_blocks(world, 1).await;
        let latest_block = world
            .provider
            .get_block_number()
            .await
            .expect("Failed to read latest block after synthetic activity");
        if latest_block.as_u64() % MATCHING_ENGINE_BLOCK_INTERVAL == 0 {
            info!(
                "Reached matching trigger block {} (mod {} == 0)",
                latest_block, MATCHING_ENGINE_BLOCK_INTERVAL
            );
            return;
        }
    }

    panic!(
        "Could not reach a matching trigger block after emitting {} synthetic blocks",
        max_blocks
    );
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= FLOAT_EPSILON
}

fn address_to_full_hex(address: Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

fn parse_or_hash_bytes32(value: &str) -> [u8; 32] {
    if value.starts_with("0x") && value.len() == 66 {
        if let Ok(parsed) = H256::from_str(value) {
            return parsed.to_fixed_bytes();
        }
    }

    keccak256(value.as_bytes())
}

fn compute_order_hash(params: &EvmOrderParamsTuple) -> H256 {
    let encoded = encode(&[
        Token::Address(params.0),
        Token::Uint(U256::from(params.1)),
        Token::FixedBytes(params.2.to_vec()),
        Token::FixedBytes(params.3.to_vec()),
        Token::Uint(U256::from(params.4)),
        Token::Uint(U256::from(params.5)),
        Token::Uint(U256::from(params.6)),
        Token::Uint(U256::from(params.7)),
        Token::Bool(params.8),
    ]);

    H256::from(keccak256(encoded))
}

fn market_id_as_hex(world: &MyWorld) -> String {
    format!(
        "0x{}",
        hex::encode(world.last_market_id.expect("Missing market id"))
    )
}

fn market_window(world: &MyWorld) -> (u32, u32) {
    (
        world.target_delivery_time as u32,
        (world.target_delivery_time + 900) as u32,
    )
}

async fn query_market_orders(world: &MyWorld) -> Vec<DbOrderSchema> {
    let (start_time, end_time) = market_window(world);
    let market_id = market_id_as_hex(world);

    let response = world
        .http_client
        .get(format!(
            "{}/orders?market_id={}&start_time={}&end_time={}",
            world.orderbook_service_url, market_id, start_time, end_time
        ))
        .send()
        .await
        .expect("Failed to query orders endpoint");

    assert!(
        response.status().is_success(),
        "Order query failed with status {}",
        response.status()
    );

    response
        .json::<Vec<DbOrderSchema>>()
        .await
        .expect("Failed to parse orders response")
}

async fn query_market_trades(world: &MyWorld) -> Vec<TradeSchema> {
    let (start_time, end_time) = market_window(world);

    let response = world
        .http_client
        .get(format!(
            "{}/trades?start_time={}&end_time={}",
            world.orderbook_service_url, start_time, end_time
        ))
        .send()
        .await
        .expect("Failed to query trades endpoint");

    assert!(
        response.status().is_success(),
        "Trade query failed with status {}",
        response.status()
    );

    response
        .json::<Vec<TradeSchema>>()
        .await
        .expect("Failed to parse trades response")
}

async fn wait_for_order_in_orderbook(world: &MyWorld, order_id: &str) -> DbOrderSchema {
    for _ in 0..40 {
        let orders = query_market_orders(world).await;
        if let Some(order) = orders
            .into_iter()
            .find(|order| order.order_id.eq_ignore_ascii_case(order_id))
        {
            return order;
        }

        sleep(Duration::from_millis(500)).await;
    }

    panic!(
        "Timeout: order {} was not indexed in orderbook service",
        order_id
    );
}

async fn upsert_order_in_orderbook(world: &MyWorld, order: DbOrderSchema) {
    let response = world
        .http_client
        .post(format!("{}/orders", world.orderbook_service_url))
        .json(&vec![order])
        .send()
        .await
        .expect("Failed to upsert order in orderbook service");

    assert!(
        response.status().is_success(),
        "Order upsert failed with status {}",
        response.status()
    );
}

async fn place_custom_order(
    world: &MyWorld,
    user_name: &str,
    is_bid: bool,
    energy: f64,
    energy_rate: f64,
    requirements: Option<DbRequirements>,
    attributes: Option<DbAttributes>,
) -> String {
    let wallet = world.wallet_for_user(user_name);
    let signer = Arc::new(SignerMiddleware::new(
        world.provider.clone(),
        wallet.clone(),
    ));
    let order_registry = OrderRegistryContract::new(world.order_registry_address, signer.clone());

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock before UNIX_EPOCH");
    let creation_time = now.as_secs();
    let nonce = now.as_micros() as u64;

    let area_uuid = parse_or_hash_bytes32(format!("area{}", user_name).as_str());
    let market_id = world.last_market_id.expect("Missing market id");

    let params: EvmOrderParamsTuple = (
        wallet.address(),
        nonce,
        area_uuid,
        market_id,
        world.target_delivery_time,
        creation_time,
        (energy * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        (energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        is_bid,
    );

    let order_hash = compute_order_hash(&params);
    let order_id = format!("0x{}", hex::encode(order_hash.as_bytes()));

    let place_order_call = order_registry.place_order(params);
    let pending_tx = place_order_call
        .send()
        .await
        .expect("Failed to submit custom placeOrder transaction");

    let receipt = pending_tx
        .await
        .expect("Failed to await custom placeOrder receipt");
    assert!(
        receipt.is_some(),
        "Custom placeOrder tx was dropped without receipt"
    );

    if requirements.is_some() || attributes.is_some() {
        let mut indexed_order = wait_for_order_in_orderbook(world, order_id.as_str()).await;
        indexed_order.requirements = requirements;
        indexed_order.attributes = attributes;
        upsert_order_in_orderbook(world, indexed_order).await;
    }

    order_id
}

#[when(regex = r#"^"([^"]*)" submits a bid$"#)]
async fn submit_bid(world: &mut MyWorld, user_name: String) {
    publish_orders(
        world.evm_node_url.clone(),
        vec![world.bid_forecast.clone().expect("Missing bid forecast")],
        world
            .topology_schema
            .clone()
            .expect("Missing topology schema"),
        address_to_full_hex(world.order_registry_address),
        world.private_key_for_user(user_name.as_str()),
    )
    .await
    .expect("Failed to publish bid order");
}

#[when(
    regex = r#"^"([^"]*)" submits a bid for (\d+) energy at a normal rate of (\d+), with a preferred rate of (\d+) for partner "([^"]*)"$"#
)]
async fn submit_preferred_partner_bid(
    world: &mut MyWorld,
    user_name: String,
    energy: f64,
    normal_rate: f64,
    preferred_rate: f64,
    partner_name: String,
) {
    let partner_address = world
        .users
        .get(&partner_name)
        .unwrap_or_else(|| panic!("Unknown partner '{}'", partner_name))
        .address;

    let requirements = DbRequirements {
        trading_partner_id: Some(address_to_full_hex(partner_address)),
        energy_type: None,
        preferred_energy_rate: Some(preferred_rate),
    };

    place_custom_order(
        world,
        user_name.as_str(),
        true,
        energy,
        normal_rate,
        Some(requirements),
        None,
    )
    .await;
}

#[when(regex = r#"^"([^"]*)" submits an offer$"#)]
async fn submit_offer(world: &mut MyWorld, user_name: String) {
    publish_orders(
        world.evm_node_url.clone(),
        vec![world
            .offer_forecast
            .clone()
            .expect("Missing offer forecast")],
        world
            .topology_schema
            .clone()
            .expect("Missing topology schema"),
        address_to_full_hex(world.order_registry_address),
        world.private_key_for_user(user_name.as_str()),
    )
    .await
    .expect("Failed to publish offer order");

    // Matching engine only runs on specific block boundaries. Emit synthetic txs
    // until we hit that boundary after both orders are in the registry.
    emit_until_matching_block(world, 12).await;
}

#[when(
    regex = r#"^"([^"]*)" submits an offer for (\d+) energy at a normal rate of (\d+), with a preferred rate of (\d+) for partner "([^"]*)"$"#
)]
async fn submit_preferred_partner_offer(
    world: &mut MyWorld,
    user_name: String,
    energy: f64,
    normal_rate: f64,
    _preferred_rate: f64,
    partner_name: String,
) {
    let partner_address = world
        .users
        .get(&partner_name)
        .unwrap_or_else(|| panic!("Unknown partner '{}'", partner_name))
        .address;

    let attributes = DbAttributes {
        trading_partner_id: Some(address_to_full_hex(partner_address)),
        energy_type: EnergyType::Clean,
    };

    place_custom_order(
        world,
        user_name.as_str(),
        false,
        energy,
        normal_rate,
        None,
        Some(attributes),
    )
    .await;
}

#[when(
    regex = r#"^"([^"]*)" submits a cheaper open-market offer for (\d+) energy at a rate of (\d+)$"#
)]
async fn submit_cheaper_offer(world: &mut MyWorld, user_name: String, energy: f64, rate: f64) {
    let order_id =
        place_custom_order(world, user_name.as_str(), false, energy, rate, None, None).await;
    world.last_charlie_offer_order_id = Some(order_id);

    // Trigger matching after all preference/open-market orders were submitted.
    emit_until_matching_block(world, 12).await;
}

#[when(regex = r#"^measurements for "([^"]*)" and "([^"]*)" assets are submitted$"#)]
async fn submit_measurements(world: &mut MyWorld, _user1: String, _user2: String) {
    let adapter = AreaMarketInfoAdapter::new(Some(world.orderbook_service_url.clone()));
    let buyer_area_id = format!("0x{}", hex::encode(keccak256(world.buyer_id.as_bytes())));
    let seller_area_id = format!("0x{}", hex::encode(keccak256(world.seller_id.as_bytes())));

    let measurements = vec![
        MeasurementSchema {
            area_uuid: buyer_area_id,
            community_uuid: "community1".to_string(),
            energy_kwh: 12.0,
            time_slot: world.target_delivery_time,
            creation_time: 1,
        },
        MeasurementSchema {
            area_uuid: seller_area_id,
            community_uuid: "community1".to_string(),
            energy_kwh: -8.0,
            time_slot: world.target_delivery_time,
            creation_time: 1,
        },
    ];

    adapter
        .forward_measurement(measurements)
        .await
        .expect("Failed to submit measurements");
}

#[then("the matching engine matches the bid and offer and a trade is settled on-chain")]
async fn verify_trade_on_chain(world: &mut MyWorld) {
    let order_registry =
        OrderRegistryContract::new(world.order_registry_address, world.provider.clone());

    let expected_market_id = market_id_as_hex(world).to_lowercase();

    for attempt in 0..60 {
        let trades = query_market_trades(world).await;

        if let Some(trade) = trades
            .into_iter()
            .find(|trade| trade.market_id.to_lowercase() == expected_market_id)
        {
            info!("Found settled trade {}", trade.trade_uuid);
            world.last_trade = Some(trade.clone());

            let bid_hash = H256::from_str(trade.bid_hash.as_str())
                .expect("Invalid bid hash in trade")
                .to_fixed_bytes();
            let ask_hash = H256::from_str(trade.offer_hash.as_str())
                .expect("Invalid ask hash in trade")
                .to_fixed_bytes();

            let bid_status = order_registry
                .get_status(bid_hash)
                .call()
                .await
                .expect("Failed to read bid status from contract");
            let ask_status = order_registry
                .get_status(ask_hash)
                .call()
                .await
                .expect("Failed to read ask status from contract");

            assert_eq!(bid_status, 2u8, "Bid order is not Executed on-chain");
            assert_eq!(ask_status, 2u8, "Ask order is not Executed on-chain");

            let orders = query_market_orders(world).await;

            let bid = orders
                .iter()
                .find(|order| order.order_id.eq_ignore_ascii_case(trade.bid_hash.as_str()))
                .expect("Bid order not found in orderbook DB");
            let ask = orders
                .iter()
                .find(|order| {
                    order
                        .order_id
                        .eq_ignore_ascii_case(trade.offer_hash.as_str())
                })
                .expect("Ask order not found in orderbook DB");

            assert_eq!(bid.status, OrderStatus::Executed);
            assert_eq!(ask.status, OrderStatus::Executed);

            return;
        }

        info!(
            "Trade not available yet (attempt {}/60). Retrying...",
            attempt + 1
        );
        sleep(Duration::from_secs(2)).await;
    }

    panic!("Timeout: no settled trade was indexed for the expected market");
}

#[then(regex = r#"^a trade is settled on-chain between "([^"]*)" and "([^"]*)" for (\d+) energy$"#)]
async fn verify_partner_trade(
    world: &mut MyWorld,
    buyer_name: String,
    seller_name: String,
    energy: f64,
) {
    let order_registry =
        OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    let expected_market_id = market_id_as_hex(world).to_lowercase();
    let expected_buyer = address_to_full_hex(
        world
            .users
            .get(&buyer_name)
            .unwrap_or_else(|| panic!("Unknown buyer '{}'", buyer_name))
            .address,
    );
    let expected_seller = address_to_full_hex(
        world
            .users
            .get(&seller_name)
            .unwrap_or_else(|| panic!("Unknown seller '{}'", seller_name))
            .address,
    );

    for attempt in 0..60 {
        let trades = query_market_trades(world).await;

        if let Some(trade) = trades.into_iter().find(|trade| {
            trade.market_id.to_lowercase() == expected_market_id
                && trade.buyer.eq_ignore_ascii_case(expected_buyer.as_str())
                && trade.seller.eq_ignore_ascii_case(expected_seller.as_str())
                && approx_eq(trade.parameters.selected_energy_kWh, energy)
        }) {
            world.last_trade = Some(trade.clone());

            let bid_hash = H256::from_str(trade.bid_hash.as_str())
                .expect("Invalid bid hash in trade")
                .to_fixed_bytes();
            let ask_hash = H256::from_str(trade.offer_hash.as_str())
                .expect("Invalid ask hash in trade")
                .to_fixed_bytes();

            let bid_status = order_registry
                .get_status(bid_hash)
                .call()
                .await
                .expect("Failed to read bid status from contract");
            let ask_status = order_registry
                .get_status(ask_hash)
                .call()
                .await
                .expect("Failed to read ask status from contract");

            assert_eq!(bid_status, 2u8, "Bid order is not Executed on-chain");
            assert_eq!(ask_status, 2u8, "Ask order is not Executed on-chain");
            return;
        }

        info!(
            "Preferred trade not available yet (attempt {}/60). Retrying...",
            attempt + 1
        );
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: no settled preferred trade found between {} and {}",
        buyer_name, seller_name
    );
}

#[then(regex = r#"^the trade price is exactly (\d+), matching the preferred rate$"#)]
async fn verify_trade_price(world: &mut MyWorld, expected_price: f64) {
    let trade = world
        .last_trade
        .as_ref()
        .expect("No trade was recorded in the previous step");

    assert!(
        approx_eq(trade.parameters.energy_rate, expected_price),
        "Trade price mismatch: expected {}, got {}",
        expected_price,
        trade.parameters.energy_rate
    );
}

#[then(
    regex = r#"^Bob's residual offer of (\d+) energy is available for the next matching phase$"#
)]
async fn verify_residual_offer(world: &mut MyWorld, expected_residual_energy: f64) {
    let trade = world
        .last_trade
        .as_ref()
        .expect("No trade was recorded in the previous step");

    let residual_energy = trade.offer.energy_kWh - trade.parameters.selected_energy_kWh;
    assert!(
        approx_eq(residual_energy, expected_residual_energy),
        "Residual offer mismatch: expected {}, got {}",
        expected_residual_energy,
        residual_energy
    );
}

#[then(regex = r#"^Charlie's cheaper offer remains untouched in this phase$"#)]
async fn verify_charlie_offer_untouched(world: &mut MyWorld) {
    let charlie_offer_order_id = world
        .last_charlie_offer_order_id
        .clone()
        .expect("Missing Charlie offer order id from previous step");

    let orders = query_market_orders(world).await;
    let charlie_offer = orders
        .iter()
        .find(|order| {
            order
                .order_id
                .eq_ignore_ascii_case(charlie_offer_order_id.as_str())
        })
        .expect("Charlie offer order was not found in orderbook");
    assert_eq!(
        charlie_offer.status,
        OrderStatus::Open,
        "Expected Charlie's cheaper offer to stay open after the preference match phase"
    );

    let trades = query_market_trades(world).await;
    let charlie_was_matched = trades.iter().any(|trade| {
        trade
            .offer_hash
            .eq_ignore_ascii_case(charlie_offer_order_id.as_str())
    });

    assert!(
        !charlie_was_matched,
        "Charlie offer from this scenario was unexpectedly matched in this phase"
    );
}

#[then("the execution engine submits penalties for the trade")]
async fn verify_penalties_on_chain(world: &mut MyWorld) {
    let trade = world
        .last_trade
        .clone()
        .expect("No trade captured in the previous step");
    let trade_settlement =
        TradeSettlementContract::new(world.trade_settlement_address, world.provider.clone());

    let trade_id = keccak256(trade.trade_uuid.as_bytes());

    for attempt in 0..60 {
        let penalty = trade_settlement
            .penalty_energy_by_trade(trade_id)
            .call()
            .await
            .expect("Failed to read penaltyEnergyByTrade");

        if penalty > U256::zero() {
            info!(
                "Penalty recorded for trade {} with amount {}",
                trade.trade_uuid, penalty
            );
            return;
        }

        info!(
            "Penalty not submitted yet (attempt {}/60). Retrying...",
            attempt + 1
        );
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: execution engine did not submit penalties for trade {}",
        trade.trade_uuid
    );
}

use crate::world::{CommunityMarketOrderPair, MyWorld, PayAsClearScenario};
use cucumber::{then, when};
use ethers::prelude::*;
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use primitives::db_api_schema::orders::{
    energy_type_to_contract, DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderStatus,
};
use primitives::db_api_schema::profiles::MeasurementSchema;
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::ewds::dto::{EwdsOrderDto, EwdsTradeDto};
use primitives::matching::matching_block_interval;
use primitives::utils::{
    parse_or_hash_bytes16, parse_uuid_or_hex_bytes16, NODE_FLOAT_SCALING_FACTOR,
};
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::info;

const FLOAT_EPSILON: f64 = 0.000_001;
const ENERGY_TYPE_UNSPECIFIED: u8 = 0;
const COMMUNITY_TRADE_POLL_ATTEMPTS: usize = 180;
const COMMUNITY_MATCHING_RETRIGGER_INTERVAL: usize = 30;
const HTTP_PENALTY_POLL_ATTEMPTS: usize = 60;
const EWDS_PENALTY_POLL_ATTEMPTS: usize = 180;

type EvmOrderParamsTuple = (
    [u8; 16],
    [u8; 16],
    [u8; 16],
    u64,
    u64,
    u64,
    u64,
    u8,
    u8,
    bool,
);

abigen!(
    OrderRegistryContract,
    r#"[
        {
            "type": "function",
            "name": "getStatus",
            "stateMutability": "view",
            "inputs": [
                {"name": "orderId", "type": "bytes16"}
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
                        {"name": "orderId", "type": "bytes16"},
                        {"name": "createdBy", "type": "bytes16"},
                        {"name": "marketId", "type": "bytes16"},
                        {"name": "timeSlot", "type": "uint64"},
                        {"name": "creationTime", "type": "uint64"},
                        {"name": "energy", "type": "uint64"},
                        {"name": "energyRate", "type": "uint64"},
                        {"name": "energySourcePreference", "type": "uint8"},
                        {"name": "energyType", "type": "uint8"},
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
        function penaltyEnergyByTrade(bytes16 tradeId) external view returns (uint256)
    ]"#
);

async fn mine_empty_blocks(world: &MyWorld, count: usize) {
    for _ in 0..count {
        world
            .provider
            .request::<_, U256>("evm_mine", None::<()>)
            .await
            .expect("Failed to mine an empty Anvil block");
    }
}

async fn mine_until_matching_block(world: &MyWorld, max_blocks: usize) {
    let matching_block_interval = matching_block_interval();
    for _ in 0..max_blocks {
        mine_empty_blocks(world, 1).await;
        let latest_block = world
            .provider
            .get_block_number()
            .await
            .expect("Failed to read latest block after mining");
        if latest_block.as_u64() % matching_block_interval == 0 {
            info!(
                "Reached matching trigger block {} (mod {} == 0)",
                latest_block, matching_block_interval
            );
            return;
        }
    }

    panic!(
        "Could not reach a matching trigger block after mining {} blocks",
        max_blocks
    );
}

async fn align_to_matching_window(world: &MyWorld, required_blocks: u64) {
    let matching_block_interval = matching_block_interval();
    assert!(
        required_blocks < matching_block_interval,
        "A {}-block order submission cannot fit in a {}-block matching interval",
        required_blocks,
        matching_block_interval
    );

    let latest_block = world
        .provider
        .get_block_number()
        .await
        .expect("Failed to read latest block before submitting the order book")
        .as_u64();
    let blocks_until_trigger = matching_block_interval - (latest_block % matching_block_interval);

    if blocks_until_trigger <= required_blocks {
        info!(
            "Only {} blocks remain in the current matching interval; advancing to the next boundary",
            blocks_until_trigger
        );
        mine_until_matching_block(world, matching_block_interval as usize + 1).await;
    }
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= FLOAT_EPSILON
}

fn address_to_full_hex(address: Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

fn actor_id_as_hex(world: &MyWorld, user_name: &str) -> String {
    format!("0x{}", hex::encode(world.actor_id_for_user(user_name)))
}

fn market_id_as_hex(world: &MyWorld) -> String {
    market_id_bytes_as_hex(world.last_market_id.expect("Missing market id"))
}

fn market_id_bytes_as_hex(market_id: [u8; 16]) -> String {
    format!("0x{}", hex::encode(market_id))
}

fn market_window(world: &MyWorld) -> (u64, u64) {
    (
        world.target_delivery_time as u64,
        (world.target_delivery_time + 900) as u64,
    )
}

async fn query_market_orders(world: &MyWorld) -> Vec<DbOrderSchema> {
    query_orders_for_market(world, market_id_as_hex(world).as_str()).await
}

async fn query_orders_for_market(world: &MyWorld, market_id: &str) -> Vec<DbOrderSchema> {
    let (start_time, end_time) = market_window(world);

    let response = world
        .http_client
        .get(format!(
            "{}/orders?market_id={}&start_time={}&end_time={}",
            world.offchain_storage_url, market_id, start_time, end_time
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
        .json::<Vec<EwdsOrderDto>>()
        .await
        .expect("Failed to parse orders response")
        .into_iter()
        .map(|dto| DbOrderSchema::try_from(dto).expect("valid order DTO"))
        .collect()
}

async fn query_market_trades(world: &MyWorld) -> Vec<DbTradeSchema> {
    let (start_time, end_time) = market_window(world);

    let response = world
        .http_client
        .get(format!(
            "{}/trades?start_time={}&end_time={}",
            world.offchain_storage_url, start_time, end_time
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
        .json::<Vec<EwdsTradeDto>>()
        .await
        .expect("Failed to parse trades response")
        .into_iter()
        .map(|dto| DbTradeSchema::try_from(dto).expect("valid trade DTO"))
        .collect()
}

async fn wait_for_order_in_offchain_storage(world: &MyWorld, order_id: &str) -> DbOrderSchema {
    wait_for_order_in_market(world, market_id_as_hex(world).as_str(), order_id).await
}

async fn wait_for_order_in_market(
    world: &MyWorld,
    market_id: &str,
    order_id: &str,
) -> DbOrderSchema {
    for _ in 0..40 {
        let orders = query_orders_for_market(world, market_id).await;
        if let Some(order) = orders
            .into_iter()
            .find(|order| order.order_id.eq_ignore_ascii_case(order_id))
        {
            return order;
        }

        sleep(Duration::from_millis(500)).await;
    }

    panic!(
        "Timeout: order {} was not indexed in off-chain storage",
        order_id
    );
}

async fn upsert_order_in_offchain_storage(world: &MyWorld, order: DbOrderSchema) {
    let dto = EwdsOrderDto::try_from(order).expect("valid order DTO");
    let response = world
        .http_client
        .post(format!("{}/orders", world.offchain_storage_url))
        .json(&vec![dto])
        .send()
        .await
        .expect("Failed to upsert order in off-chain storage");

    assert!(
        response.status().is_success(),
        "Order upsert failed with status {}",
        response.status()
    );
}

fn order_energy_source_preference(requirements: &Option<DbRequirements>) -> u8 {
    requirements
        .as_ref()
        .and_then(|requirements| requirements.energy_type.as_ref())
        .map(energy_type_to_contract)
        .unwrap_or(energy_type_to_contract(&EnergyType::None))
}

fn order_energy_type(attributes: &Option<DbAttributes>) -> u8 {
    attributes
        .as_ref()
        .map(|attributes| energy_type_to_contract(&attributes.energy_type))
        .unwrap_or(energy_type_to_contract(&EnergyType::None))
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
    place_custom_order_for_market(
        world,
        world.last_market_id.expect("Missing market id"),
        user_name,
        is_bid,
        energy,
        energy_rate,
        requirements,
        attributes,
    )
    .await
}

async fn place_custom_order_for_market(
    world: &MyWorld,
    market_id: [u8; 16],
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

    let actor_id = world.actor_id_for_user(user_name);
    let order_id_bytes = parse_or_hash_bytes16(
        format!(
            "custom:{}:{}:{}:{}:{}:{}",
            user_name,
            is_bid,
            creation_time,
            energy,
            energy_rate,
            hex::encode(market_id)
        )
        .as_str(),
    );

    let params: EvmOrderParamsTuple = (
        order_id_bytes,
        actor_id,
        market_id,
        world.target_delivery_time,
        creation_time,
        (energy * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        (energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        order_energy_source_preference(&requirements),
        order_energy_type(&attributes),
        is_bid,
    );

    let order_id = format!("0x{}", hex::encode(order_id_bytes));

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
        let market_id = market_id_bytes_as_hex(market_id);
        let mut indexed_order =
            wait_for_order_in_market(world, market_id.as_str(), order_id.as_str()).await;
        indexed_order.requirements = requirements;
        indexed_order.attributes = attributes;
        upsert_order_in_offchain_storage(world, indexed_order).await;
    }

    order_id
}

#[when("compatible orders are submitted to different community markets")]
async fn submit_cross_community_orders(world: &mut MyWorld) {
    let market_ids = world
        .community_market_ids
        .expect("Missing community market ids");
    align_to_matching_window(world, 2).await;

    let primary_bid =
        place_custom_order_for_market(world, market_ids[0], "alice", true, 2.0, 20.0, None, None)
            .await;
    let secondary_offer =
        place_custom_order_for_market(world, market_ids[1], "charlie", false, 2.0, 5.0, None, None)
            .await;

    wait_for_order_in_market(
        world,
        market_id_bytes_as_hex(market_ids[0]).as_str(),
        primary_bid.as_str(),
    )
    .await;
    wait_for_order_in_market(
        world,
        market_id_bytes_as_hex(market_ids[1]).as_str(),
        secondary_offer.as_str(),
    )
    .await;

    world.cross_community_order_ids = Some((primary_bid, secondary_offer));
    mine_until_matching_block(world, matching_block_interval() as usize + 1).await;
}

#[then("no cross-community trade is settled")]
async fn verify_no_cross_community_trade(world: &mut MyWorld) {
    let (bid_id, offer_id) = world
        .cross_community_order_ids
        .as_ref()
        .expect("Missing cross-community order ids");

    // Allow the matching engine to observe and process the trigger block.
    sleep(Duration::from_secs(3)).await;

    let cross_trade = query_market_trades(world).await.into_iter().find(|trade| {
        trade.bid_hash.eq_ignore_ascii_case(bid_id)
            && trade.offer_hash.eq_ignore_ascii_case(offer_id)
    });
    assert!(
        cross_trade.is_none(),
        "Matching engine settled a bid and offer from different community markets"
    );

    let order_registry =
        OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    for order_id in [bid_id, offer_id] {
        let status = order_registry
            .get_status(parse_or_hash_bytes16(order_id))
            .call()
            .await
            .expect("Failed to read cross-community order status");
        assert_eq!(
            status, 1u8,
            "Cross-community order {} did not remain Open",
            order_id
        );
    }
}

#[when("matching counterpart orders are submitted within both community markets")]
async fn submit_community_market_counterparts(world: &mut MyWorld) {
    let market_ids = world
        .community_market_ids
        .expect("Missing community market ids");
    let (primary_bid, secondary_offer) = world
        .cross_community_order_ids
        .clone()
        .expect("Missing cross-community order ids");
    align_to_matching_window(world, 2).await;

    let primary_offer = place_custom_order_for_market(
        world,
        market_ids[0],
        "charlie",
        false,
        2.0,
        10.0,
        None,
        None,
    )
    .await;
    let secondary_bid =
        place_custom_order_for_market(world, market_ids[1], "bob", true, 2.0, 15.0, None, None)
            .await;

    wait_for_order_in_market(
        world,
        market_id_bytes_as_hex(market_ids[0]).as_str(),
        primary_offer.as_str(),
    )
    .await;
    wait_for_order_in_market(
        world,
        market_id_bytes_as_hex(market_ids[1]).as_str(),
        secondary_bid.as_str(),
    )
    .await;

    world.community_market_order_pairs = vec![
        CommunityMarketOrderPair {
            market_id: market_ids[0],
            bid_id: primary_bid,
            offer_id: primary_offer,
        },
        CommunityMarketOrderPair {
            market_id: market_ids[1],
            bid_id: secondary_bid,
            offer_id: secondary_offer,
        },
    ];
    mine_until_matching_block(world, matching_block_interval() as usize + 1).await;
}

#[then("each community market settles only its own bid and offer")]
async fn verify_community_market_settlements(world: &mut MyWorld) {
    let order_pairs = world.community_market_order_pairs.clone();
    assert_eq!(order_pairs.len(), 2, "Missing community-market order pairs");

    let scenario_order_ids = order_pairs
        .iter()
        .flat_map(|pair| {
            [
                pair.bid_id.to_ascii_lowercase(),
                pair.offer_id.to_ascii_lowercase(),
            ]
        })
        .collect::<HashSet<_>>();

    for attempt in 0..COMMUNITY_TRADE_POLL_ATTEMPTS {
        if attempt > 0 && attempt % COMMUNITY_MATCHING_RETRIGGER_INTERVAL == 0 {
            info!(
                "Community-market trades are still pending; advancing to another matching boundary"
            );
            mine_until_matching_block(world, matching_block_interval() as usize + 1).await;
        }

        let scenario_trades = query_market_trades(world)
            .await
            .into_iter()
            .filter(|trade| {
                scenario_order_ids.contains(&trade.bid_hash.to_ascii_lowercase())
                    || scenario_order_ids.contains(&trade.offer_hash.to_ascii_lowercase())
            })
            .collect::<Vec<_>>();

        if scenario_trades.len() < order_pairs.len() {
            info!(
                "Community-market trades not available yet (attempt {}/{}). Retrying...",
                attempt + 1,
                COMMUNITY_TRADE_POLL_ATTEMPTS
            );
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        assert_eq!(
            scenario_trades.len(),
            order_pairs.len(),
            "Unexpected number of trades involving the multi-community orders"
        );

        for pair in &order_pairs {
            let expected_market_id = market_id_bytes_as_hex(pair.market_id);
            let trade = scenario_trades
                .iter()
                .find(|trade| {
                    trade.bid_hash.eq_ignore_ascii_case(pair.bid_id.as_str())
                        && trade
                            .offer_hash
                            .eq_ignore_ascii_case(pair.offer_id.as_str())
                })
                .unwrap_or_else(|| {
                    panic!(
                        "No same-market trade found for bid {} and offer {}",
                        pair.bid_id, pair.offer_id
                    )
                });
            assert!(
                trade
                    .market_id
                    .eq_ignore_ascii_case(expected_market_id.as_str()),
                "Trade {} was indexed under the wrong community market",
                trade.trade_uuid
            );
            assert_eq!(
                trade.time_slot, world.target_delivery_time,
                "Trade {} was indexed under the wrong delivery slot",
                trade.trade_uuid
            );
            assert_trade_settled_on_chain(world, trade).await;
        }

        world.last_trade = scenario_trades.first().cloned();
        world.community_market_trades = scenario_trades;
        return;
    }

    panic!("Timeout: community-market trades were not indexed in off-chain storage");
}

#[then("measurements for both community markets are submitted")]
async fn submit_community_market_measurements(world: &mut MyWorld) {
    let measurements = vec![
        MeasurementSchema {
            facility_id: "areaalice".to_string(),
            community_uuid: world.community_id.clone(),
            time_slot: world.target_delivery_time,
            creation_time: 1,
            energy_kwh: 3.0,
        },
        MeasurementSchema {
            facility_id: "areabob".to_string(),
            community_uuid: world.secondary_community_id.clone(),
            time_slot: world.target_delivery_time,
            creation_time: 1,
            energy_kwh: 3.0,
        },
    ];

    AreaMarketInfoAdapter::new(Some(world.offchain_storage_url.clone()))
        .forward_measurement(measurements)
        .await
        .expect("Failed to submit multi-community measurements");
}

#[when(expr = "{string} submits a bid")]
async fn submit_bid(world: &mut MyWorld, user_name: String) {
    publish_orders(
        world.evm_node_url.clone(),
        vec![world.bid_forecast.clone().expect("Missing bid forecast")],
        world.market_schema.clone().expect("Missing market schema"),
        address_to_full_hex(world.order_registry_address),
        world.private_key_for_user(user_name.as_str()),
    )
        .await
        .expect("Failed to publish bid order");
}

#[when(
    expr = "{string} submits a bid for {float} energy with a preferred rate of {float} for partner {string}"
)]
async fn submit_preferred_partner_bid(
    world: &mut MyWorld,
    user_name: String,
    energy: f64,
    preferred_rate: f64,
    partner_name: String,
) {
    let requirements = DbRequirements {
        trading_partner_id: Some(actor_id_as_hex(world, &partner_name)),
        energy_type: None,
        preferred_energy_rate: Some(preferred_rate),
    };

    place_custom_order(
        world,
        user_name.as_str(),
        true,
        energy,
        preferred_rate,
        Some(requirements),
        None,
    )
        .await;
}

#[when(expr = "{string} submits an offer")]
async fn submit_offer(world: &mut MyWorld, user_name: String) {
    publish_orders(
        world.evm_node_url.clone(),
        vec![world
            .offer_forecast
            .clone()
            .expect("Missing offer forecast")],
        world.market_schema.clone().expect("Missing market schema"),
        address_to_full_hex(world.order_registry_address),
        world.private_key_for_user(user_name.as_str()),
    )
        .await
        .expect("Failed to publish offer order");

    // Matching runs on block boundaries. Fast-forward local Anvil after both
    // orders are present in the registry.
    mine_until_matching_block(world, 12).await;
}

#[when(
    expr = "{string} submits an offer for {float} energy at a rate of {float} for the preferred partner {string}"
)]
async fn submit_preferred_partner_offer(
    world: &mut MyWorld,
    user_name: String,
    energy: f64,
    energy_rate: f64,
    partner_name: String,
) {
    let attributes = DbAttributes {
        trading_partner_id: Some(actor_id_as_hex(world, &partner_name)),
        energy_type: EnergyType::Green,
    };

    place_custom_order(
        world,
        user_name.as_str(),
        false,
        energy,
        energy_rate,
        None,
        Some(attributes),
    )
        .await;
}

#[when(
    expr = "{string} submits a cheaper open-market offer for {float} energy at a rate of {float}"
)]
async fn submit_cheaper_offer(world: &mut MyWorld, user_name: String, energy: f64, rate: f64) {
    let order_id =
        place_custom_order(world, user_name.as_str(), false, energy, rate, None, None).await;
    world.last_charlie_offer_order_id = Some(order_id);

    // Trigger matching after all preference/open-market orders were submitted.
    mine_until_matching_block(world, 12).await;
}

#[when("the pay-as-clear order book is submitted")]
async fn submit_pay_as_clear_order_book(world: &mut MyWorld) {
    // A uniform-price auction must observe the complete book in one interval.
    align_to_matching_window(world, 8).await;
    world.pay_as_clear_scenario = Some(place_standard_pay_as_clear_order_book(world).await);

    mine_until_matching_block(world, matching_block_interval() as usize + 1).await;
}

async fn place_standard_pay_as_clear_order_book(world: &MyWorld) -> PayAsClearScenario {
    let first_offer = place_custom_order(world, "charlie", false, 3.0, 8.0, None, None).await;
    let second_offer = place_custom_order(world, "charlie", false, 4.0, 10.0, None, None).await;
    let unmatched_offer = place_custom_order(world, "charlie", false, 1.0, 12.0, None, None).await;
    wait_for_order_in_offchain_storage(world, first_offer.as_str()).await;
    wait_for_order_in_offchain_storage(world, second_offer.as_str()).await;
    wait_for_order_in_offchain_storage(world, unmatched_offer.as_str()).await;

    // The cumulative curves clear 7 energy at 10. The next bid/offer tranche
    // crosses at 9 < 12, so both orders must remain outside the clearing point.
    let first_bid = place_custom_order(world, "alice", true, 3.0, 20.0, None, None).await;
    let second_bid = place_custom_order(world, "bob", true, 4.0, 17.0, None, None).await;
    let unmatched_bid = place_custom_order(world, "alice", true, 1.0, 9.0, None, None).await;
    wait_for_order_in_offchain_storage(world, first_bid.as_str()).await;
    wait_for_order_in_offchain_storage(world, second_bid.as_str()).await;
    wait_for_order_in_offchain_storage(world, unmatched_bid.as_str()).await;

    PayAsClearScenario {
        accepted_order_ids: vec![first_bid, second_bid, first_offer, second_offer],
        unmatched_bid_order_id: unmatched_bid,
        unmatched_offer_order_id: unmatched_offer,
        expected_match_count: 2,
        preferred_order_ids: None,
    }
}

#[when("a preferred bilateral pair and standard pay-as-clear order book are submitted")]
async fn submit_combined_pay_as_clear_order_book(world: &mut MyWorld) {
    // Submit both pricing paths before reaching the same clearing boundary.
    align_to_matching_window(world, 12).await;

    let preferred_bid_requirements = DbRequirements {
        trading_partner_id: Some(actor_id_as_hex(world, "bob")),
        energy_type: None,
        preferred_energy_rate: Some(11.0),
    };
    let preferred_bid = place_custom_order(
        world,
        "alice",
        true,
        2.0,
        20.0,
        Some(preferred_bid_requirements),
        None,
    )
        .await;

    let preferred_offer_attributes = DbAttributes {
        trading_partner_id: Some(actor_id_as_hex(world, "alice")),
        energy_type: EnergyType::Green,
    };
    let preferred_offer = place_custom_order(
        world,
        "bob",
        false,
        2.0,
        10.0,
        None,
        Some(preferred_offer_attributes),
    )
        .await;

    wait_for_order_in_offchain_storage(world, preferred_bid.as_str()).await;
    wait_for_order_in_offchain_storage(world, preferred_offer.as_str()).await;

    let mut scenario = place_standard_pay_as_clear_order_book(world).await;
    scenario.preferred_order_ids = Some((preferred_bid, preferred_offer));
    world.pay_as_clear_scenario = Some(scenario);

    mine_until_matching_block(world, matching_block_interval() as usize + 1).await;
}

#[when(expr = "measurements for facilities are submitted")]
async fn submit_measurements(world: &mut MyWorld) {
    let adapter = AreaMarketInfoAdapter::new(Some(world.offchain_storage_url.clone()));
    let mut measurements = vec![];
    for facility in world.facilities_topology.iter() {
        measurements.push(MeasurementSchema {
            facility_id: facility.facility_id.clone(),
            community_uuid: world.community_id.clone(),
            energy_kwh: 12.0,
            time_slot: world.target_delivery_time,
            creation_time: 1,
        })
    }

    // send measurements to offchain storage
    adapter
        .forward_measurement(measurements)
        .await
        .expect("Failed to submit measurements");
}

async fn assert_trade_settled_on_chain(world: &MyWorld, trade: &DbTradeSchema) {
    let order_registry =
        OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    let bid_id = parse_uuid_or_hex_bytes16(trade.bid_hash.as_str())
        .expect("could not convert hex to bytes");
    let offer_id = parse_uuid_or_hex_bytes16(trade.offer_hash.as_str())
        .expect("could not convert hex to bytes");
    let bid_status = order_registry
        .get_status(bid_id)
        .call()
        .await
        .expect("Failed to read bid status from contract");
    let offer_status = order_registry
        .get_status(offer_id)
        .call()
        .await
        .expect("Failed to read offer status from contract");

    assert_eq!(bid_status, 2u8, "Bid order is not Executed on-chain");
    assert_eq!(offer_status, 2u8, "Offer order is not Executed on-chain");

    for attempt in 0..40 {
        let orders = query_orders_for_market(world, trade.market_id.as_str()).await;
        let bid_executed = orders.iter().any(|order| {
            order.order_id.eq_ignore_ascii_case(trade.bid_hash.as_str())
                && order.status == OrderStatus::Executed
        });
        let offer_executed = orders.iter().any(|order| {
            order
                .order_id
                .eq_ignore_ascii_case(trade.offer_hash.as_str())
                && order.status == OrderStatus::Executed
        });

        if bid_executed && offer_executed {
            return;
        }

        info!(
            "Off-chain order statuses not synchronized yet (attempt {}/40). Retrying...",
            attempt + 1
        );
        sleep(Duration::from_millis(500)).await;
    }

    panic!(
        "Timeout: off-chain order statuses were not updated for trade {}",
        trade.trade_uuid
    );
}

#[then("the matching engine matches the bid and offer and a trade is settled on-chain")]
async fn verify_trade_on_chain(world: &mut MyWorld) {
    if !world.pay_as_clear_trades.is_empty() {
        let trades = world.pay_as_clear_trades.clone();
        for trade in &trades {
            assert_trade_settled_on_chain(world, trade).await;
        }

        info!(
            "Found {} settled pay-as-clear trades on-chain",
            trades.len()
        );
        world.last_trade = trades.first().cloned();
        return;
    }

    let expected_market_id = market_id_as_hex(world).to_lowercase();

    for attempt in 0..60 {
        let trades = query_market_trades(world).await;

        if let Some(trade) = trades
            .into_iter()
            .find(|trade| trade.market_id.to_lowercase() == expected_market_id)
        {
            info!("Found settled trade {}", trade.trade_uuid);
            assert_trade_settled_on_chain(world, &trade).await;
            world.last_trade = Some(trade);
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

#[then(expr = "a trade is settled on-chain between {string} and {string} for {float} energy")]
async fn verify_partner_trade(
    world: &mut MyWorld,
    buyer_name: String,
    seller_name: String,
    energy: f64,
) {
    let order_registry =
        OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    let expected_market_id = market_id_as_hex(world).to_lowercase();
    let expected_buyer = actor_id_as_hex(world, &buyer_name);
    let expected_seller = actor_id_as_hex(world, &seller_name);

    for attempt in 0..60 {
        let trades = query_market_trades(world).await;

        if let Some(trade) = trades.into_iter().find(|trade| {
            trade.market_id.to_lowercase() == expected_market_id
                && trade.buyer.eq_ignore_ascii_case(expected_buyer.as_str())
                && trade.seller.eq_ignore_ascii_case(expected_seller.as_str())
                && approx_eq(trade.parameters.selected_energy_kWh, energy)
        }) {
            world.last_trade = Some(trade.clone());

            let bid_id = parse_uuid_or_hex_bytes16(trade.bid_hash.as_str())
                .expect("could not convert hex to bytes");
            let offer_id = parse_uuid_or_hex_bytes16(trade.offer_hash.as_str())
                .expect("could not convert hex to bytes");

            let bid_status = order_registry
                .get_status(bid_id)
                .call()
                .await
                .expect("Failed to read bid status from contract");
            let offer_status = order_registry
                .get_status(offer_id)
                .call()
                .await
                .expect("Failed to read offer status from contract");

            assert_eq!(bid_status, 2u8, "Bid order is not Executed on-chain");
            assert_eq!(offer_status, 2u8, "Offer order is not Executed on-chain");
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

#[then(expr = "the market clears {float} energy at a uniform price of {float}")]
async fn verify_pay_as_clear_result(
    world: &mut MyWorld,
    expected_energy: f64,
    expected_price: f64,
) {
    let scenario = world
        .pay_as_clear_scenario
        .clone()
        .expect("Missing pay-as-clear scenario state");
    let expected_market_id = market_id_as_hex(world).to_lowercase();
    let expected_order_ids = scenario
        .accepted_order_ids
        .iter()
        .map(|order_id| order_id.to_ascii_lowercase())
        .collect::<HashSet<_>>();

    for attempt in 0..60 {
        let matching_trades = query_market_trades(world)
            .await
            .into_iter()
            .filter(|trade| {
                trade.market_id.to_lowercase() == expected_market_id
                    && expected_order_ids.contains(&trade.bid_hash.to_ascii_lowercase())
                    && expected_order_ids.contains(&trade.offer_hash.to_ascii_lowercase())
            })
            .collect::<Vec<_>>();

        if matching_trades.len() < scenario.expected_match_count {
            info!(
                "Pay-as-clear trades not available yet (attempt {}/60). Retrying...",
                attempt + 1
            );
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        assert_eq!(
            matching_trades.len(),
            scenario.expected_match_count,
            "Expected exactly {} pay-as-clear matches",
            scenario.expected_match_count
        );

        let settled_order_ids = matching_trades
            .iter()
            .flat_map(|trade| {
                [
                    trade.bid_hash.to_ascii_lowercase(),
                    trade.offer_hash.to_ascii_lowercase(),
                ]
            })
            .collect::<HashSet<_>>();
        assert_eq!(
            settled_order_ids, expected_order_ids,
            "Unexpected orders were included in the pay-as-clear result"
        );
        assert!(
            approx_eq(
                matching_trades
                    .iter()
                    .map(|trade| trade.parameters.selected_energy_kWh)
                    .sum(),
                expected_energy,
            ),
            "Pay-as-clear traded energy does not match the clearing volume"
        );
        assert!(
            matching_trades
                .iter()
                .all(|trade| approx_eq(trade.parameters.energy_rate, expected_price)),
            "Pay-as-clear trades do not share the expected uniform clearing price"
        );

        world.last_trade = matching_trades.first().cloned();
        world.pay_as_clear_trades = matching_trades;
        return;
    }

    panic!("Timeout: pay-as-clear trades were not indexed in off-chain storage");
}

#[then(
    expr = "the preferred bilateral trade clears {float} energy at a negotiated price of {float}"
)]
async fn verify_combined_preferred_trade(
    world: &mut MyWorld,
    expected_energy: f64,
    expected_price: f64,
) {
    let scenario = world
        .pay_as_clear_scenario
        .as_ref()
        .expect("Missing pay-as-clear scenario state");
    let (preferred_bid_id, preferred_offer_id) = scenario
        .preferred_order_ids
        .as_ref()
        .expect("Missing preferred order ids for combined pay-as-clear scenario");

    for attempt in 0..60 {
        let preferred_trade = query_market_trades(world).await.into_iter().find(|trade| {
            trade.bid_hash.eq_ignore_ascii_case(preferred_bid_id)
                && trade.offer_hash.eq_ignore_ascii_case(preferred_offer_id)
        });

        if let Some(trade) = preferred_trade {
            assert!(
                approx_eq(trade.parameters.selected_energy_kWh, expected_energy),
                "Preferred trade energy mismatch: expected {}, got {}",
                expected_energy,
                trade.parameters.selected_energy_kWh
            );
            assert!(
                approx_eq(trade.parameters.energy_rate, expected_price),
                "Preferred trade price mismatch: expected {}, got {}",
                expected_price,
                trade.parameters.energy_rate
            );
            assert_trade_settled_on_chain(world, &trade).await;
            world.preferred_trade = Some(trade);
            return;
        }

        info!(
            "Combined preferred trade not available yet (attempt {}/60). Retrying...",
            attempt + 1
        );
        sleep(Duration::from_secs(2)).await;
    }

    panic!("Timeout: combined preferred trade was not indexed in off-chain storage");
}

#[then(expr = "the remaining standard market clears {float} energy at a uniform price of {float}")]
async fn verify_remaining_pay_as_clear_result(
    world: &mut MyWorld,
    expected_energy: f64,
    expected_price: f64,
) {
    verify_pay_as_clear_result(world, expected_energy, expected_price).await;
}

#[then("orders beyond the clearing point remain open")]
async fn verify_pay_as_clear_unmatched_orders(world: &mut MyWorld) {
    let scenario = world
        .pay_as_clear_scenario
        .as_ref()
        .expect("Missing pay-as-clear scenario state");
    let market_orders = query_market_orders(world).await;
    let unmatched_bid = market_orders
        .iter()
        .find(|order| {
            order
                .order_id
                .eq_ignore_ascii_case(scenario.unmatched_bid_order_id.as_str())
        })
        .expect("Unmatched pay-as-clear bid was not found in off-chain storage");
    assert_eq!(
        unmatched_bid.status,
        OrderStatus::Submitted,
        "Bid beyond the clearing point must remain open"
    );

    let unmatched_offer = market_orders
        .iter()
        .find(|order| {
            order
                .order_id
                .eq_ignore_ascii_case(scenario.unmatched_offer_order_id.as_str())
        })
        .expect("Unmatched pay-as-clear offer was not found in off-chain storage");
    assert_eq!(
        unmatched_offer.status,
        OrderStatus::Submitted,
        "Offer beyond the clearing point must remain open"
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

#[then(expr = "Bob's residual offer of {float} energy is available for the next matching phase")]
async fn verify_residual_offer(world: &mut MyWorld, expected_residual_energy: f64) {
    let trade = world
        .last_trade
        .as_ref()
        .expect("No trade was recorded in the previous step");

    let orders = query_market_orders(world).await;
    let offer = orders
        .into_iter()
        .find(|order| order.order_id == trade.offer_hash)
        .unwrap_or_else(|| {
            panic!(
                "No order found with order_id matching offer_hash: {}",
                trade.offer_hash
            )
        });
    let residual_energy = offer.energy_kWh - trade.parameters.selected_energy_kWh;
    assert!(
        approx_eq(residual_energy, expected_residual_energy),
        "Residual offer mismatch: expected {}, got {}",
        expected_residual_energy,
        residual_energy
    );
}

#[then(expr = "Charlie's cheaper offer remains untouched in this phase")]
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
        .expect("Charlie offer order was not found in off-chain storage");
    assert_eq!(
        charlie_offer.status,
        OrderStatus::Submitted,
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
    let mut trades = world.community_market_trades.clone();
    trades.extend(world.pay_as_clear_trades.clone());
    if let Some(preferred_trade) = world.preferred_trade.clone() {
        trades.push(preferred_trade);
    }
    if trades.is_empty() {
        trades.push(
            world
                .last_trade
                .clone()
                .expect("No trade captured in the previous step"),
        );
    }
    let trade_settlement =
        TradeSettlementContract::new(world.trade_settlement_address, world.provider.clone());
    let mut recorded_trade_ids = HashSet::new();
    let poll_attempts = if env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .unwrap_or_else(|_| "http".to_string())
        .eq_ignore_ascii_case("ewds")
    {
        EWDS_PENALTY_POLL_ATTEMPTS
    } else {
        HTTP_PENALTY_POLL_ATTEMPTS
    };

    for attempt in 0..poll_attempts {
        for trade in &trades {
            if recorded_trade_ids.contains(trade.trade_uuid.as_str()) {
                continue;
            }

            let trade_id = parse_uuid_or_hex_bytes16(trade.trade_uuid.as_str())
                .expect("Could not convert trade ID to bytes16");
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
                recorded_trade_ids.insert(trade.trade_uuid.clone());
            }
        }

        if recorded_trade_ids.len() == trades.len() {
            return;
        }

        info!(
            "Penalties not submitted for {} of {} trade(s) yet (attempt {}/{}). Retrying...",
            trades.len() - recorded_trade_ids.len(),
            trades.len(),
            attempt + 1,
            poll_attempts,
        );
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: execution engine submitted penalties for only {} of {} trade(s)",
        recorded_trade_ids.len(),
        trades.len()
    );
}

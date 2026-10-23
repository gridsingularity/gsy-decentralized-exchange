use super::trade_steps::{
    delivery_verification_timestamp, place_custom_order, query_market_trades,
    query_orders_for_market, wait_for_order_in_offchain_storage, OrderRegistryContract,
    PenaltyRecordedFilter, TradeSettledFilter, TradeSettlementContract,
};
use crate::utils::indexed_bytes16_topic;
use crate::world::MyWorld;
use cucumber::then;
use ethers::prelude::*;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use primitives::db_api_schema::{
    orders::OrderStatus,
    trades::{DbTradeSchema, TradeStatus},
};
use primitives::utils::{bytes16_to_hex, parse_uuid_or_hex_bytes16};
use std::{sync::Arc, time::Duration};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

abigen!(
    LifecycleMarket,
    r#"[
        struct NewMarket { bytes16 marketId; bytes16 communityId; uint64 openingTime; uint64 closingTime; uint64 deliveryStartTime; uint64 deliveryEndTime; uint8 marketType; uint8 matchingAlgorithm; }
        function createMarkets(NewMarket[] newMarkets)
    ]"#
);
abigen!(
    CancellableOrderRegistry,
    r#"[
        struct OrderParams { bytes16 orderId; bytes16 createdBy; bytes16 marketId; uint64 timeSlot; uint64 creationTime; uint64 energy; uint64 energyRate; uint8 energySourcePreference; uint8 energyType; bool isBid; bytes16 preferredTradingPartner; uint64 preferredEnergyRate; }
        function getOrder(bytes16 orderId) view returns (OrderParams)
        function cancelOrder(OrderParams params)
    ]"#
);

async fn receipt_timestamp(world: &MyWorld, receipt: TransactionReceipt) -> u64 {
    assert_eq!(
        receipt.status,
        Some(1u64.into()),
        "Lifecycle transaction reverted"
    );
    world
        .provider
        .get_block(receipt.block_hash.unwrap())
        .await
        .unwrap()
        .unwrap()
        .timestamp
        .as_u64()
}

async fn assert_order_status(
    world: &MyWorld,
    id: &str,
    status: OrderStatus,
    onchain_status: u8,
    timestamp: Option<u64>,
) {
    let registry = OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    assert_eq!(
        registry
            .get_status(parse_uuid_or_hex_bytes16(id).unwrap())
            .call()
            .await
            .unwrap(),
        onchain_status
    );
    timeout(Duration::from_secs(120), async {
        loop {
            let order = wait_for_order_in_offchain_storage(world, id).await;
            if order.status == status {
                assert!(
                    order.status_updated_at.is_some(),
                    "Lifecycle timestamp is missing"
                );
                if let Some(timestamp) = timestamp {
                    assert_eq!(order.status_updated_at, Some(timestamp));
                }
                return;
            }
            sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Timeout waiting for order {id} to become {status:?}"));
}

#[then("unmatched orders can be cancelled on-chain and in storage")]
async fn verify_cancellation(world: &mut MyWorld) {
    // A dedicated market keeps the orchestrator's normal slot scheduling untouched.
    let market_id = *Uuid::new_v4().as_bytes();
    world.last_market_id = Some(market_id);
    world.target_delivery_time = get_last_and_next_timeslot().1;
    let signer = Arc::new(SignerMiddleware::new(
        world.provider.clone(),
        world.wallet_for_user("alice"),
    ));
    let market = LifecycleMarket::new(world.market_controller_address, signer.clone());
    let registry = CancellableOrderRegistry::new(world.order_registry_address, signer);
    let now = world
        .provider
        .get_block(BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap()
        .timestamp
        .as_u64();
    let call = market.create_markets(vec![NewMarket {
        market_id,
        community_id: *Uuid::new_v4().as_bytes(),
        opening_time: now,
        closing_time: now + 3600,
        delivery_start_time: world.target_delivery_time,
        delivery_end_time: world.target_delivery_time + 900,
        market_type: 0,
        matching_algorithm: 0,
    }]);
    receipt_timestamp(world, call.send().await.unwrap().await.unwrap().unwrap()).await;
    let cancelled = place_custom_order(world, "alice", true, 1.0, 20.0, None, None).await;
    assert_order_status(world, &cancelled, OrderStatus::Submitted, 1, None).await;
    let params = registry
        .get_order(parse_uuid_or_hex_bytes16(&cancelled).unwrap())
        .call()
        .await
        .unwrap();
    let call = registry.cancel_order(OrderParams {
        order_id: params.0,
        created_by: params.1,
        market_id: params.2,
        time_slot: params.3,
        creation_time: params.4,
        energy: params.5,
        energy_rate: params.6,
        energy_source_preference: params.7,
        energy_type: params.8,
        is_bid: params.9,
        preferred_trading_partner: params.10,
        preferred_energy_rate: params.11,
    });
    let cancelled_at =
        receipt_timestamp(world, call.send().await.unwrap().await.unwrap().unwrap()).await;
    assert_order_status(
        world,
        &cancelled,
        OrderStatus::Cancelled,
        3,
        Some(cancelled_at),
    )
    .await;
}

pub(crate) async fn trade_status_timestamp(
    world: &MyWorld,
    trade: &DbTradeSchema,
    status: &TradeStatus,
) -> u64 {
    let settlement =
        TradeSettlementContract::new(world.trade_settlement_address, world.provider.clone());
    let topic = indexed_bytes16_topic(parse_uuid_or_hex_bytes16(&trade.trade_uuid).unwrap());
    let block_hash = match status {
        TradeStatus::Settled => panic!("Financial settlement is not implemented"),
        TradeStatus::Executed => return delivery_verification_timestamp(world, trade).await,
        TradeStatus::Matched => {
            let events = settlement
                .event::<TradeSettledFilter>()
                .from_block(0u64)
                .topic1(topic)
                .query_with_meta()
                .await
                .unwrap();
            assert_eq!(events.len(), 1, "Trade must settle exactly once");
            events[0].1.block_hash
        }
        TradeStatus::Penalized => {
            let events = settlement
                .event::<PenaltyRecordedFilter>()
                .from_block(0u64)
                .topic3(topic)
                .query_with_meta()
                .await
                .unwrap();
            // Both parties can be penalized; the first penalty sets the terminal status.
            events
                .iter()
                .filter(|(event, _)| event.penalty_energy > 0)
                .min_by_key(|(_, meta)| (meta.block_number, meta.log_index))
                .expect("Penalized trade has no positive penalty event")
                .1
                .block_hash
        }
    };
    world
        .provider
        .get_block(block_hash)
        .await
        .unwrap()
        .unwrap()
        .timestamp
        .as_u64()
}

#[then("the trade is Matched before delivery verification")]
async fn verify_matched_trade(world: &mut MyWorld) {
    let trade = world.last_trade.as_ref().expect("Missing settled trade");
    let stored = query_market_trades(world)
        .await
        .into_iter()
        .find(|stored| stored.trade_uuid == trade.trade_uuid)
        .unwrap();
    assert_eq!(stored.status, TradeStatus::Matched);
    let persisted = crate::utils::test_database()
        .await
        .collection::<DbTradeSchema>("trades")
        .find_one(mongodb::bson::doc! {"trade_uuid": &trade.trade_uuid})
        .await
        .unwrap()
        .unwrap();
    let mut expected = stored;
    expected.status_updated_at =
        Some(trade_status_timestamp(world, trade, &TradeStatus::Matched).await);
    assert_eq!(persisted, expected);
}

pub(crate) async fn assert_trade_verified(
    world: &MyWorld,
    trade: &DbTradeSchema,
    expected_status: TradeStatus,
) {
    assert!(matches!(
        expected_status,
        TradeStatus::Executed | TradeStatus::Penalized
    ));
    let registry = OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    timeout(Duration::from_secs(360), async {
        loop {
            let orders = query_orders_for_market(world, &trade.market_id).await;
            let trades = primitives::offchain_storage::OffchainStorageClient::from_env(
                "EWDS_E2E_CLIENT_ID",
                "gsye2e",
            )
            .fetch_trades(Some(&trade.market_id), None, None)
            .await
            .unwrap();
            let verified_trade = trades.iter().find(|stored| {
                stored.trade_uuid == trade.trade_uuid && stored.status == expected_status
            });
            let executed_orders = [&trade.bid_hash, &trade.offer_hash].iter().all(|id| {
                orders
                    .iter()
                    .any(|order| &order.order_id == *id && order.status == OrderStatus::Executed)
            });
            if let Some(stored) = verified_trade.filter(|_| executed_orders) {
                let timestamp = delivery_verification_timestamp(world, trade).await;
                // The trade DTO exposes its status, but not the internal verdict timestamp.
                let persisted = crate::utils::test_database()
                    .await
                    .collection::<DbTradeSchema>("trades")
                    .find_one(mongodb::bson::doc! {"trade_uuid": &stored.trade_uuid})
                    .await
                    .expect("Failed to read the verified trade")
                    .expect("Verified trade is missing from storage");
                let mut expected = trade.clone();
                expected.status = expected_status.clone();
                expected.status_updated_at =
                    Some(trade_status_timestamp(world, trade, &expected_status).await);
                assert_eq!(
                    persisted, expected,
                    "Verification changed trade details or verdict timestamp"
                );
                for id in [&trade.bid_hash, &trade.offer_hash] {
                    assert_eq!(
                        registry
                            .get_status(parse_uuid_or_hex_bytes16(id).unwrap())
                            .call()
                            .await
                            .unwrap(),
                        2
                    );
                    assert_eq!(
                        orders
                            .iter()
                            .find(|order| &order.order_id == id)
                            .unwrap()
                            .status_updated_at,
                        Some(timestamp)
                    );
                }
                let settlement = TradeSettlementContract::new(
                    world.trade_settlement_address,
                    world.provider.clone(),
                );
                let penalty = settlement
                    .penalty_energy_by_trade(parse_uuid_or_hex_bytes16(&trade.trade_uuid).unwrap())
                    .call()
                    .await
                    .unwrap();
                assert_eq!(penalty.is_zero(), expected_status == TradeStatus::Executed);
                return;
            }
            sleep(Duration::from_secs(2)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "Timeout: trade {} and its consumed orders were not verified",
            trade.trade_uuid
        )
    });
}

#[then("delivery verification marks the trade and orders Executed without penalties")]
async fn verify_full_delivery(world: &mut MyWorld) {
    let trade = world.last_trade.as_ref().expect("Missing settled trade");
    assert_trade_verified(world, trade, TradeStatus::Executed).await;
    assert_eq!(
        query_market_trades(world).await.len(),
        1,
        "Status notifications must not submit new orders"
    );
    let orders =
        query_orders_for_market(world, &bytes16_to_hex(world.last_market_id.unwrap())).await;
    assert_eq!(orders.len(), 2, "Expected exactly the two submitted orders");
}

#[then(expr = "delivery verification marks the trade {word} and both orders Executed")]
async fn verify_trade_verdict(world: &mut MyWorld, status: String) {
    let status = match status.as_str() {
        "Executed" => TradeStatus::Executed,
        "Penalized" => TradeStatus::Penalized,
        _ => panic!("Unexpected delivery verdict: {status}"),
    };
    assert_trade_verified(world, world.last_trade.as_ref().unwrap(), status).await;
    assert_eq!(query_market_trades(world).await.len(), 1);
    assert_eq!(
        query_orders_for_market(world, &bytes16_to_hex(world.last_market_id.unwrap()))
            .await
            .len(),
        2,
        "Status notifications must not submit new orders"
    );
}

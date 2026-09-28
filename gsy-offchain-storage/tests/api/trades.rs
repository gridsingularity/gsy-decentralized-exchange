use crate::helpers::init_app;
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order as DbOrder, OrderStatus,
};
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid as InsertBid, Offer as InsertOffer, OrderComponent as InsertOrderComponent,
};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::{
    Trade, TradeParameters as InsertTradeParameters,
};
use gsy_offchain_primitives::utils::h256_to_string;
use gsy_offchain_storage::db::{Coll, TradeService};
use mongodb::bson::Bson;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use subxt::utils::{AccountId32, H256};

fn create_test_trade_schema(trade_uuid: &str) -> TradeSchema {
    let order_component = DbOrderComponent {
        area_uuid: "area".to_string(),
        market_id: "market".to_string(),
        time_slot: 100,
        creation_time: 1677453190,
        energy: 100.0,
        energy_rate: 10.0,
    };
    TradeSchema {
        _id: h256_to_string(H256::random()),
        status: TradeStatus::Settled,
        seller: "seller_account".to_string(),
        buyer: "buyer_account".to_string(),
        market_id: "market".to_string(),
        time_slot: 100,
        trade_uuid: trade_uuid.to_string(),
        creation_time: 1677453190,
        status_updated_at: None,
        offer: DbOffer {
            seller: "seller_account".to_string(),
            nonce: 1,
            offer_component: order_component.clone(),
        },
        offer_hash: h256_to_string(H256::random()),
        bid: DbBid {
            buyer: "buyer_account".to_string(),
            nonce: 1,
            bid_component: order_component,
        },
        bid_hash: h256_to_string(H256::random()),
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy: 14.0,
            energy_rate: 3.0,
            trade_uuid: trade_uuid.to_string(),
        },
    }
}

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
    let app = init_app().await;
    let address = app.address;
    let account: AccountId32 = crate::orders::create_test_accountid();
    let market_id = H256::random();
    let area_id = H256::random();
    let area_id_2 = H256::random();

    let bid = InsertBid {
        buyer: account.clone(),
        nonce: 1,
        bid_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: area_id.clone(),
            market_id: market_id.clone(),
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let offer = InsertOffer {
        seller: account.clone(),
        nonce: 1,
        offer_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: area_id_2.clone(),
            market_id: market_id.clone(),
            time_slot: 1,
            creation_time: 1677453190,
        },
    };

    let trade_uuid = H256::random();
    let trade1 = Trade {
        seller: account.clone(),
        buyer: account.clone(),
        market_id: market_id.clone(),
        time_slot: 123456123,
        trade_uuid,
        creation_time: 123456123,
        offer,
        offer_hash: H256::random(),
        bid,
        bid_hash: H256::random(),
        residual_offer: None,
        residual_bid: None,
        parameters: InsertTradeParameters {
            selected_energy: 14,
            energy_rate: 3,
            trade_uuid,
        },
    };

    let tradelist = vec![trade1.clone()];
    let body = Vec::<Trade<AccountId32, H256>>::encode(&tradelist);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    let status = resp.unwrap().status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db.get_ref().trades().get_all_trades().await.unwrap();

    let result_trade = saved.first().unwrap();
    assert_eq!(result_trade.trade_uuid, h256_to_string(trade1.trade_uuid));
}

async fn status_of(
    db: &web::Data<gsy_offchain_storage::db::DatabaseWrapper>,
    id: &str,
) -> OrderStatus {
    db.get_ref()
        .orders()
        .get_order_by_id(&Bson::String(id.to_string()))
        .await
        .expect("Failed to fetch order")
        .expect("Order not found")
        .status
}

#[tokio::test]
async fn post_trades_marks_matching_orders_executed() {
    let app = init_app().await;
    let address = app.address;
    let account: AccountId32 = crate::orders::create_test_accountid();
    let market_id = H256::random();
    let bid_area = H256::random();
    let offer_area = H256::random();
    let offer_hash = H256::from_low_u64_be(1);
    let bid_hash = H256::from_low_u64_be(2);

    // Seed an Open bid and an Open offer whose `_id`s equal the fixed hashes
    // that the trade below will carry as `offer_hash`/`bid_hash`.
    let db = web::Data::new(app.db_wrapper);
    let seed_bid = DbOrderSchema {
        _id: h256_to_string(bid_hash),
        status: OrderStatus::Open,
        order: DbOrder::Bid(DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: DbOrderComponent {
                area_uuid: h256_to_string(bid_area),
                market_id: h256_to_string(market_id),
                time_slot: 1,
                creation_time: 1677453190,
                energy: 100.0,
                energy_rate: 10.0,
            },
        }),
    };
    let seed_offer = DbOrderSchema {
        _id: h256_to_string(offer_hash),
        status: OrderStatus::Open,
        order: DbOrder::Offer(DbOffer {
            seller: "seller".to_string(),
            nonce: 1,
            offer_component: DbOrderComponent {
                area_uuid: h256_to_string(offer_area),
                market_id: h256_to_string(market_id),
                time_slot: 1,
                creation_time: 1677453190,
                energy: 100.0,
                energy_rate: 10.0,
            },
        }),
    };
    db.get_ref()
        .orders()
        .insert_orders(vec![seed_bid, seed_offer])
        .await
        .expect("Failed to insert seed orders");

    let bid = InsertBid {
        buyer: account.clone(),
        nonce: 1,
        bid_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: bid_area,
            market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let offer = InsertOffer {
        seller: account.clone(),
        nonce: 1,
        offer_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: offer_area,
            market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let trade_uuid = H256::random();
    let trade = Trade {
        seller: account.clone(),
        buyer: account.clone(),
        market_id,
        time_slot: 123456123,
        trade_uuid,
        creation_time: 123456123,
        offer,
        offer_hash,
        bid,
        bid_hash,
        residual_offer: None,
        residual_bid: None,
        parameters: InsertTradeParameters {
            selected_energy: 14,
            energy_rate: 3,
            trade_uuid,
        },
    };
    let body = Vec::<Trade<AccountId32, H256>>::encode(&vec![trade]);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    assert_eq!(
        status_of(&db, &h256_to_string(bid_hash)).await,
        OrderStatus::Executed
    );
    assert_eq!(
        status_of(&db, &h256_to_string(offer_hash)).await,
        OrderStatus::Executed
    );
}

#[tokio::test]
async fn post_trades_leaves_same_area_market_residual_open() {
    let app = init_app().await;
    let address = app.address;
    let account: AccountId32 = crate::orders::create_test_accountid();
    let market_id = H256::random();
    let bid_area = H256::random();
    let offer_area = H256::random();
    let offer_hash = H256::from_low_u64_be(3);
    let bid_hash = H256::from_low_u64_be(4);

    // Seed the two parent orders (matched by the trade below) plus a residual
    // order sharing the offer's (area_uuid, market_id) but with a different
    // `_id` and reduced energy, as the node creates on a partial match.
    let db = web::Data::new(app.db_wrapper);
    let seed_bid = DbOrderSchema {
        _id: h256_to_string(bid_hash),
        status: OrderStatus::Open,
        order: DbOrder::Bid(DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: DbOrderComponent {
                area_uuid: h256_to_string(bid_area),
                market_id: h256_to_string(market_id),
                time_slot: 1,
                creation_time: 1677453190,
                energy: 100.0,
                energy_rate: 10.0,
            },
        }),
    };
    let seed_offer = DbOrderSchema {
        _id: h256_to_string(offer_hash),
        status: OrderStatus::Open,
        order: DbOrder::Offer(DbOffer {
            seller: "seller".to_string(),
            nonce: 1,
            offer_component: DbOrderComponent {
                area_uuid: h256_to_string(offer_area),
                market_id: h256_to_string(market_id),
                time_slot: 1,
                creation_time: 1677453190,
                energy: 100.0,
                energy_rate: 10.0,
            },
        }),
    };
    let residual_offer = DbOrderSchema {
        _id: "residual_offer".to_string(),
        status: OrderStatus::Open,
        order: DbOrder::Offer(DbOffer {
            seller: "seller".to_string(),
            nonce: 2,
            offer_component: DbOrderComponent {
                area_uuid: h256_to_string(offer_area),
                market_id: h256_to_string(market_id),
                time_slot: 1,
                creation_time: 1677453190,
                energy: 40.0,
                energy_rate: 10.0,
            },
        }),
    };
    db.get_ref()
        .orders()
        .insert_orders(vec![seed_bid, seed_offer, residual_offer])
        .await
        .expect("Failed to insert seed orders");

    let bid = InsertBid {
        buyer: account.clone(),
        nonce: 1,
        bid_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: bid_area,
            market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let offer = InsertOffer {
        seller: account.clone(),
        nonce: 1,
        offer_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: offer_area,
            market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let trade_uuid = H256::random();
    let trade = Trade {
        seller: account.clone(),
        buyer: account.clone(),
        market_id,
        time_slot: 123456123,
        trade_uuid,
        creation_time: 123456123,
        offer,
        offer_hash,
        bid,
        bid_hash,
        residual_offer: None,
        residual_bid: None,
        parameters: InsertTradeParameters {
            selected_energy: 60,
            energy_rate: 3,
            trade_uuid,
        },
    };
    let body = Vec::<Trade<AccountId32, H256>>::encode(&vec![trade]);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    assert_eq!(
        status_of(&db, &h256_to_string(bid_hash)).await,
        OrderStatus::Executed
    );
    assert_eq!(
        status_of(&db, &h256_to_string(offer_hash)).await,
        OrderStatus::Executed
    );
    assert_eq!(status_of(&db, "residual_offer").await, OrderStatus::Open);
}

#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    let app = init_app().await;
    let address = app.address;

    let client = reqwest::Client::new();
    let test_cases = vec![("test", "err"), ("test2", "err")];

    for (invalid_body, error_message) in test_cases {
        let resp = client
            .post(&format!("{}/orders", &address))
            .header("Content-Type", "application/json")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn get_trades_filters_by_status() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);

    let mut trades = Vec::new();
    for status in [
        TradeStatus::Settled,
        TradeStatus::Executed,
        TradeStatus::Penalized,
    ] {
        let mut trade = create_test_trade_schema(&h256_to_string(H256::random()));
        trade.status = status;
        trades.push(trade);
    }
    let expected: Vec<(TradeStatus, String)> = trades
        .iter()
        .map(|t| (t.status.clone(), t.trade_uuid.clone()))
        .collect();

    db.get_ref()
        .trades()
        .insert_trades(trades)
        .await
        .expect("Failed to insert trades");

    let client = reqwest::Client::new();

    for (status, trade_uuid) in expected {
        let response = client
            .get(&format!("{}/trades?status={:?}", &address, status))
            .send()
            .await
            .expect("Failed to fetch trades");
        assert_eq!(200, response.status().as_u16());

        let filtered: Vec<TradeSchema> = response.json().await.expect("Failed to decode trades");
        assert_eq!(
            filtered.len(),
            1,
            "expected exactly one {:?} trade, got {:?}",
            status,
            filtered.iter().map(|t| &t.status).collect::<Vec<_>>()
        );
        assert_eq!(filtered[0].trade_uuid, trade_uuid);
        assert_eq!(filtered[0].status, status);
    }

    let response = client
        .get(&format!("{}/trades", &address))
        .send()
        .await
        .expect("Failed to fetch trades");
    let unfiltered: Vec<TradeSchema> = response.json().await.expect("Failed to decode trades");
    assert_eq!(unfiltered.len(), 3, "an absent status filter returns every trade");
}

#[tokio::test]
async fn update_trade_status_by_uuid_promotes_settled_to_executed() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade_uuid = h256_to_string(H256::random());
    let trade = create_test_trade_schema(&trade_uuid);

    db.get_ref()
        .trades()
        .insert_trades(vec![trade])
        .await
        .expect("Failed to insert trade");

    let summary = db
        .get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
        .expect("Failed to update trade status");
    assert_eq!(summary.matched_count, 1);
    assert_eq!(summary.modified_count, 1);

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let updated = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found");
    assert_eq!(updated.status, TradeStatus::Executed);
}

#[tokio::test]
async fn update_trade_status_by_uuid_matches_nothing_for_unknown_uuid() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade = create_test_trade_schema(&h256_to_string(H256::random()));

    db.get_ref()
        .trades()
        .insert_trades(vec![trade])
        .await
        .expect("Failed to insert trade");

    let summary = db
        .get_ref()
        .trades()
        .update_trade_status_by_uuid(&h256_to_string(H256::random()), TradeStatus::Executed)
        .await
        .expect("Failed to update trade status");
    assert_eq!(summary.matched_count, 0);
    assert_eq!(summary.modified_count, 0);

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    assert!(saved.iter().all(|t| t.status == TradeStatus::Settled));
}

#[tokio::test]
async fn update_trade_status_by_uuid_reclassifies_executed_as_penalized() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade_uuid = h256_to_string(H256::random());
    let trade = create_test_trade_schema(&trade_uuid);

    db.get_ref()
        .trades()
        .insert_trades(vec![trade])
        .await
        .expect("Failed to insert trade");

    db.get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
        .expect("Failed to update trade status");

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let first_status_updated_at = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found")
        .status_updated_at
        .expect("status_updated_at should be set after promotion to Executed");

    let summary = db
        .get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Penalized)
        .await
        .expect("Failed to update trade status");
    assert_eq!(summary.matched_count, 1);
    assert_eq!(summary.modified_count, 1);

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let updated = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found");
    assert_eq!(updated.status, TradeStatus::Penalized);
    let second_status_updated_at = updated
        .status_updated_at
        .expect("status_updated_at should be set after promotion to Penalized");
    assert!(
        second_status_updated_at >= first_status_updated_at,
        "a later real transition must not move status_updated_at backwards"
    );
}

#[tokio::test]
async fn update_trade_status_by_uuid_is_idempotent() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade_uuid = h256_to_string(H256::random());
    let trade = create_test_trade_schema(&trade_uuid);

    db.get_ref()
        .trades()
        .insert_trades(vec![trade])
        .await
        .expect("Failed to insert trade");

    db.get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
        .expect("Failed to update trade status");

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let first_status_updated_at = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found")
        .status_updated_at
        .expect("status_updated_at should be set after the first transition");

    let summary = db
        .get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
        .expect("Failed to update trade status");
    assert_eq!(
        summary.matched_count, 0,
        "a repeat update with the same status matches nothing: the filter excludes trades \
         already in that status, so the repeat is a true no-op rather than a same-value write"
    );
    assert_eq!(summary.modified_count, 0);

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let updated = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found");
    assert_eq!(updated.status, TradeStatus::Executed);
    assert_eq!(
        updated.status_updated_at,
        Some(first_status_updated_at),
        "a repeat update with the same status must not touch status_updated_at"
    );
}

#[tokio::test]
async fn update_trade_status_by_uuid_bounds_status_updated_at_to_the_call() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade_uuid = h256_to_string(H256::random());
    let trade = create_test_trade_schema(&trade_uuid);

    db.get_ref()
        .trades()
        .insert_trades(vec![trade])
        .await
        .expect("Failed to insert trade");

    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    db.get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
        .expect("Failed to update trade status");
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let status_updated_at = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found")
        .status_updated_at
        .expect("status_updated_at should be set on a real transition");
    assert!(
        before <= status_updated_at && status_updated_at <= after,
        "status_updated_at ({status_updated_at}) must fall within [{before}, {after}]"
    );
}

#[tokio::test]
async fn inserted_trade_has_no_status_updated_at() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade_uuid = h256_to_string(H256::random());
    let trade = create_test_trade_schema(&trade_uuid);

    db.get_ref()
        .trades()
        .insert_trades(vec![trade])
        .await
        .expect("Failed to insert trade");

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let inserted = saved
        .iter()
        .find(|t| t.trade_uuid == trade_uuid)
        .expect("trade not found");
    assert_eq!(
        inserted.status_updated_at, None,
        "a trade that has never transitioned out of Settled has no status_updated_at"
    );
}

/// A trade in the node's wire format, as the orderbook worker posts it to `POST /trades`.
fn node_trade(account: &AccountId32) -> Trade<AccountId32, H256> {
    let market_id = H256::random();
    let component = |area_uuid: H256| InsertOrderComponent {
        energy: 100,
        energy_rate: 10,
        area_uuid,
        market_id,
        time_slot: 1,
        creation_time: 1677453190,
    };
    let trade_uuid = H256::random();
    Trade {
        seller: account.clone(),
        buyer: account.clone(),
        market_id,
        time_slot: 123456123,
        trade_uuid,
        creation_time: 123456123,
        offer: InsertOffer {
            seller: account.clone(),
            nonce: 1,
            offer_component: component(H256::random()),
        },
        offer_hash: H256::random(),
        bid: InsertBid {
            buyer: account.clone(),
            nonce: 1,
            bid_component: component(H256::random()),
        },
        bid_hash: H256::random(),
        residual_offer: None,
        residual_bid: None,
        parameters: InsertTradeParameters {
            selected_energy: 14,
            energy_rate: 3,
            trade_uuid,
        },
    }
}

async fn post_node_trades(address: &str, trades: &Vec<Trade<AccountId32, H256>>) -> u16 {
    reqwest::Client::new()
        .post(&format!("{}/trades", address))
        .header("Content-Type", "application/json")
        .json(&Vec::<Trade<AccountId32, H256>>::encode(trades))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// A trade service over an in-memory store seeded as given, so that duplicates stored before
/// `insert_trades` keyed on `trade_uuid` can be set up.
fn seeded_trade_service(trades: Vec<TradeSchema>) -> TradeService {
    TradeService(Coll::InMemory(Arc::new(RwLock::new(trades))))
}

#[tokio::test]
async fn reposting_a_trade_answers_200_and_keeps_one_record_with_its_status() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    let account: AccountId32 = crate::orders::create_test_accountid();
    let trade = node_trade(&account);
    let trade_uuid = h256_to_string(trade.trade_uuid);
    let body = vec![trade];

    assert_eq!(post_node_trades(&address, &body).await, 200);
    assert_eq!(
        post_node_trades(&address, &body).await,
        200,
        "a re-post must succeed, so the worker clears the trade from its queue"
    );
    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    assert_eq!(saved.len(), 1, "a re-post must not store the trade again");
    let stored_id = saved[0]._id.clone();

    db.get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Penalized)
        .await
        .unwrap();
    let judged = db.get_ref().trades().get_all_trades().await.unwrap();

    assert_eq!(post_node_trades(&address, &body).await, 200);
    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    assert_eq!(
        saved, judged,
        "a re-post after the verdict must leave the stored record, status included, untouched"
    );
    assert_eq!(saved[0].status, TradeStatus::Penalized);
    assert_eq!(saved[0]._id, stored_id);
}

#[tokio::test]
async fn insert_trades_stores_one_record_per_trade_uuid() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);
    let trade_uuid = h256_to_string(H256::random());
    let first = create_test_trade_schema(&trade_uuid);
    let other = create_test_trade_schema(&h256_to_string(H256::random()));
    // Same trade, another random `_id`, as a second post carries it.
    let mut again = create_test_trade_schema(&trade_uuid);
    again.status = TradeStatus::Executed;

    let inserted = db
        .get_ref()
        .trades()
        .insert_trades(vec![first.clone(), again.clone(), other.clone()])
        .await
        .unwrap();
    assert_eq!(
        inserted,
        [
            (0, Bson::String(first._id.clone())),
            (2, Bson::String(other._id.clone()))
        ]
        .into_iter()
        .collect(),
        "only newly stored trades are reported"
    );

    let inserted = db
        .get_ref()
        .trades()
        .insert_trades(vec![again])
        .await
        .unwrap();
    assert!(inserted.is_empty());

    let mut saved = db.get_ref().trades().get_all_trades().await.unwrap();
    saved.sort_by(|a, b| a._id.cmp(&b._id));
    let mut expected = vec![first, other];
    expected.sort_by(|a, b| a._id.cmp(&b._id));
    assert_eq!(
        saved, expected,
        "the first stored copy wins, status included"
    );
}

#[tokio::test]
async fn remove_duplicate_trades_keeps_a_judged_copy_else_the_oldest() {
    let with = |trade_uuid: &str,
                id: &str,
                status: TradeStatus,
                creation_time: u64,
                status_updated_at: Option<u64>| {
        let mut trade = create_test_trade_schema(trade_uuid);
        trade._id = id.to_string();
        trade.status = status;
        trade.creation_time = creation_time;
        trade.status_updated_at = status_updated_at;
        trade
    };
    let executed = with("judged", "c", TradeStatus::Executed, 100, Some(500));
    let first_verdict = with("two_verdicts", "z", TradeStatus::Penalized, 100, Some(400));
    let oldest_settled = with("settled", "y", TradeStatus::Settled, 90, None);
    let smallest_id = with("same_age", "a", TradeStatus::Settled, 100, None);
    let single = with("single", "s", TradeStatus::Settled, 100, None);
    let service = seeded_trade_service(vec![
        with("judged", "a", TradeStatus::Settled, 90, None),
        executed.clone(),
        with("judged", "b", TradeStatus::Settled, 100, None),
        with("two_verdicts", "a", TradeStatus::Executed, 100, Some(600)),
        first_verdict.clone(),
        with("settled", "b", TradeStatus::Settled, 100, None),
        oldest_settled.clone(),
        with("same_age", "b", TradeStatus::Settled, 100, None),
        smallest_id.clone(),
        single.clone(),
    ]);

    assert_eq!(service.remove_duplicate_trades().await.unwrap(), 5);
    assert_eq!(
        service.get_all_trades().await.unwrap(),
        vec![executed, first_verdict, oldest_settled, smallest_id, single]
    );
    assert_eq!(
        service.remove_duplicate_trades().await.unwrap(),
        0,
        "a second run finds nothing to remove"
    );
}

#[tokio::test]
async fn update_trade_status_by_uuid_updates_every_copy() {
    let trade_uuid = h256_to_string(H256::random());
    let other = create_test_trade_schema(&h256_to_string(H256::random()));
    let service = seeded_trade_service(vec![
        create_test_trade_schema(&trade_uuid),
        other.clone(),
        create_test_trade_schema(&trade_uuid),
    ]);

    let summary = service
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
        .unwrap();
    assert_eq!(summary.matched_count, 2);
    assert_eq!(summary.modified_count, 2);

    let saved = service.get_all_trades().await.unwrap();
    let copies: Vec<&TradeSchema> = saved
        .iter()
        .filter(|t| t.trade_uuid == trade_uuid)
        .collect();
    assert_eq!(copies.len(), 2);
    assert!(
        copies
            .iter()
            .all(|t| t.status == TradeStatus::Executed && t.status_updated_at.is_some())
    );
    assert!(saved.contains(&other), "other trades are left alone");
}

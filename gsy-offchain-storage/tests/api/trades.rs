use crate::helpers::init_app;
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order as DbOrder, OrderStatus,
};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid as InsertBid, Offer as InsertOffer, OrderComponent as InsertOrderComponent,
};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::{
    Trade, TradeParameters as InsertTradeParameters,
};
use gsy_offchain_primitives::utils::h256_to_string;
use mongodb::bson::Bson;
use subxt::utils::{AccountId32, H256};

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

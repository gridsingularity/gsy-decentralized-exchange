use crate::helpers::{init_app, stop_app};
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::{MarketSchema, MarketType};
use gsy_offchain_primitives::db_api_schema::trades::{
    ClearingResultSchema, ClearingStatus, MarketRoleSchema,
};

fn make_market(market_id: &str, community_id: &str, opening_time: &str) -> MarketSchema {
    MarketSchema {
        market_id: market_id.to_string(),
        community_id: community_id.to_string(),
        opening_time: opening_time.to_string(),
        closing_time: "2026-03-28T09:45:00Z".to_string(),
        delivery_start_time: "2026-03-28T10:00:00Z".to_string(),
        delivery_end_time: "2026-03-28T10:15:00Z".to_string(),
        market_type: MarketType::Spot,
    }
}

#[tokio::test]
async fn get_market_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();

    let market1 = make_market("my_market", "community1", "2026-03-27T18:00:00Z");
    let market2 = make_market("my_market2", "community2", "2026-03-27T19:00:00Z");

    let db = web::Data::new(app.db_wrapper.clone());
    let market_ref = db.get_ref().markets();
    market_ref.insert(market1.clone()).await.unwrap();
    market_ref.insert(market2.clone()).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/market?market_id=my_market", &address))
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());
    let resp_json: Vec<MarketSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json[0].market_id, "my_market");
    stop_app(app).await;
}

#[tokio::test]
async fn get_market_from_community_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();

    let market1 = make_market("my_market", "communityhash", "2026-03-27T18:00:00Z");
    let db = web::Data::new(app.db_wrapper.clone());
    let market_ref = db.get_ref().markets();
    market_ref.insert(market1).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!(
            "{}/community-market?community_id=communityhash",
            &address
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());
    let resp_json: Vec<MarketSchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json[0].market_id, "my_market");
    stop_app(app).await;
}

#[tokio::test]
async fn get_market_returns_404_for_wrong_market_id() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/market?market_id=no_such_market", &address))
        .send()
        .await
        .unwrap();

    assert_eq!(404, resp.status().as_u16());
    stop_app(app).await;
}

#[tokio::test]
async fn post_market_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();
    let market = make_market("new_market", "my_community", "2026-03-27T18:00:00Z");

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/market", &address))
        .json(&market)
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());

    let db = web::Data::new(app.db_wrapper.clone());
    let saved = db
        .get_ref()
        .markets()
        .filter("new_market".to_string())
        .await
        .unwrap();
    let first = saved.first().unwrap();
    assert_eq!(*first, market);
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_get_clearing_result() {
    let app = init_app().await;
    let address = app.address.clone();
    let result = ClearingResultSchema {
        market_id: "DEX-SPOT-0001".to_string(),
        clearing_status: ClearingStatus::Cleared,
        clearing_price: 0.213,
        total_supply: 3.75,
        total_demand: 2.10,
        traded_quantity: 2.10,
        num_trades: 6,
        tx_hash: "0xabc123def456789".to_string(),
        clearing_time: "2026-03-28T09:45:00Z".to_string(),
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/clearing-results", &address))
        .json(&result)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!(
            "{}/clearing-results?market_id=DEX-SPOT-0001",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let saved: Vec<ClearingResultSchema> = resp.json().await.unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].num_trades, 6);
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_get_market_role() {
    let app = init_app().await;
    let address = app.address.clone();

    let role = MarketRoleSchema {
        role_name: "Prosumer".to_string(),
        role_description: "Generates and consumes energy; can submit both bids and offers."
            .to_string(),
        assigned_to: vec!["PARTY-IE-0007".to_string()],
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/market-roles", &address))
        .json(&role)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/market-roles", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let saved: Vec<MarketRoleSchema> = resp.json().await.unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].role_name, "Prosumer");
    stop_app(app).await;
}

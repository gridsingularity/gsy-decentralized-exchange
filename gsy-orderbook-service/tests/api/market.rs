use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use crate::helpers::init_app;

#[tokio::test]
async fn get_market_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let area_uuids_1 = vec!["area1".to_string(), "area2".to_string()];
    let market1 = MarketTopologySchema {
        market_id: "my_market".to_string(),
        area_uuids: area_uuids_1.clone(),
        time_slot: 1232123213,
        creation_time: 1232123213,
    };
    let area_uuids_2 = vec!["area3".to_string(), "area4".to_string()];
    let market2 = MarketTopologySchema {
        market_id: "my_market2".to_string(),
        area_uuids: area_uuids_2.clone(),
        time_slot: 1242123213,
        creation_time: 1242123213,
    };

    let db = web::Data::new(app.db_wrapper);
    let market_ref = db.get_ref().markets();
    let saved = market_ref
        .insert(market1.clone())
        .await
        .unwrap();
    assert_eq!(saved.market_id, "my_market");

    let saved = market_ref
        .insert(market2.clone())
        .await
        .unwrap();
    assert_eq!(saved.market_id, "my_market2");

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/market?market_id=my_market", &address))
        .header("Content-Type", "application/json")
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: MarketTopologySchema = resp.json().await.unwrap();
    assert_eq!(resp_json.market_id, "my_market".to_string());
    assert_eq!(resp_json.area_uuids, area_uuids_1);
    assert_eq!(resp_json.time_slot, market1.time_slot);

    let resp = client
        .get(&format!("{}/market?market_id=my_market2", &address))
        .header("Content-Type", "application/json")
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: MarketTopologySchema = resp.json().await.unwrap();
    assert_eq!(resp_json.market_id, "my_market2".to_string());
    assert_eq!(resp_json.area_uuids, area_uuids_2);
    assert_eq!(resp_json.time_slot, market2.time_slot);
}

#[tokio::test]
async fn get_market_fails_for_wrong_market_id() {
    let app = init_app().await;
    let address = app.address;
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/market?market_id=no_such_market", &address))
        .header("Content-Type", "application/json")
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(404, status.as_u16());
}

#[tokio::test]
async fn post_market_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let area_uuids = vec!["area1".to_string(), "area2".to_string()];
    let market = MarketTopologySchema {
        market_id: "new_market".to_string(),
        area_uuids: area_uuids,
        time_slot: 432321123,
        creation_time: 432321121,
    };


    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/market", &address))
        .header("Content-Type", "application/json")
        .json(&market)
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .markets()
        .filter("new_market".to_string())
        .await
        .unwrap();

    let first_element = saved.iter().nth(0).unwrap();
    assert_eq!(*first_element, market);
}

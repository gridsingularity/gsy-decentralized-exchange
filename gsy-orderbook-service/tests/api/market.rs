use actix_web::web;
use subxt::utils::H256;
use gsy_offchain_primitives::db_api_schema::market::{MarketTopologySchema, AreaTopologySchema};
use gsy_offchain_primitives::utils::h256_to_string;
use crate::helpers::init_app;

#[tokio::test]
async fn get_market_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let area_uuids_1 = vec![
        AreaTopologySchema{
            name: "area1".to_string(),
            area_uuid: "area1hash".to_string(),
            area_hash: h256_to_string(H256::random())
        },
        AreaTopologySchema{
            name: "area2".to_string(),
            area_uuid: "area2hash".to_string(),
            area_hash: h256_to_string(H256::random())
        },
    ];
    let market1 = MarketTopologySchema {
        market_id: "my_market".to_string(),
        area_uuids: area_uuids_1.clone(),
        time_slot: 1232123213,
        creation_time: 1232123213,
        community_name: "my_community1".to_string(),
        community_uuid: "my_community1_hash".to_string(),
    };
    let area_uuids_2 = vec![
        AreaTopologySchema{
            name: "area3".to_string(),
            area_uuid: "area3hash".to_string(),
            area_hash: h256_to_string(H256::random())
        },
        AreaTopologySchema{
            name: "area4".to_string(),
            area_uuid: "area4hash".to_string(),
            area_hash: h256_to_string(H256::random())
        },
    ];
    let market2 = MarketTopologySchema {
        market_id: "my_market2".to_string(),
        area_uuids: area_uuids_2.clone(),
        time_slot: 1242123213,
        creation_time: 1242123213,
        community_name: "my_community2".to_string(),
        community_uuid: "my_community2_hash".to_string(),
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

fn create_market_topology_schema(
    market_id: String, community_uuid: String, community_name: String,
    area1_name: String, area1_uuid: String,
    area2_name: String, area2_uuid: String) -> (MarketTopologySchema, Vec<AreaTopologySchema>) {
    let area_uuids = vec![
        AreaTopologySchema{
            name: area1_name,
            area_uuid: area1_uuid,
            area_hash: h256_to_string(H256::random())
        },
        AreaTopologySchema{
            name: area2_name,
            area_uuid: area2_uuid,
            area_hash: h256_to_string(H256::random())
        },
    ];
    let market = MarketTopologySchema {
        market_id,
        area_uuids: area_uuids.clone(),
        time_slot: 1232123213,
        creation_time: 1232123213,
        community_name,
        community_uuid,
    };
    (market, area_uuids)
}

#[tokio::test]
async fn get_market_from_community_succeeds() {
    let app = init_app().await;
    let address = app.address;

    let (market1, area_uuids_1) = create_market_topology_schema(
        "my_market".to_string(), "communityhash".to_string(), "community1".to_string(),
        "area1".to_string(), "area1hash".to_string(), "area2".to_string(), "area2hash".to_string());

    let db = web::Data::new(app.db_wrapper);
    let market_ref = db.get_ref().markets();
    let saved = market_ref
        .insert(market1.clone())
        .await
        .unwrap();
    assert_eq!(saved.market_id, "my_market");


    let (market2, area_uuids_2) = create_market_topology_schema(
        "my_market2".to_string(), "communityhash2".to_string(), "community2".to_string(),
        "area3".to_string(), "area3hash".to_string(), "area4".to_string(), "area4hash".to_string());

    let saved = market_ref
        .insert(market2.clone())
        .await
        .unwrap();
    assert_eq!(saved.market_id, "my_market2");

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/community-market?community_uuid=communityhash&time_slot=1232123213", &address))
        .header("Content-Type", "application/json")
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: MarketTopologySchema = resp.json().await.unwrap();
    assert_eq!(resp_json.market_id, "my_market".to_string());
    assert_eq!(resp_json.community_name, "community1".to_string());
    assert_eq!(resp_json.area_uuids, area_uuids_1);
    assert_eq!(resp_json.time_slot, market1.time_slot);

    let resp = client
        .get(&format!("{}/community-market?community_uuid=communityhash2&time_slot=1232123213", &address))
        .header("Content-Type", "application/json")
        .send()
        .await.unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: MarketTopologySchema = resp.json().await.unwrap();
    assert_eq!(resp_json.market_id, "my_market2".to_string());
    assert_eq!(resp_json.community_name, "community2".to_string());
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
    let area_uuids = vec![
        AreaTopologySchema{
            name: "area1".to_string(),
            area_uuid: "area1hash".to_string(),
            area_hash: h256_to_string(H256::random())
        },
        AreaTopologySchema{
            name: "area2".to_string(),
            area_uuid: "area2hash".to_string(),
            area_hash: h256_to_string(H256::random())
        },
    ];
    let market = MarketTopologySchema {
        market_id: "new_market".to_string(),
        area_uuids: area_uuids,
        time_slot: 432321123,
        creation_time: 432321121,
        community_name: "my_community".to_string(),
        community_uuid: "my_community_hash".to_string(),
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

use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::{
    AreaTopologySchema, AssetType, CommunitySummary, MarketTopologySchema,
};
use gsy_offchain_primitives::utils::h256_to_string;
use subxt::utils::H256;

#[tokio::test]
async fn get_market_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let community_areas_1 = vec![
        AreaTopologySchema {
            name: "area1".to_string(),
            area_uuid: "area1hash".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
        AreaTopologySchema {
            name: "area2".to_string(),
            area_uuid: "area2hash".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
    ];
    let market1 = MarketTopologySchema {
        market_id: "my_market".to_string(),
        community_areas: community_areas_1.clone(),
        time_slot: 1232123213,
        creation_time: 1232123213,
        community_name: "my_community1".to_string(),
        community_uuid: "my_community1_hash".to_string(),
    };
    let community_areas_2 = vec![
        AreaTopologySchema {
            name: "area3".to_string(),
            area_uuid: "area3hash".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
        AreaTopologySchema {
            name: "area4".to_string(),
            area_uuid: "area4hash".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
    ];
    let market2 = MarketTopologySchema {
        market_id: "my_market2".to_string(),
        community_areas: community_areas_2.clone(),
        time_slot: 1242123213,
        creation_time: 1242123213,
        community_name: "my_community2".to_string(),
        community_uuid: "my_community2_hash".to_string(),
    };

    let db = web::Data::new(app.db_wrapper);
    let market_ref = db.get_ref().markets();
    let saved = market_ref.insert(market1.clone()).await.unwrap();
    assert_eq!(saved.market_id, "my_market");

    let saved = market_ref.insert(market2.clone()).await.unwrap();
    assert_eq!(saved.market_id, "my_market2");

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/market?market_id=my_market", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: MarketTopologySchema = resp.json().await.unwrap();
    assert_eq!(resp_json.market_id, "my_market".to_string());
    assert_eq!(resp_json.community_areas, community_areas_1);
    assert_eq!(resp_json.time_slot, market1.time_slot);

    let resp = client
        .get(&format!("{}/market?market_id=my_market2", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: MarketTopologySchema = resp.json().await.unwrap();
    assert_eq!(resp_json.market_id, "my_market2".to_string());
    assert_eq!(resp_json.community_areas, community_areas_2);
    assert_eq!(resp_json.time_slot, market2.time_slot);
}

fn create_market_topology_schema(
    market_id: String,
    community_uuid: String,
    community_name: String,
    area1_name: String,
    area1_uuid: String,
    area2_name: String,
    area2_uuid: String,
) -> (MarketTopologySchema, Vec<AreaTopologySchema>) {
    let community_areas = vec![
        AreaTopologySchema {
            name: area1_name,
            area_uuid: area1_uuid,
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
        AreaTopologySchema {
            name: area2_name,
            area_uuid: area2_uuid,
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
    ];
    let market = MarketTopologySchema {
        market_id,
        community_areas: community_areas.clone(),
        time_slot: 1232123213,
        creation_time: 1232123213,
        community_name,
        community_uuid,
    };
    (market, community_areas)
}

#[tokio::test]
async fn get_market_from_community_succeeds() {
    let app = init_app().await;
    let address = app.address;

    let (market1, community_areas_1) = create_market_topology_schema(
        "my_market".to_string(),
        "communityhash".to_string(),
        "community1".to_string(),
        "area1".to_string(),
        "area1hash".to_string(),
        "area2".to_string(),
        "area2hash".to_string(),
    );

    let db = web::Data::new(app.db_wrapper);
    let market_ref = db.get_ref().markets();
    let saved = market_ref.insert(market1.clone()).await.unwrap();
    assert_eq!(saved.market_id, "my_market");

    let (market2, community_areas_2) = create_market_topology_schema(
        "my_market2".to_string(),
        "communityhash2".to_string(),
        "community2".to_string(),
        "area3".to_string(),
        "area3hash".to_string(),
        "area4".to_string(),
        "area4hash".to_string(),
    );

    let saved = market_ref.insert(market2.clone()).await.unwrap();
    assert_eq!(saved.market_id, "my_market2");

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!(
            "{}/community-market?community_name=community1&time_slot=1232123213",
            &address
        ))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    // println!("{:?}", resp.text().await.unwrap());
    // assert_eq!(false, true);
    let resp_json: Vec<MarketTopologySchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    let topology = resp_json.get(0).unwrap();
    assert_eq!(topology.market_id, "my_market".to_string());
    assert_eq!(topology.community_name, "community1".to_string());
    assert_eq!(topology.community_uuid, "communityhash".to_string());
    assert_eq!(topology.community_areas, community_areas_1);
    assert_eq!(topology.time_slot, market1.time_slot);

    let resp = client
        .get(&format!(
            "{}/community-market?community_name=community2&time_slot=1232123213",
            &address
        ))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(200, status.as_u16());
    let resp_json: Vec<MarketTopologySchema> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    let topology = resp_json.get(0).unwrap();
    assert_eq!(topology.market_id, "my_market2".to_string());
    assert_eq!(topology.community_name, "community2".to_string());
    assert_eq!(topology.community_uuid, "communityhash2".to_string());
    assert_eq!(topology.community_areas, community_areas_2);
    assert_eq!(topology.time_slot, market2.time_slot);
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
        .await
        .unwrap();

    let status = resp.status();
    assert_eq!(404, status.as_u16());
}

#[tokio::test]
async fn post_market_succeeds() {
    let app = init_app().await;
    let address = app.address;
    let community_areas = vec![
        AreaTopologySchema {
            name: "area1".to_string(),
            area_uuid: "area1hash".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
        AreaTopologySchema {
            name: "area2".to_string(),
            area_uuid: "area2hash".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        },
    ];
    let market = MarketTopologySchema {
        market_id: "new_market".to_string(),
        community_areas,
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
        .await
        .unwrap();

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

fn market_with_slot(
    market_id: &str,
    community_name: &str,
    community_uuid: &str,
    time_slot: u32,
) -> MarketTopologySchema {
    MarketTopologySchema {
        market_id: market_id.to_string(),
        community_areas: vec![AreaTopologySchema {
            name: "area1".to_string(),
            area_uuid: "area1uuid".to_string(),
            area_hash: h256_to_string(H256::random()),
            area_type: AssetType::AREA,
        }],
        time_slot,
        creation_time: time_slot,
        community_name: community_name.to_string(),
        community_uuid: community_uuid.to_string(),
    }
}

#[tokio::test]
async fn get_communities_summarizes_markets_per_community() {
    let app = init_app().await;
    let address = app.address;

    let db = web::Data::new(app.db_wrapper);
    let market_ref = db.get_ref().markets();

    // Community "zeta": two markets across two slots. The uuid is randomized
    // per market; the summary should report the uuid of the LATEST slot.
    market_ref
        .insert(market_with_slot("zeta_1", "zeta", "zeta_uuid_early", 1000))
        .await
        .unwrap();
    market_ref
        .insert(market_with_slot("zeta_2", "zeta", "zeta_uuid_late", 3000))
        .await
        .unwrap();

    // Community "alpha": three markets across three slots.
    market_ref
        .insert(market_with_slot("alpha_1", "alpha", "alpha_uuid_a", 2000))
        .await
        .unwrap();
    market_ref
        .insert(market_with_slot("alpha_2", "alpha", "alpha_uuid_b", 500))
        .await
        .unwrap();
    market_ref
        .insert(market_with_slot("alpha_3", "alpha", "alpha_uuid_latest", 5000))
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/communities", &address))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());
    let communities: Vec<CommunitySummary> = resp.json().await.unwrap();

    // Two distinct communities, sorted by name ascending.
    assert_eq!(communities.len(), 2);
    assert_eq!(communities[0].community_name, "alpha");
    assert_eq!(communities[1].community_name, "zeta");

    let alpha = &communities[0];
    assert_eq!(alpha.market_count, 3);
    assert_eq!(alpha.earliest_slot, 500);
    assert_eq!(alpha.latest_slot, 5000);
    // uuid comes from the market with the latest time_slot (5000).
    assert_eq!(alpha.community_uuid, "alpha_uuid_latest");

    let zeta = &communities[1];
    assert_eq!(zeta.market_count, 2);
    assert_eq!(zeta.earliest_slot, 1000);
    assert_eq!(zeta.latest_slot, 3000);
    assert_eq!(zeta.community_uuid, "zeta_uuid_late");
}

#[tokio::test]
async fn insert_market_rejects_duplicate_market_id() {
    let app = init_app().await;

    let (market, _areas) = create_market_topology_schema(
        "dup_market".to_string(),
        "communityhash".to_string(),
        "community1".to_string(),
        "area1".to_string(),
        "area1hash".to_string(),
        "area2".to_string(),
        "area2hash".to_string(),
    );

    let db = web::Data::new(app.db_wrapper);
    let market_ref = db.get_ref().markets();

    // First insert of a fresh market_id succeeds.
    let saved = market_ref.insert(market.clone()).await.unwrap();
    assert_eq!(saved.market_id, "dup_market");

    // Re-inserting the same market_id is rejected rather than silently
    // creating a duplicate.
    let duplicate = market_ref.insert(market.clone()).await;
    assert!(
        duplicate.is_err(),
        "inserting a duplicate market_id must be rejected"
    );

    // Exactly one copy is stored, so reads stay healthy (no "more than one
    // market" error).
    let stored = market_ref.filter("dup_market".to_string()).await.unwrap();
    assert_eq!(stored.len(), 1);
}

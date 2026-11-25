use crate::helpers::{init_app, stop_app};
use actix_web::web;
use mongodb::bson::{doc, Bson, Document};
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::trades::MarketRoleSchema;
use primitives::ewds::dto::{EwdsClearingResultDto, EwdsMarketDto};
use primitives::utils::rfc3339_to_datetime;

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";
/// Stored and returned market IDs are lowercase UUIDv4s; other spellings are normalized.
const MARKET_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const MARKET_UUID: &str = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
const OTHER_MARKET_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

fn make_market(market_id: &str, community_id: &str, opening_time: &str) -> MarketSchema {
    MarketSchema {
        market_id: market_id.to_string(),
        community_id: community_id.to_string(),
        opening_time: rfc3339_to_datetime(opening_time).unwrap(),
        closing_time: rfc3339_to_datetime("2026-03-28T09:45:00Z").unwrap(),
        delivery_start_time: rfc3339_to_datetime("2026-03-28T10:00:00Z").unwrap(),
        delivery_end_time: rfc3339_to_datetime("2026-03-28T10:15:00Z").unwrap(),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: rfc3339_to_datetime("2026-03-28T09:45:00Z").unwrap(),
    }
}

/// The HTTP request body for `market`.
fn market_dto(market: &MarketSchema) -> EwdsMarketDto {
    EwdsMarketDto::try_from(market.clone()).unwrap()
}

/// A request body with `market_id` as given, which the conversion from `MarketSchema` would
/// normalize or reject.
fn market_request(market_id: &str) -> EwdsMarketDto {
    EwdsMarketDto {
        market_id: market_id.to_string(),
        ..market_dto(&make_market(MARKET_ID, COMMUNITY_ID, "2026-03-27T18:00:00Z"))
    }
}

#[tokio::test]
async fn post_market_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();
    let market = make_market(MARKET_ID, COMMUNITY_ID, "2026-03-27T18:00:00Z");

    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/markets", &address))
        .json(&market_dto(&market))
        .send()
        .await
        .unwrap();

    assert_eq!(200, response.status().as_u16());
    let saved: EwdsMarketDto = response.json().await.unwrap();
    assert_eq!(saved, market_dto(&market));

    // Stored with the canonical market ID and every time as a BSON date.
    let stored = app
        .db_wrapper
        .collection::<Document>("markets")
        .find_one(doc! {"market_id": MARKET_ID})
        .await
        .unwrap()
        .unwrap();
    for field in [
        "opening_time",
        "closing_time",
        "delivery_start_time",
        "delivery_end_time",
        "created_at",
    ] {
        assert!(
            matches!(stored.get(field), Some(Bson::DateTime(_))),
            "{field} is stored as {:?}",
            stored.get(field)
        );
    }
    stop_app(app).await;
}

#[tokio::test]
async fn market_writes_and_queries_share_a_canonical_id() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    for id in [MARKET_UUID, MARKET_ID] {
        let response = client
            .post(format!("{}/markets", app.address))
            .json(&market_request(id))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.json::<EwdsMarketDto>().await.unwrap().market_id,
            MARKET_ID
        );
        for route in ["market", "markets"] {
            let response = client
                .get(format!("{}/{route}", app.address))
                .query(&[("market_id", id)])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200, "{route}: {id}");
            let data: serde_json::Value = response.json().await.unwrap();
            let market = if route == "market" { &data } else { &data[0] };
            assert_eq!(market["marketId"], MARKET_ID);
        }
        assert_eq!(
            app.db_wrapper
                .markets()
                .get_by_ids(vec![id.into()])
                .await
                .unwrap()
                .len(),
            1
        );
    }
    assert_eq!(
        app.db_wrapper
            .markets()
            .filter(None, None, None, None)
            .await
            .unwrap()
            .len(),
        1
    );
    stop_app(app).await;
}

#[tokio::test]
async fn market_routes_reject_invalid_and_nil_ids_before_writing() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    for id in [
        "market-1",
        "00000000-0000-0000-0000-000000000000",
        "0x00000000000000000000000000000000",
        "00000000000000000000000000000000",
        " AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
    ] {
        for route in ["market", "markets"] {
            assert_eq!(
                client
                    .get(format!("{}/{route}", app.address))
                    .query(&[("market_id", id)])
                    .send()
                    .await
                    .unwrap()
                    .status(),
                400
            );
        }
        assert_eq!(
            client
                .post(format!("{}/markets", app.address))
                .json(&market_request(id))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert!(app
        .db_wrapper
        .markets()
        .filter(None, None, None, None)
        .await
        .unwrap()
        .is_empty());
    stop_app(app).await;
}

#[tokio::test]
async fn get_market_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();

    let market1 = make_market(MARKET_ID, COMMUNITY_ID, "2026-03-27T18:00:00Z");
    let market2 = make_market(
        OTHER_MARKET_ID,
        "22222222-2222-4222-8222-222222222222",
        "2026-03-27T19:00:00Z",
    );

    let db = web::Data::new(app.db_wrapper.clone());
    let market_ref = db.get_ref().markets();
    market_ref.insert_one(market1.clone()).await.unwrap();
    market_ref.insert_one(market2.clone()).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{address}/markets?market_id={MARKET_ID}"))
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());
    let resp_json: Vec<EwdsMarketDto> = resp.json().await.unwrap();
    assert_eq!(resp_json, vec![market_dto(&market1)]);
    stop_app(app).await;
}

#[tokio::test]
async fn get_market_from_community_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();

    let market1 = make_market(MARKET_ID, COMMUNITY_ID, "2026-03-27T18:00:00Z");
    let db = web::Data::new(app.db_wrapper.clone());
    let market_ref = db.get_ref().markets();
    market_ref.insert_one(market1).await.unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!(
            "{}/markets?community_id={}",
            &address, COMMUNITY_ID
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());
    let resp_json: Vec<EwdsMarketDto> = resp.json().await.unwrap();
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json[0].market_id, MARKET_ID);
    stop_app(app).await;
}

#[tokio::test]
async fn market_community_ids_preserve_exact_spelling() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let communities = [
        "Community-MiXeD/LIC01",
        "community-mixed/lic01",
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "3F2504E0-4F89-41D3-9A0C-0305E82C3301",
        "0x3f2504e04f8941d39a0c0305e82c3301",
        "00000000-0000-0000-0000-000000000000",
    ];
    for community_id in communities {
        let market_id = uuid::Uuid::new_v4().to_string();
        let market = make_market(&market_id, community_id, "2026-03-27T18:00:00Z");
        for _ in 0..2 {
            let response = client
                .post(format!("{}/markets", address))
                .json(&market_dto(&market))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let saved: EwdsMarketDto = response.json().await.unwrap();
            assert_eq!(saved.community_id, community_id);
            assert_eq!(
                saved.market_id,
                primitives::utils::normalize_uuid(&market_id).unwrap()
            );
        }
    }
    for community_id in communities {
        let resp = client
            .get(format!("{}/markets", address))
            .query(&[("community_id", community_id)])
            .send()
            .await
            .unwrap();

        assert_eq!(200, resp.status().as_u16(), "{community_id}");
        let resp_json: Vec<EwdsMarketDto> = resp.json().await.unwrap();
        assert_eq!(resp_json.len(), 1, "{community_id}");
        assert_eq!(resp_json[0].community_id, community_id);
    }
    stop_app(app).await;
}

#[tokio::test]
async fn post_market_rejects_an_empty_community_id() {
    let app = init_app().await;
    let address = app.address.clone();
    let market = make_market(MARKET_ID, "", "2026-03-27T18:00:00Z");

    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/markets", &address))
        .json(&market_dto(&market))
        .send()
        .await
        .unwrap();

    assert_eq!(400, response.status().as_u16());
    let stored = app
        .db_wrapper
        .markets()
        .filter(Some(MARKET_ID.to_string()), None, None, None)
        .await
        .unwrap();
    assert!(stored.is_empty());
    stop_app(app).await;
}

#[tokio::test]
async fn post_market_rejects_epoch_times() {
    let app = init_app().await;
    let market = EwdsMarketDto {
        opening_time: "00000000001774634400".to_string(),
        ..market_dto(&make_market(
            MARKET_ID,
            COMMUNITY_ID,
            "2026-03-27T18:00:00Z",
        ))
    };

    let response = reqwest::Client::new()
        .post(format!("{}/markets", app.address))
        .json(&market)
        .send()
        .await
        .unwrap();

    assert_eq!(400, response.status().as_u16());
    stop_app(app).await;
}

#[tokio::test]
async fn get_markets_filters_opening_time_by_rfc3339_bounds() {
    let app = init_app().await;
    let markets = app.db_wrapper.markets();
    let early = make_market(MARKET_ID, COMMUNITY_ID, "2026-03-27T18:00:00Z");
    let late = make_market(OTHER_MARKET_ID, COMMUNITY_ID, "2026-03-27T19:00:00Z");
    markets.insert_one(early.clone()).await.unwrap();
    markets.insert_one(late.clone()).await.unwrap();
    let client = reqwest::Client::new();
    let get = |query: &'static str| {
        client
            .get(format!("{}/markets?{query}", app.address))
            .send()
    };

    // `opening_time` in [start_time, end_time], both bounds inclusive; any offset is accepted.
    for (query, expected) in [
        ("start_time=2026-03-27T19:00:00Z", vec![market_dto(&late)]),
        ("end_time=2026-03-27T18:00:00Z", vec![market_dto(&early)]),
        (
            "start_time=2026-03-27T20:00:00%2B02:00&end_time=2026-03-27T19:00:00Z",
            vec![market_dto(&early), market_dto(&late)],
        ),
    ] {
        let resp = get(query).await.unwrap();
        assert_eq!(200, resp.status().as_u16(), "{query}");
        let mut found: Vec<EwdsMarketDto> = resp.json().await.unwrap();
        found.sort_by(|a, b| a.opening_time.cmp(&b.opening_time));
        assert_eq!(found, expected, "{query}");
    }

    // Padded epoch seconds (the old stored form) are no longer accepted.
    let resp = get("start_time=00000000001774634400").await.unwrap();
    assert_eq!(400, resp.status().as_u16());
    stop_app(app).await;
}

#[tokio::test]
async fn get_markets_rejects_a_padded_community_id() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/markets", &address))
        .query(&[("community_id", " padded ")])
        .send()
        .await
        .unwrap();

    assert_eq!(400, resp.status().as_u16());
    stop_app(app).await;
}

#[tokio::test]
async fn get_market_returns_404_for_wrong_market_id() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!(
            "{}/markets?market_id=cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            &address
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(404, resp.status().as_u16());
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_get_clearing_result() {
    let app = init_app().await;
    let address = app.address.clone();
    let mut clearing_result = EwdsClearingResultDto {
        market_id: MARKET_UUID.to_string(),
        clearing_status: "final".to_string(),
        no_bid_reason: None,
        clearing_price: 0.213,
        total_supply: 3.75,
        total_demand: 2.10,
        trade_quantity: 2.10,
        num_trades: 6,
        tx_hash: "0xabc123def456789".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/clearing-results", &address))
        .json(&clearing_result)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    assert_eq!(
        resp.json::<EwdsClearingResultDto>()
            .await
            .unwrap()
            .market_id,
        MARKET_ID
    );

    for id in [MARKET_ID, MARKET_UUID] {
        let resp = client
            .get(&format!("{}/clearing-results", &address))
            .query(&[("market_id", id)])
            .send()
            .await
            .unwrap();
        assert_eq!(200, resp.status().as_u16());
        let saved: Vec<EwdsClearingResultDto> = resp.json().await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].num_trades, 6);
        assert_eq!(saved[0].market_id, MARKET_ID);
    }
    for id in [
        "invalid",
        "00000000-0000-0000-0000-000000000000",
        "0x00000000000000000000000000000000",
    ] {
        clearing_result.market_id = id.into();
        assert_eq!(
            client
                .post(format!("{}/clearing-results", address))
                .json(&clearing_result)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    assert_eq!(
        app.db_wrapper
            .clearing_results()
            .count_documents(mongodb::bson::doc! {})
            .await
            .unwrap(),
        1
    );
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

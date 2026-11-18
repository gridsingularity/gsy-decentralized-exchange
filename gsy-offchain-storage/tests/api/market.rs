use crate::helpers::{init_app, stop_app};
use actix_web::web;
use mongodb::bson::{doc, Bson, Document};
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::trades::MarketRoleSchema;
use primitives::ewds::dto::{EwdsClearingResultDto, EwdsMarketDto};
use primitives::utils::rfc3339_to_datetime;

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";
/// Stored market IDs are 16 hex bytes; the HTTP API returns them as UUIDs.
const MARKET_ID: &str = "0x0000000000000000000000000000000a";
const MARKET_UUID: &str = "00000000-0000-0000-0000-00000000000a";
const OTHER_MARKET_ID: &str = "0x0000000000000000000000000000000b";

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

    // Stored with the hex market ID and every time as a BSON date.
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
async fn get_market_succeeds() {
    let app = init_app().await;
    let address = app.address.clone();

    let market1 = make_market(MARKET_ID, COMMUNITY_ID, "2026-03-27T18:00:00Z");
    let market2 = make_market(OTHER_MARKET_ID, "community2", "2026-03-27T19:00:00Z");

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
    assert_eq!(resp_json[0].market_id, MARKET_UUID);
    stop_app(app).await;
}

#[tokio::test]
async fn get_markets_matches_any_form_of_a_uuid_community_id() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let market = make_market(
        MARKET_ID,
        "3F2504E0-4F89-41D3-9A0C-0305E82C3301",
        "2026-03-27T18:00:00Z",
    );
    let saved: EwdsMarketDto = client
        .post(&format!("{}/markets", &address))
        .json(&market_dto(&market))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved.community_id, "3f2504e0-4f89-41d3-9a0c-0305e82c3301");

    for community_id in [
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "3F2504E0-4F89-41D3-9A0C-0305E82C3301",
        "0x3f2504e04f8941d39a0c0305e82c3301",
    ] {
        let resp = client
            .get(&format!(
                "{}/markets?community_id={}",
                &address, community_id
            ))
            .send()
            .await
            .unwrap();

        assert_eq!(200, resp.status().as_u16(), "{community_id}");
        let resp_json: Vec<EwdsMarketDto> = resp.json().await.unwrap();
        assert_eq!(resp_json.len(), 1, "{community_id}");
        assert_eq!(resp_json[0].market_id, MARKET_UUID);
    }
    stop_app(app).await;
}

#[tokio::test]
async fn post_market_rejects_a_non_uuid_community_id() {
    let app = init_app().await;
    let address = app.address.clone();
    let market = make_market(MARKET_ID, "community1", "2026-03-27T18:00:00Z");

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
async fn get_markets_rejects_a_non_uuid_community_id() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/markets?community_id=community1", &address))
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
        .get(&format!("{}/markets?market_id=no_such_market", &address))
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
    let clearing_result = EwdsClearingResultDto {
        market_id: "0x00000000000000000000000000000001".to_string(),
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

    let resp = client
        .get(&format!(
            "{}/clearing-results?market_id=0x00000000000000000000000000000001",
            &address
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let saved: Vec<EwdsClearingResultDto> = resp.json().await.unwrap();
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

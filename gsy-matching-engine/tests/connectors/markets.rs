use super::*;
use primitives::db_api_schema::market::{MarketSchema, MarketType};
use primitives::ewds::dto::EwdsMarketDto;
use primitives::offchain_storage::OffchainStorageTransport;
use primitives::utils::epoch_to_datetime;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const OPENING: u64 = 1_700_000_100;
const CLOSING: u64 = OPENING + 3600;
const LAST_DISCOVERY: u64 = OPENING + 60;

fn market_id(market: u8) -> String {
    format!("00000000-0000-4000-8000-{market:012x}")
}

fn bid(market: u8) -> Order {
    Order {
        order_id: Uuid::new_v4().to_string(),
        order_type: OrderEnum::Bid,
        status: OrderStatus::Submitted,
        market_id: market_id(market),
        time_slot: CLOSING,
        creation_time: OPENING,
        energy: 1,
        energy_rate: 1,
        created_by: "alice".to_string(),
        requirements: None,
        attributes: None,
    }
}

/// Open orders in `markets`.
fn orders(markets: &[u8]) -> PreparedOrders {
    PreparedOrders {
        open_bids: markets.iter().map(|market| bid(*market)).collect(),
        open_offers: Vec::new(),
        by_order_id: HashMap::new(),
    }
}

fn client(server: &MockServer) -> OffchainStorageClient {
    OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "EWDS_MATCHING_ENGINE_CLIENT_ID",
        "gsymatchingengine",
    )
}

/// `GET /market` returning the record of `market`.
fn market_record(market: u8, matching_algorithm: MatchingAlgorithm) -> Mock {
    let record = EwdsMarketDto::try_from(MarketSchema {
        market_id: market_id(market),
        community_id: "community".to_string(),
        opening_time: epoch_to_datetime(OPENING).unwrap(),
        closing_time: epoch_to_datetime(CLOSING).unwrap(),
        delivery_start_time: epoch_to_datetime(CLOSING).unwrap(),
        delivery_end_time: epoch_to_datetime(CLOSING + 900).unwrap(),
        market_type: MarketType::Spot,
        matching_algorithm,
        created_at: epoch_to_datetime(OPENING).unwrap(),
    })
    .unwrap();
    Mock::given(method("GET"))
        .and(path("/market"))
        .and(query_param("market_id", market_id(market)))
        .respond_with(ResponseTemplate::new(200).set_body_json(record))
}

fn tracked(matching_algorithm: MatchingAlgorithm, checked_until: u64) -> TrackedMarket {
    TrackedMarket {
        matching_algorithm,
        opening_time: OPENING,
        closing_time: CLOSING,
        checked_until,
    }
}

#[tokio::test]
async fn discovery_tracks_the_algorithm_and_window_of_each_market() {
    let server = MockServer::start().await;
    market_record(1, MatchingAlgorithm::PayAsBid)
        .mount(&server)
        .await;
    market_record(2, MatchingAlgorithm::PayAsClear)
        .mount(&server)
        .await;
    // Tracked although it is never cleared, so that it is not fetched again.
    market_record(3, MatchingAlgorithm::AMM)
        .mount(&server)
        .await;
    let mut markets = HashMap::new();

    discover_markets(
        &client(&server),
        &orders(&[1, 2, 3]),
        &mut markets,
        LAST_DISCOVERY,
    )
    .await;

    assert_eq!(
        markets,
        HashMap::from([
            (
                market_id(1),
                tracked(MatchingAlgorithm::PayAsBid, LAST_DISCOVERY)
            ),
            (
                market_id(2),
                tracked(MatchingAlgorithm::PayAsClear, LAST_DISCOVERY)
            ),
            (
                market_id(3),
                tracked(MatchingAlgorithm::AMM, LAST_DISCOVERY)
            ),
        ])
    );
}

#[tokio::test]
async fn a_market_whose_record_cannot_be_read_is_fetched_again_at_the_next_discovery() {
    let server = MockServer::start().await;
    let client = client(&server);
    let mut markets = HashMap::new();

    // A failed request, then no record (the mock server answers 404).
    Mock::given(method("GET"))
        .and(path("/market"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    discover_markets(&client, &orders(&[1]), &mut markets, LAST_DISCOVERY).await;
    assert!(markets.is_empty());
    discover_markets(&client, &orders(&[1]), &mut markets, LAST_DISCOVERY + 5).await;
    assert!(markets.is_empty());

    market_record(1, MatchingAlgorithm::PayAsBid)
        .mount(&server)
        .await;
    discover_markets(&client, &orders(&[1]), &mut markets, LAST_DISCOVERY + 10).await;
    assert_eq!(
        markets,
        HashMap::from([(
            market_id(1),
            tracked(MatchingAlgorithm::PayAsBid, LAST_DISCOVERY + 10)
        )])
    );
}

#[tokio::test]
async fn a_tracked_market_is_not_fetched_again_and_is_dropped_without_open_orders() {
    let server = MockServer::start().await;
    market_record(1, MatchingAlgorithm::PayAsBid)
        .expect(1)
        .mount(&server)
        .await;
    market_record(2, MatchingAlgorithm::PayAsClear)
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server);
    let mut markets = HashMap::new();

    discover_markets(&client, &orders(&[1, 2]), &mut markets, LAST_DISCOVERY).await;
    discover_markets(&client, &orders(&[1]), &mut markets, LAST_DISCOVERY + 5).await;

    // Market 1 keeps the cursor of its first discovery.
    assert_eq!(
        markets,
        HashMap::from([(
            market_id(1),
            tracked(MatchingAlgorithm::PayAsBid, LAST_DISCOVERY)
        )])
    );
}

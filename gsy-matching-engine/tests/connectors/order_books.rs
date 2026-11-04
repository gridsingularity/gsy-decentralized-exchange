use super::*;
use primitives::offchain_storage::OffchainStorageTransport;
use primitives::utils::bytes16_to_uuid_string;

fn source() -> OffchainStorageClient {
    OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        "invalid-url",
        "UNUSED",
        "unused",
    )
}

fn order(id: u8, market: u8, slot: u64, side: OrderEnum) -> EwdsOrderDto {
    EwdsOrderDto::from(DbOrderSchema {
        order_id: bytes16_to_hex([id; 16]),
        created_by: bytes16_to_hex([id; 16]),
        area_uuid: bytes16_to_hex([id; 16]),
        market_id: bytes16_to_hex([market; 16]),
        time_slot: slot,
        creation_time: 90,
        order_type: side,
        status: OrderStatus::Submitted,
        status_updated_at: None,
        energy_kWh: 1.0,
        energy_rate: 10.0,
        requirements: None,
        attributes: None,
    })
}

fn books() -> Vec<EwdsOrderDto> {
    vec![
        order(1, 10, 100, OrderEnum::Bid),
        order(2, 10, 100, OrderEnum::Offer),
        order(3, 10, 200, OrderEnum::Bid),
        order(4, 10, 200, OrderEnum::Offer),
        order(5, 20, 100, OrderEnum::Bid),
        order(6, 20, 100, OrderEnum::Offer),
    ]
}

fn assert_books(prepared: PreparedOrders, expected: Vec<(u8, u64)>) {
    assert_eq!(prepared.by_order_id.len(), expected.len() * 2);
    let books = partition_orders_by_market_slot(prepared.open_bids, prepared.open_offers).unwrap();
    let keys = books
        .iter()
        .map(|book| (book.market_id().to_string(), book.time_slot()))
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        expected
            .into_iter()
            .map(|(market, slot)| (bytes16_to_hex([market; 16]), slot))
            .collect::<Vec<_>>()
    );
    // Excluded books must produce neither matches nor clearing results.
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        let matches =
            match_order_books(books.clone(), &algorithm, PayAsClearPricing::MaxOffer).unwrap();
        assert_eq!(matches.len(), keys.len());
        assert!(matches.iter().all(|book| book.bid_offer_matches.len() == 1));
    }
}

#[tokio::test]
async fn valid_listener_orders_remain_matchable() {
    assert_books(
        prepare_market_orders(books(), &source()).await.unwrap(),
        vec![(10, 100), (10, 200), (20, 100)],
    );
}

#[tokio::test]
async fn known_inactive_orders_are_ignored_before_conversion() {
    for status in [
        "filled",
        "partially_filled",
        "executed",
        "cancelled",
        "expired",
        "rejected",
    ] {
        let mut body = books();
        let mut invalid = order(7, 10, 100, OrderEnum::Bid);
        invalid.order_status = status.into();
        invalid.market_id = "invalid".into();
        invalid.time_slot = "invalid".into();
        invalid.order_type = "invalid".into();
        body.push(invalid);
        assert_books(
            prepare_market_orders(body, &source()).await.unwrap(),
            vec![(10, 100), (10, 200), (20, 100)],
        );
    }
}

#[tokio::test]
async fn invalid_active_order_excludes_earlier_orders_in_its_book() {
    for defect in [
        "type",
        "status",
        "created_by",
        "order_id",
        "creation_time",
        "nil_order_id",
        "nil_created_by",
    ] {
        let mut body = books();
        let mut invalid = order(7, 10, 100, OrderEnum::Bid);
        // A different spelling must still exclude the same book.
        invalid.market_id = bytes16_to_uuid_string([10; 16]).to_uppercase();
        match defect {
            "type" => invalid.order_type = "invalid".into(),
            "status" => invalid.order_status = "unknown".into(),
            "created_by" => invalid.created_by = "invalid".into(),
            "order_id" => invalid.order_id = "invalid".into(),
            "nil_order_id" => invalid.order_id = bytes16_to_hex([0; 16]),
            "nil_created_by" => invalid.created_by = bytes16_to_hex([0; 16]),
            _ => invalid.creation_time = "invalid".into(),
        }
        body.push(invalid);
        assert_books(
            prepare_market_orders(body, &source()).await.unwrap(),
            vec![(10, 200), (20, 100)],
        );
    }
}

#[tokio::test]
async fn unreadable_slot_excludes_the_entire_market() {
    let millisecond_timestamp = order(7, 10, 1_700_000_000_000, OrderEnum::Bid).time_slot;
    for slot in [
        "invalid",
        "1700000000000",
        "+999999-01-01T00:00:00Z",
        &millisecond_timestamp,
    ] {
        let mut body = books();
        let mut invalid = order(7, 10, 100, OrderEnum::Bid);
        invalid.time_slot = slot.into();
        body.push(invalid);
        assert_books(
            prepare_market_orders(body, &source()).await.unwrap(),
            vec![(20, 100)],
        );
    }
}

#[tokio::test]
async fn unidentified_market_aborts_the_cycle() {
    for market in ["invalid", "0x00000000000000000000000000000000"] {
        let mut body = books();
        body[0].market_id = market.into();
        assert!(prepare_market_orders(body, &source()).await.is_err());
    }
}

#[tokio::test]
async fn id_service_failure_aborts_the_cycle() {
    let mut body = books();
    body[4].preferred_trading_partner = Some("facility-a".into());
    assert!(prepare_market_orders(body, &source()).await.is_err());
}

#[tokio::test]
async fn http_failure_is_not_treated_as_an_empty_order_book() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let _ = stream.read(&mut [0; 4096]).unwrap();
        stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]").unwrap();
    });
    assert!(fetch_open_orders_from_orderbook_service(url).await.is_err());
    server.join().unwrap();
}

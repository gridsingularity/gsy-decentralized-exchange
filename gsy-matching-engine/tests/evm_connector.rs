use gsy_matching_engine::connectors::evm_connector::{
    match_order_books, partition_orders_by_market_slot,
};
use gsy_matching_engine::models::Order;
use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};
use primitives::db_api_schema::trades::ClearingStatus;
use primitives::MatchingAlgorithm;

fn id(value: u8) -> String {
    format!("0x{value:032x}")
}

fn order(id_value: u8, order_type: OrderEnum, market: u8, time_slot: u64) -> Order {
    Order {
        order_id: id(id_value),
        order_type,
        status: OrderStatus::Submitted,
        area_uuid: id(id_value),
        market_id: id(market),
        time_slot,
        creation_time: 1,
        energy: 1,
        energy_rate: 1,
        created_by: id(id_value),
        requirements: None,
        attributes: None,
    }
}

#[test]
fn partitions_orders_by_market_and_time_slot() {
    let order_books = partition_orders_by_market_slot(
        vec![
            order(1, OrderEnum::Bid, 1, 100),
            order(2, OrderEnum::Bid, 1, 200),
            order(3, OrderEnum::Bid, 2, 100),
        ],
        vec![
            order(4, OrderEnum::Offer, 1, 100),
            order(5, OrderEnum::Offer, 1, 200),
            order(6, OrderEnum::Offer, 2, 100),
        ],
    )
    .expect("partitioned orders should be valid");

    assert_eq!(order_books.len(), 3);
    for order_book in order_books {
        assert!(order_book
            .bids()
            .iter()
            .all(|order| order.market_id == order_book.market_id()
                && order.time_slot == order_book.time_slot()));
        assert!(order_book
            .offers()
            .iter()
            .all(|order| order.market_id == order_book.market_id()
                && order.time_slot == order_book.time_slot()));
    }
}

#[test]
fn pay_as_clear_calculates_an_independent_price_for_each_order_book() {
    let mut first_bid = order(1, OrderEnum::Bid, 1, 100);
    first_bid.energy_rate = 20;
    let mut first_offer = order(2, OrderEnum::Offer, 1, 100);
    first_offer.energy_rate = 10;
    let mut second_bid = order(3, OrderEnum::Bid, 2, 100);
    second_bid.energy_rate = 40;
    let mut second_offer = order(4, OrderEnum::Offer, 2, 100);
    second_offer.energy_rate = 30;

    let order_books = partition_orders_by_market_slot(
        vec![first_bid, second_bid],
        vec![first_offer, second_offer],
    )
    .expect("partitioned orders should be valid");
    let market_matches = match_order_books(order_books, &MatchingAlgorithm::PayAsClear)
        .expect("partitioned order books should match");

    assert_eq!(market_matches.len(), 2);

    // Each market should produce exactly one match with its own clearing price.
    let mut clearing_prices = market_matches
        .iter()
        .flat_map(|market| market.bid_offer_matches.iter())
        .map(|item| item.energy_rate)
        .collect::<Vec<_>>();
    clearing_prices.sort_unstable();
    assert_eq!(clearing_prices, vec![10, 30]);

    assert!(market_matches.iter().all(|market| {
        market.bid_offer_matches.iter().all(|item| {
            item.bid.market_id == item.offer.market_id && item.bid.time_slot == item.offer.time_slot
        })
    }));

    // Clearing stats should reflect the single match per market.
    let mut computed_prices = market_matches
        .iter()
        .map(|market| market.clearing_result.clearing_price.unwrap())
        .collect::<Vec<_>>();
    computed_prices.sort_unstable();
    assert_eq!(computed_prices, vec![10, 30]);
    assert!(market_matches
        .iter()
        .all(|market| market.clearing_result.clearing_status == ClearingStatus::Final));
}

#[test]
fn total_supply_and_demand_are_summed_over_the_whole_order_book() {
    let mut first_bid = order(1, OrderEnum::Bid, 1, 100);
    first_bid.energy = 5;
    first_bid.energy_rate = 20;
    let mut second_bid = order(2, OrderEnum::Bid, 1, 100);
    second_bid.energy = 7;
    // Priced below every offer, so this bid never matches but still counts as demand.
    second_bid.energy_rate = 1;
    let mut first_offer = order(3, OrderEnum::Offer, 1, 100);
    first_offer.energy = 3;
    first_offer.energy_rate = 10;
    let mut second_offer = order(4, OrderEnum::Offer, 1, 100);
    second_offer.energy = 4;
    second_offer.energy_rate = 10;

    let order_books = partition_orders_by_market_slot(
        vec![first_bid, second_bid],
        vec![first_offer, second_offer],
    )
    .expect("partitioned orders should be valid");
    let market_matches = match_order_books(order_books, &MatchingAlgorithm::PayAsClear)
        .expect("partitioned order books should match");

    assert_eq!(market_matches.len(), 1);
    let clearing_result = &market_matches[0].clearing_result;
    assert_eq!(clearing_result.total_supply, Some(7));
    assert_eq!(clearing_result.total_demand, Some(12));
}

#[test]
fn rejected_clearing_still_reports_total_supply_and_demand() {
    let mut bid = order(1, OrderEnum::Bid, 1, 100);
    bid.energy = 5;
    bid.energy_rate = 1;
    let mut offer = order(2, OrderEnum::Offer, 1, 100);
    offer.energy = 3;
    offer.energy_rate = 10;

    let order_books = partition_orders_by_market_slot(vec![bid], vec![offer])
        .expect("partitioned orders should be valid");
    let market_matches = match_order_books(order_books, &MatchingAlgorithm::PayAsClear)
        .expect("partitioned order books should match");

    assert_eq!(market_matches.len(), 1);
    let clearing_result = &market_matches[0].clearing_result;
    assert_eq!(clearing_result.clearing_status, ClearingStatus::Rejected);
    assert_eq!(clearing_result.total_supply, Some(3));
    assert_eq!(clearing_result.total_demand, Some(5));
    assert_eq!(clearing_result.num_trades, None);
}

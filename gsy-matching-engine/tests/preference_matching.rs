use gsy_matching_engine::algorithms::MatchOrders;
use gsy_matching_engine::models::{BidOfferMatch, MatchType, MatchingData, Order, Requirements};
use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};
use primitives::MatchingAlgorithm;

fn order(
    id: &str,
    side: OrderEnum,
    rate: u64,
    partner: Option<&str>,
    preferred: Option<u64>,
) -> Order {
    Order {
        order_id: id.into(),
        order_type: side,
        status: OrderStatus::Submitted,
        area_uuid: id.into(),
        market_id: "market".into(),
        time_slot: 100,
        creation_time: 90,
        energy: 10,
        energy_rate: rate,
        created_by: id.into(),
        requirements: (partner.is_some() || preferred.is_some()).then(|| Requirements {
            trading_partner_id: partner.map(str::to_owned),
            energy_type: None,
            preferred_energy_rate: preferred,
        }),
        attributes: None,
    }
}

fn run(algorithm: &MatchingAlgorithm, bids: Vec<Order>, offers: Vec<Order>) -> Vec<BidOfferMatch> {
    let mut book = MatchingData::new("market".into(), 100, bids, offers).unwrap();
    algorithm.match_orders(&mut book).unwrap()
}

#[test]
fn every_declared_partner_must_identify_the_counterparty() {
    for (algorithm, normal_price) in [
        (MatchingAlgorithm::PayAsBid, 20),
        (MatchingAlgorithm::PayAsClear, 10),
    ] {
        for (buyer_partner, seller_partner, eligible) in [
            (None, None, false),
            (Some("seller"), None, true),
            (None, Some("buyer"), true),
            (Some("seller"), Some("buyer"), true),
            (Some("unavailable"), None, false),
            (None, Some("unavailable"), false),
            (Some("seller"), Some("another-buyer"), false),
            (Some("another-seller"), Some("buyer"), false),
        ] {
            let bid = order("buyer", OrderEnum::Bid, 20, buyer_partner, Some(15));
            let offer = order("seller", OrderEnum::Offer, 10, seller_partner, Some(15));
            let matches = run(&algorithm, vec![bid], vec![offer]);
            assert_eq!(matches.len(), 1);
            assert_eq!(
                matches[0].energy_rate,
                if eligible { 15 } else { normal_price }
            );
            assert_eq!(
                matches[0].match_type,
                if eligible {
                    MatchType::Preferred
                } else {
                    MatchType::Standard
                }
            );
        }
    }
}

#[test]
fn absent_preferred_rates_use_each_orders_normal_rate() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        for (
            bid_rate,
            offer_rate,
            bid_partner,
            offer_partner,
            bid_preferred,
            offer_preferred,
            price,
        ) in [
            (20, 10, Some("seller"), None, Some(10), None, 10),
            (20, 10, None, Some("buyer"), None, Some(20), 20),
            (15, 15, Some("seller"), Some("buyer"), None, None, 15),
            (15, 15, None, Some("buyer"), Some(0), Some(0), 15),
        ] {
            let bid = order(
                "buyer",
                OrderEnum::Bid,
                bid_rate,
                bid_partner,
                bid_preferred,
            );
            let offer = order(
                "seller",
                OrderEnum::Offer,
                offer_rate,
                offer_partner,
                offer_preferred,
            );
            let matches = run(&algorithm, vec![bid], vec![offer]);
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].energy_rate, price);
            assert_eq!(matches[0].match_type, MatchType::Preferred);
        }
    }
}

#[test]
fn unequal_effective_rates_fall_back_to_normal_prices() {
    for (algorithm, price) in [
        (MatchingAlgorithm::PayAsBid, 20),
        (MatchingAlgorithm::PayAsClear, 10),
    ] {
        for (bid_preferred, offer_preferred) in
            [(Some(15), Some(12)), (Some(9), None), (None, None)]
        {
            let bid = order("buyer", OrderEnum::Bid, 20, Some("seller"), bid_preferred);
            let offer = order(
                "seller",
                OrderEnum::Offer,
                10,
                Some("buyer"),
                offer_preferred,
            );
            let matches = run(&algorithm, vec![bid], vec![offer]);
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].energy_rate, price);
            assert_eq!(matches[0].match_type, MatchType::Standard);
        }
        let bid = order("buyer", OrderEnum::Bid, 8, Some("seller"), Some(15));
        let offer = order("seller", OrderEnum::Offer, 10, Some("buyer"), Some(12));
        assert!(run(&algorithm, vec![bid], vec![offer]).is_empty());
    }
}

#[test]
fn equal_preferred_rates_supersede_normal_price_limits() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        for price in [5, 25] {
            let bid = order("buyer", OrderEnum::Bid, 20, Some("seller"), Some(price));
            let offer = order("seller", OrderEnum::Offer, 10, Some("buyer"), Some(price));
            let matches = run(&algorithm, vec![bid], vec![offer]);
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].energy_rate, price);
            assert_eq!(matches[0].match_type, MatchType::Preferred);
        }
    }
}

#[test]
fn seller_only_preference_takes_priority_over_a_higher_standard_bid() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        let other = order("other-buyer", OrderEnum::Bid, 25, None, None);
        let bid = order("buyer", OrderEnum::Bid, 20, None, None);
        let offer = order("seller", OrderEnum::Offer, 10, Some("buyer"), Some(20));
        let matches = run(&algorithm, vec![other, bid], vec![offer]);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].bid.created_by, "buyer");
        assert_eq!(matches[0].energy_rate, 20);
    }
}

#[test]
fn remaining_preferred_quantity_can_match_in_the_standard_phase() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        let mut bid = order("buyer", OrderEnum::Bid, 20, Some("seller"), Some(15));
        bid.energy = 15;
        let preferred = order("seller", OrderEnum::Offer, 15, None, None);
        let mut standard = order("other-seller", OrderEnum::Offer, 10, None, None);
        standard.energy = 5;
        let matches = run(&algorithm, vec![bid], vec![standard, preferred]);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].offer.created_by, "seller");
        assert_eq!(matches[0].energy_rate, 15);
        let residual = matches[0].residual_bid.as_ref().unwrap();
        assert_eq!(residual.energy, 5);
        assert_eq!(&matches[1].bid, residual);
        assert_eq!(matches[1].offer.created_by, "other-seller");
        assert_eq!(
            matches.iter().map(|item| item.selected_energy).sum::<u64>(),
            15
        );
    }
}

#[test]
fn preferred_offer_residual_is_reused_in_the_standard_phase() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        let bid = order("buyer", OrderEnum::Bid, 20, Some("seller"), Some(15));
        let mut offer = order("seller", OrderEnum::Offer, 15, None, None);
        offer.energy = 15;
        let mut standard = order("other-buyer", OrderEnum::Bid, 20, None, None);
        standard.energy = 5;
        let matches = run(&algorithm, vec![standard, bid], vec![offer]);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].bid.created_by, "buyer");
        let residual = matches[0].residual_offer.as_ref().unwrap();
        assert_eq!(residual.energy, 5);
        assert_ne!(residual.order_id, matches[0].offer.order_id);
        assert_eq!(&matches[1].offer, residual);
        assert_eq!(matches[1].bid.order_id, "other-buyer");
        assert!(matches[1].residual_offer.is_none());
        assert_eq!(
            matches.iter().map(|item| item.selected_energy).sum::<u64>(),
            15
        );
    }
}

#[test]
fn latest_residual_after_multiple_preferred_fills_is_reused() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        for residual_is_bid in [true, false] {
            let mut bid = order("buyer", OrderEnum::Bid, 20, Some("seller"), Some(15));
            let mut offer = order("seller", OrderEnum::Offer, 15, None, None);
            let (bids, offers) = if residual_is_bid {
                bid.energy = 25;
                let mut second_offer = offer.clone();
                second_offer.order_id = "second-offer".into();
                let mut standard = order("other-seller", OrderEnum::Offer, 10, None, None);
                standard.energy = 5;
                (vec![bid], vec![standard, offer, second_offer])
            } else {
                offer.energy = 25;
                let mut second_bid = bid.clone();
                second_bid.order_id = "second-bid".into();
                let mut standard = order("other-buyer", OrderEnum::Bid, 20, None, None);
                standard.energy = 5;
                (vec![standard, bid, second_bid], vec![offer])
            };
            let matches = run(&algorithm, bids, offers);
            assert_eq!(matches.len(), 3);
            assert_eq!(
                matches.iter().map(|item| item.match_type).collect::<Vec<_>>(),
                vec![
                    MatchType::Preferred,
                    MatchType::Preferred,
                    MatchType::Standard,
                ]
            );
            let (first_residual, latest_residual, standard_order) = if residual_is_bid {
                (
                    &matches[0].residual_bid,
                    &matches[1].residual_bid,
                    &matches[2].bid,
                )
            } else {
                (
                    &matches[0].residual_offer,
                    &matches[1].residual_offer,
                    &matches[2].offer,
                )
            };
            let first_residual = first_residual.as_ref().unwrap();
            let latest_residual = latest_residual.as_ref().unwrap();
            assert_eq!(
                if residual_is_bid {
                    &matches[1].bid
                } else {
                    &matches[1].offer
                },
                first_residual,
            );
            assert_eq!(first_residual.energy, 15);
            assert_eq!(latest_residual.energy, 5);
            assert_ne!(first_residual.order_id, latest_residual.order_id);
            assert_eq!(standard_order, latest_residual);
            assert_eq!(
                matches.iter().map(|item| item.selected_energy).sum::<u64>(),
                25
            );
        }
    }
}

#[test]
fn standard_fills_consume_the_preceding_residual() {
    for algorithm in [MatchingAlgorithm::PayAsBid, MatchingAlgorithm::PayAsClear] {
        for residual_is_bid in [true, false] {
            let mut bid = order("buyer", OrderEnum::Bid, 20, None, None);
            let mut offer = order("seller", OrderEnum::Offer, 10, None, None);
            let (bids, offers) = if residual_is_bid {
                bid.energy = 25;
                let mut second = offer.clone();
                second.order_id = "second-offer".into();
                (vec![bid], vec![offer, second])
            } else {
                offer.energy = 25;
                let mut second = bid.clone();
                second.order_id = "second-bid".into();
                (vec![bid, second], vec![offer])
            };
            let matches = run(&algorithm, bids, offers);
            assert_eq!(matches.len(), 2);
            assert!(matches
                .iter()
                .all(|item| item.match_type == MatchType::Standard));
            let (residual, next_order, last_residual) = if residual_is_bid {
                (
                    &matches[0].residual_bid,
                    &matches[1].bid,
                    &matches[1].residual_bid,
                )
            } else {
                (
                    &matches[0].residual_offer,
                    &matches[1].offer,
                    &matches[1].residual_offer,
                )
            };
            assert_eq!(next_order, residual.as_ref().unwrap());
            assert_eq!(next_order.energy, 15);
            assert_eq!(last_residual.as_ref().unwrap().energy, 5);
            assert_eq!(
                matches.iter().map(|item| item.selected_energy).sum::<u64>(),
                20
            );
        }
    }
}

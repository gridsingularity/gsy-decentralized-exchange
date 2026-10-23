//! MS-10 golden test: the optimized matching (`controller::match_orders` and
//! `impl PayAsBid for DbMatchingData`) gives the same matches as the code
//! before MS-10 (`common::reference`), on many seeded random batches.

mod common;

use common::payload::{BatchShape, batch_of_size};
use common::reference::{ReferenceMatchingData, reference_match_orders};
use common::rng::Rng;
use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order,
};
use launchpad_matching_service::api::controller::match_orders;
use launchpad_matching_service::api::types::{
    DbBidOfferMatch, DbMatchingData, MatchingBid, MatchingOffer, OrdersToMatch,
};
use std::collections::HashMap;

/// A match as a comparable string: everything except `match_id` (random),
/// with every `f64` written by serde_json, which is exact for finite values
/// (distinct bit patterns give distinct strings, including `-0.0`).
fn fingerprint(m: &DbBidOfferMatch) -> String {
    let mut m = m.clone();
    m.match_id = String::new();
    serde_json::to_string(&m).expect("Failed to serialize a match")
}

fn fingerprints(matches: &[DbBidOfferMatch]) -> Vec<String> {
    matches.iter().map(fingerprint).collect()
}

/// Parameters of one random batch.
struct Shape {
    markets: u64,
    slots: u64,
    orders: usize,
    areas: u64,
    /// Bid rates are drawn from `bid_rates`, offer rates from `offer_rates`.
    bid_rates: (f64, f64),
    offer_rates: (f64, f64),
}

/// Energies that hit the interesting cases: zero, below / at / just above
/// EPS (1e-6), equal values (ties, exact full fills) and fractions whose
/// differences are not exact in binary.
const ENERGY_POOL: [f64; 14] = [
    0.0, 1e-7, 5e-7, 1e-6, 1.5e-6, 0.1, 0.2, 0.3, 1.0, 1.0, 2.5, 3.0, 5.0, 10.0,
];

fn random_energy(rng: &mut Rng) -> f64 {
    match rng.below(4) {
        0 => *rng.pick(&ENERGY_POOL),
        1 => (rng.below(20) as f64) * 0.25,
        _ => rng.range(0.0, 8.0),
    }
}

fn random_rate(rng: &mut Rng, range: (f64, f64)) -> f64 {
    if rng.chance(0.6) {
        // Discrete steps inside the range: many ties.
        let steps = ((range.1 - range.0) / 0.5).max(0.0) as u64 + 1;
        range.0 + 0.5 * rng.below(steps) as f64
    } else {
        rng.range(range.0, range.1)
    }
}

/// A random batch. Every order has a unique `nonce`, so no two orders share
/// their content (the reference keys the remaining energy by the order's
/// content, see `two_orders_with_identical_content_*` below).
fn random_batch(rng: &mut Rng, shape: &Shape, nonce: &mut u32) -> OrdersToMatch {
    let mut orders = Vec::with_capacity(shape.orders);
    for i in 0..shape.orders {
        *nonce += 1;
        let is_bid = rng.chance(0.5);
        let rates = if is_bid {
            shape.bid_rates
        } else {
            shape.offer_rates
        };
        let component = DbOrderComponent {
            area_uuid: format!("area-{}", rng.below(shape.areas)),
            market_id: format!("market-{}", rng.below(shape.markets)),
            time_slot: 1_700_000_000 + 900 * rng.below(shape.slots),
            creation_time: 1_699_999_000 + rng.below(3),
            energy: random_energy(rng),
            energy_rate: random_rate(rng, rates),
        };
        let party = format!("party-{}", rng.below(5));
        let order = if is_bid {
            Order::Bid(DbBid {
                buyer: party,
                nonce: *nonce,
                bid_component: component,
            })
        } else {
            Order::Offer(DbOffer {
                seller: party,
                nonce: *nonce,
                offer_component: component,
            })
        };
        orders.push(DbOrderSchema {
            _id: format!("order-{}", i),
            status: Default::default(),
            order,
        });
    }
    OrdersToMatch {
        user_id: "golden-user".to_string(),
        orders,
    }
}

/// Compares old and new on one batch and returns the number of matches.
fn assert_same_matches(batch: &OrdersToMatch, context: &str) -> usize {
    let (reference_map, reference_all) = reference_match_orders(batch.clone());
    let new = match_orders(&batch.orders, &batch.user_id);

    // Same markets (every market of the batch has a key).
    let mut reference_markets: Vec<&String> = reference_map.keys().collect();
    reference_markets.sort();
    let new_markets: Vec<&String> = new.iter().map(|(market, _)| market).collect();
    assert_eq!(
        reference_markets, new_markets,
        "{}: markets differ",
        context
    );

    // Same matches per market, in the same order, with the same values.
    let mut total = 0;
    for (market_id, new_matches) in &new {
        let reference_matches = &reference_map[market_id];
        assert_eq!(
            fingerprints(reference_matches),
            fingerprints(new_matches),
            "{}: matches of {} differ",
            context,
            market_id
        );
        total += new_matches.len();
    }
    assert_eq!(
        reference_all.len(),
        total,
        "{}: match counts differ",
        context
    );

    // The new response map is the same as the old one.
    let new_map: HashMap<String, Vec<String>> = new
        .iter()
        .map(|(market, matches)| (market.clone(), fingerprints(matches)))
        .collect();
    let reference_fingerprints: HashMap<String, Vec<String>> = reference_map
        .iter()
        .map(|(market, matches)| (market.clone(), fingerprints(matches)))
        .collect();
    assert_eq!(reference_fingerprints, new_map, "{}", context);
    total
}

#[test]
fn optimized_matching_equals_reference_on_random_batches() {
    let shapes = [
        // Small batches, many collisions of areas and rates.
        Shape {
            markets: 1,
            slots: 1,
            orders: 12,
            areas: 2,
            bid_rates: (0.0, 2.5),
            offer_rates: (0.0, 2.5),
        },
        // Several markets and slots, overlapping prices.
        Shape {
            markets: 3,
            slots: 4,
            orders: 120,
            areas: 6,
            bid_rates: (0.5, 3.0),
            offer_rates: (0.0, 2.5),
        },
        // One big group: long partial-fill chains.
        Shape {
            markets: 1,
            slots: 1,
            orders: 400,
            areas: 40,
            bid_rates: (1.0, 3.0),
            offer_rates: (0.0, 2.0),
        },
        // Bids below all offers: no match at all.
        Shape {
            markets: 2,
            slots: 3,
            orders: 80,
            areas: 10,
            bid_rates: (0.0, 1.0),
            offer_rates: (1.5, 2.5),
        },
        // Every bid above every offer: everything is price compatible.
        Shape {
            markets: 2,
            slots: 2,
            orders: 150,
            areas: 15,
            bid_rates: (2.0, 2.5),
            offer_rates: (0.0, 1.5),
        },
        // Every order in the same area: nothing may match.
        Shape {
            markets: 2,
            slots: 2,
            orders: 60,
            areas: 1,
            bid_rates: (0.0, 2.5),
            offer_rates: (0.0, 2.5),
        },
        // Many markets and slots, few orders per group.
        Shape {
            markets: 8,
            slots: 12,
            orders: 300,
            areas: 30,
            bid_rates: (0.0, 2.5),
            offer_rates: (0.0, 2.5),
        },
    ];

    let mut nonce = 0u32;
    let mut batches = 0;
    let mut matches = 0;
    for (shape_index, shape) in shapes.iter().enumerate() {
        // Fewer seeds for the large shapes: the reference is slow in debug
        // builds (it serializes and hashes both orders per pair).
        let seeds = if shape.orders >= 300 { 15 } else { 60 };
        for seed in 0..seeds {
            let mut rng = Rng::new(seed * 1_000 + shape_index as u64);
            let batch = random_batch(&mut rng, shape, &mut nonce);
            matches += assert_same_matches(&batch, &format!("shape {} seed {}", shape_index, seed));
            batches += 1;
        }
    }
    // Guard against a generator that never produces matches.
    assert!(
        matches > batches,
        "only {} matches in {} batches",
        matches,
        batches
    );
}

#[test]
fn optimized_matching_equals_reference_with_shuffled_submission_order() {
    // Ties in energy_rate are decided by the submission order (stable sort),
    // so permutations of the same batch are covered too.
    let shape = Shape {
        markets: 2,
        slots: 2,
        orders: 60,
        areas: 5,
        bid_rates: (0.0, 2.0),
        offer_rates: (0.0, 2.0),
    };
    let mut nonce = 0u32;
    for seed in 0..40u64 {
        let mut rng = Rng::new(9_000 + seed);
        let mut batch = random_batch(&mut rng, &shape, &mut nonce);
        for permutation in 0..5 {
            rng.shuffle(&mut batch.orders);
            assert_same_matches(
                &batch,
                &format!("seed {} permutation {}", seed, permutation),
            );
        }
    }
}

/// The benchmark payloads: realistic orders (UUID market ids, 96 slots,
/// SS58-like accounts) at plan sizes.
#[test]
fn optimized_matching_equals_reference_on_benchmark_payloads() {
    for (shape, bytes) in [
        (BatchShape::Realistic, 50 * 1024),
        (BatchShape::Realistic, 500 * 1024),
        (BatchShape::WorstCase, 50 * 1024),
        (BatchShape::WorstCase, 200 * 1024),
    ] {
        let (batch, _) = batch_of_size(shape, bytes, 11);
        let matches = assert_same_matches(&batch, &format!("{} {} bytes", shape.name(), bytes));
        assert!(matches > 0);
    }
}

#[test]
fn optimized_matching_on_empty_and_one_sided_batches() {
    let mut nonce = 0u32;
    let mut rng = Rng::new(7);
    let shape = Shape {
        markets: 3,
        slots: 2,
        orders: 30,
        areas: 4,
        bid_rates: (0.0, 2.0),
        offer_rates: (0.0, 2.0),
    };
    let batch = random_batch(&mut rng, &shape, &mut nonce);

    let empty = OrdersToMatch {
        user_id: "u".to_string(),
        orders: Vec::new(),
    };
    assert_eq!(assert_same_matches(&empty, "empty"), 0);
    assert!(match_orders(&[], "u").is_empty());

    for keep_bids in [true, false] {
        let one_sided = OrdersToMatch {
            user_id: batch.user_id.clone(),
            orders: batch
                .orders
                .iter()
                .filter(|o| matches!(o.order, Order::Bid(_)) == keep_bids)
                .cloned()
                .collect(),
        };
        assert_eq!(assert_same_matches(&one_sided, "one sided"), 0);
    }
}

fn component(area: &str, energy: f64, energy_rate: f64) -> DbOrderComponent {
    DbOrderComponent {
        area_uuid: area.to_string(),
        market_id: "m".to_string(),
        time_slot: 900,
        creation_time: 1,
        energy,
        energy_rate,
    }
}

fn offer(id: &str, energy: f64, energy_rate: f64) -> MatchingOffer {
    MatchingOffer {
        order_id: id.to_string(),
        offer: DbOffer {
            seller: "seller".to_string(),
            nonce: 1,
            offer_component: component("area-seller", energy, energy_rate),
        },
    }
}

fn bid(id: &str, energy: f64, energy_rate: f64) -> MatchingBid {
    MatchingBid {
        order_id: id.to_string(),
        bid: DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: component("area-buyer", energy, energy_rate),
        },
    }
}

/// Two offers with different `_id`s but identical content. Before MS-10 the
/// remaining energy was keyed by the hash of the order content, so the two
/// offers shared 5 kWh and only the first one matched (a bug). Now each
/// submitted order has its own remaining energy and both are matched.
#[test]
fn two_orders_with_identical_content_match_independently() {
    let bids = vec![bid("bid-1", 20.0, 20.0)];
    let offers = vec![offer("offer-1", 5.0, 10.0), offer("offer-2", 5.0, 10.0)];

    let mut reference = ReferenceMatchingData {
        bids: bids.clone(),
        offers: offers.clone(),
        market_id: "m".to_string(),
        user_id: "u".to_string(),
    };
    let old = reference.pay_as_bid();
    assert_eq!(old.len(), 1, "the old code shared the energy of offer-2");

    let mut data = DbMatchingData {
        bids,
        offers,
        market_id: "m".to_string(),
        user_id: "u".to_string(),
    };
    let new = data.pay_as_bid();
    assert_eq!(new.len(), 2);
    assert_eq!(new[0].offer_order_id, "offer-1");
    assert_eq!(new[1].offer_order_id, "offer-2");
    for m in &new {
        assert_eq!(m.bid_order_id, "bid-1");
        assert_eq!(m.selected_energy, 5.0);
        assert!(m.residual_offer.is_none());
    }
    assert_eq!(
        new[0].residual_bid.as_ref().unwrap().bid_component.energy,
        15.0
    );
    assert_eq!(
        new[1].residual_bid.as_ref().unwrap().bid_component.energy,
        10.0
    );

    // The same for two identical bids against one offer.
    let mut data = DbMatchingData {
        bids: vec![bid("bid-1", 3.0, 20.0), bid("bid-2", 3.0, 20.0)],
        offers: vec![offer("offer-1", 10.0, 10.0)],
        market_id: "m".to_string(),
        user_id: "u".to_string(),
    };
    let new = data.pay_as_bid();
    assert_eq!(new.len(), 2);
    assert_eq!(new[0].bid_order_id, "bid-1");
    assert_eq!(new[1].bid_order_id, "bid-2");
    assert_eq!(
        new[1]
            .residual_offer
            .as_ref()
            .unwrap()
            .offer_component
            .energy,
        4.0
    );
}

/// `pay_as_bid` does not reorder the caller's bids and offers.
#[test]
fn pay_as_bid_leaves_its_input_unchanged() {
    let bids = vec![bid("b-low", 1.0, 11.0), bid("b-high", 1.0, 30.0)];
    let offers = vec![offer("o-high", 1.0, 12.0), offer("o-low", 1.0, 10.0)];
    let mut data = DbMatchingData {
        bids: bids.clone(),
        offers: offers.clone(),
        market_id: "m".to_string(),
        user_id: "u".to_string(),
    };
    let matches = data.pay_as_bid();
    assert_eq!(data.bids, bids);
    assert_eq!(data.offers, offers);
    // Cheapest offer first, against the highest bid.
    assert_eq!(matches[0].offer_order_id, "o-low");
    assert_eq!(matches[0].bid_order_id, "b-high");
}

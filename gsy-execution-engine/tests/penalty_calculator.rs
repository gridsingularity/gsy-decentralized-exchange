use gsy_execution_engine::primitives::penalty_calculator::{
    compute_delivery_verifications, compute_penalties,
};
use primitives::db_api_schema::{
    orders::{DbOrderSchema, OrderEnum, OrderStatus},
    profiles::MeasurementSchema,
    trades::{DbTradeSchema, TradeParameters, TradeStatus},
};
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};
use std::collections::HashMap;

fn onchain_id(id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(id))
}

fn order(order_id: &str, facility_id: &str, is_bid: bool) -> DbOrderSchema {
    let actor_id = bytes16_to_hex(create_encrypted_bytes16_from_string(facility_id));
    DbOrderSchema {
        order_id: order_id.to_string(),
        status: OrderStatus::Executed,
        status_updated_at: None,
        order_type: if is_bid {
            OrderEnum::Bid
        } else {
            OrderEnum::Offer
        },
        area_uuid: actor_id.clone(),
        market_id: "market-1".to_string(),
        time_slot: 1_000,
        creation_time: 900,
        energy_kWh: 10.0,
        energy_rate: 1.0,
        created_by: actor_id,
        requirements: None,
        attributes: None,
    }
}

fn trade() -> DbTradeSchema {
    let bid = order("bid-1", "alice", true);
    let offer = order("offer-1", "bob", false);
    DbTradeSchema {
        trade_uuid: "trade-1".to_string(),
        status: TradeStatus::Settled,
        seller: offer.created_by.clone(),
        buyer: bid.created_by.clone(),
        market_id: "market-1".to_string(),
        creation_time: 950,
        offer_hash: "offer-1".to_string(),
        bid_hash: "bid-1".to_string(),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 10.0,
            energy_rate: 1.0,
        },
        status_updated_at: None,
    }
}

#[test]
fn compute_penalties_matches_facility_measurements_to_onchain_ids() {
    let measurements = vec![MeasurementSchema {
        facility_id: "areaalice".to_string(),
        community_uuid: "community1".to_string(),
        time_slot: 1_000,
        creation_time: 1_000,
        energy_kwh: 12.0,
    }];

    // The mapping comes from the same ID service used by the community client.
    let facility_onchain_ids = HashMap::from([("areaalice".to_string(), onchain_id("alice"))]);

    let penalties = compute_penalties(&[trade()], &measurements, &facility_onchain_ids, 0.10);

    assert_eq!(penalties.len(), 1);
    assert_eq!(penalties[0].penalty_cost, 2_000);
    assert_eq!(
        penalties[0].penalized_account,
        bytes16_to_hex(create_encrypted_bytes16_from_string("alice"))
    );
}

#[test]
fn verification_distinguishes_missing_measurements_from_zero_penalties() {
    let mapping = HashMap::from([("meter".to_string(), onchain_id("alice"))]);
    let measurement = MeasurementSchema {
        facility_id: "meter".to_string(),
        community_uuid: "community1".to_string(),
        time_slot: 1000,
        creation_time: 1000,
        energy_kwh: 10.0,
    };
    assert!(compute_delivery_verifications(&[trade()], &[], &mapping, 0.1).is_empty());
    assert!(compute_delivery_verifications(
        &[trade()],
        &[measurement.clone()],
        &HashMap::new(),
        0.1
    )
    .is_empty());
    let verdicts = compute_delivery_verifications(&[trade()], &[measurement], &mapping, 0.1);
    assert_eq!(verdicts.len(), 1);
    assert_eq!(verdicts[0].buyer_penalty_cost, 0);
    assert_eq!(verdicts[0].seller_penalty_cost, 0);
}

#[test]
fn verification_preserves_buyer_first_and_seller_fallback_policy() {
    let mapping = HashMap::from([
        ("buyer-meter".to_string(), onchain_id("alice")),
        ("seller-meter".to_string(), onchain_id("bob")),
    ]);
    let buyer = MeasurementSchema {
        facility_id: "buyer-meter".to_string(),
        community_uuid: "community1".to_string(),
        time_slot: 1000,
        creation_time: 1000,
        energy_kwh: 12.0,
    };
    let seller = MeasurementSchema {
        facility_id: "seller-meter".to_string(),
        energy_kwh: 8.0,
        ..buyer.clone()
    };
    let both = compute_delivery_verifications(&[trade()], &[buyer, seller.clone()], &mapping, 0.1);
    assert_eq!(
        (both[0].buyer_penalty_cost, both[0].seller_penalty_cost),
        (2000, 0)
    );
    let fallback = compute_delivery_verifications(&[trade()], &[seller], &mapping, 0.1);
    assert_eq!(
        (
            fallback[0].buyer_penalty_cost,
            fallback[0].seller_penalty_cost
        ),
        (0, 2000)
    );
}

#[test]
fn verification_ignores_unrelated_or_invalid_measurements() {
    let mapping = HashMap::from([("meter".to_string(), onchain_id("charlie"))]);
    let measurement = MeasurementSchema {
        facility_id: "meter".to_string(),
        community_uuid: "community1".to_string(),
        time_slot: 1000,
        creation_time: 1000,
        energy_kwh: 10.0,
    };
    assert!(
        compute_delivery_verifications(&[trade()], &[measurement.clone()], &mapping, 0.1)
            .is_empty()
    );
    let mapping = HashMap::from([("meter".to_string(), onchain_id("alice"))]);
    let measurement = MeasurementSchema {
        energy_kwh: f64::NAN,
        ..measurement
    };
    assert!(compute_delivery_verifications(&[trade()], &[measurement], &mapping, 0.1).is_empty());
}

#[test]
fn verification_keeps_distinct_facilities_separate_and_uses_stored_ids() {
    // Two facilities may share an owner; only their mapped facility IDs identify trades.
    let first_id = "0x11111111111111111111111111111111";
    let second_id = "0x22222222222222222222222222222222";
    let mapping = HashMap::from([
        ("first-meter".into(), first_id.into()),
        ("second-meter".into(), second_id.into()),
    ]);
    let mut first = trade();
    first.buyer = first_id.into();
    let mut second = first.clone();
    second.trade_uuid = "trade-2".into();
    second.buyer = second_id.into();
    let measurement = MeasurementSchema {
        facility_id: "first-meter".into(),
        community_uuid: "community1".into(),
        time_slot: 1000,
        creation_time: 1000,
        energy_kwh: 12.0,
    };
    let trades = [first, second];
    let measurements = [measurement];
    let verifications = compute_delivery_verifications(&trades, &measurements, &mapping, 0.1);
    assert_eq!(verifications.len(), 1);
    assert_eq!(verifications[0].trade_uuid, "trade-1");
    let penalties = compute_penalties(&trades, &measurements, &mapping, 0.1);
    assert_eq!(penalties.len(), 1);
    assert_eq!(penalties[0].penalized_account, first_id);
}

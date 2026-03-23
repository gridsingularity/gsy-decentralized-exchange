use ethers::types::Address;
use gsy_community_client::node_connector::orders::create_input_orders;
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::NODE_FLOAT_SCALING_FACTOR;
use gsy_offchain_primitives::MarketType;
use std::collections::HashSet;
use std::str::FromStr;

fn test_market() -> MarketTopologySchema {
    MarketTopologySchema {
        creation_time: 345_345,
        time_slot: 456_456,
        market_id: format!("0x{}", "11".repeat(32)),
        market_type: MarketType::Spot,
        community_uuid: "community1".to_string(),
        community_name: "My Community".to_string(),
        community_areas: vec![
            AreaTopologySchema {
                area_uuid: "area1".to_string(),
                name: "Area 1".to_string(),
                area_type: "Area".to_string(),
            },
            AreaTopologySchema {
                area_uuid: "area2".to_string(),
                name: "Area 2".to_string(),
                area_type: "Area".to_string(),
            },
        ],
    }
}

#[test]
fn test_orders_to_evm_params_are_created_correctly() {
    let forecasts: Vec<ForecastSchema> = vec![
        ForecastSchema {
            area_uuid: "area1".to_string(),
            creation_time: 123_123,
            time_slot: 456_456,
            energy_kwh: 12.0,
            community_uuid: "community1".to_string(),
            confidence: 0.8,
        },
        ForecastSchema {
            area_uuid: "area2".to_string(),
            creation_time: 234_234,
            time_slot: 456_456,
            energy_kwh: -1.0,
            community_uuid: "community1".to_string(),
            confidence: 0.1,
        },
    ];

    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let input_orders = create_input_orders(forecasts, market.clone(), owner);
    assert_eq!(input_orders.len(), 2);

    let current_time = get_current_timestamp_in_secs();

    let (
        bid_owner,
        _bid_nonce,
        _bid_area,
        _bid_market,
        bid_slot,
        bid_creation,
        bid_energy,
        bid_rate,
        bid_type,
    ) = input_orders[0];
    assert_eq!(bid_owner, owner);
    assert_eq!(bid_slot, market.time_slot as u64);
    assert!(current_time >= bid_creation && current_time - bid_creation <= 1);
    assert_eq!(bid_energy, (12.0 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert_eq!(bid_rate, (12.0 * 0.3 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert!(bid_type);

    let (
        offer_owner,
        _offer_nonce,
        _offer_area,
        _offer_market,
        offer_slot,
        offer_creation,
        offer_energy,
        offer_rate,
        offer_type,
    ) = input_orders[1];
    assert_eq!(offer_owner, owner);
    assert_eq!(offer_slot, market.time_slot as u64);
    assert!(current_time >= offer_creation && current_time - offer_creation <= 1);
    assert_eq!(offer_energy, (1.0 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert_eq!(offer_rate, (1.0 * 0.07 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert!(!offer_type);
}

#[test]
fn test_create_input_orders_skips_unknown_area_uuid() {
    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let forecasts = vec![
        ForecastSchema {
            area_uuid: "area1".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: 5.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            area_uuid: "missing-area".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: 7.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market, owner);
    assert_eq!(orders.len(), 1);
}

#[test]
fn test_create_input_orders_skips_zero_energy_forecasts() {
    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let forecasts = vec![
        ForecastSchema {
            area_uuid: "area1".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: 0.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            area_uuid: "area2".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: -2.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market, owner);
    assert_eq!(orders.len(), 1);
    assert!(!orders[0].8);
}

#[test]
fn test_create_input_orders_assigns_unique_nonces_and_stable_side_mapping() {
    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let forecasts = vec![
        ForecastSchema {
            area_uuid: "area1".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: 2.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            area_uuid: "area2".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: -3.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            area_uuid: "area1".to_string(),
            creation_time: 1,
            time_slot: market.time_slot as u64,
            energy_kwh: 4.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market, owner);
    assert_eq!(orders.len(), 3);

    assert!(orders[0].8);
    assert!(!orders[1].8);
    assert!(orders[2].8);

    let nonces: HashSet<u64> = orders.iter().map(|order| order.1).collect();
    assert_eq!(nonces.len(), orders.len());
}

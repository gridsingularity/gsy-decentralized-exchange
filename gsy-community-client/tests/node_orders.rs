use ethers::types::Address;
use gsy_community_client::node_connector::orders::create_input_orders;
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use primitives::db_api_schema::market::MarketSchema;
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::utils::{parse_or_hash_bytes16, NODE_FLOAT_SCALING_FACTOR};
use primitives::{MarketType, MatchingAlgorithm};
use std::collections::HashSet;
use std::str::FromStr;

fn test_market() -> MarketSchema {
    MarketSchema {
        market_id: format!("0x{}", "11".repeat(16)),
        community_id: "community-1".to_string(),
        opening_time: "00000000000000456446".to_string(),
        closing_time: "00000000000000456447".to_string(),
        delivery_start_time: "00000000000000456456".to_string(),
        delivery_end_time: "00000000000000456457".to_string(),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: "00000000000000345345".to_string(),
    }
}

#[tokio::test]
async fn test_orders_to_evm_params_are_created_correctly() -> anyhow::Result<()> {
    let forecasts: Vec<ForecastSchema> = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 123_123,
            time_slot: 456_456,
            energy_kwh: 12.0,
            community_uuid: "community1".to_string(),
            confidence: 0.8,
        },
        ForecastSchema {
            facility_id: "area2".to_string(),
            creation_time: 234_234,
            time_slot: 456_456,
            energy_kwh: -1.0,
            community_uuid: "community1".to_string(),
            confidence: 0.1,
        },
    ];

    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let input_orders = create_input_orders(forecasts, market.clone(), owner).await?;
    assert_eq!(input_orders.len(), 2);

    let current_time = get_current_timestamp_in_secs();

    let (
        _bid_order_id,
        bid_created_by,
        bid_market,
        bid_slot,
        bid_creation,
        bid_energy,
        bid_rate,
        bid_type,
    ) = input_orders[0];
    assert_eq!(bid_created_by, parse_or_hash_bytes16("area1"));
    assert_eq!(bid_market, parse_or_hash_bytes16(market.market_id.as_str()));
    assert_eq!(bid_slot, 456_456);
    assert!(current_time >= bid_creation && current_time - bid_creation <= 1);
    assert_eq!(bid_energy, (12.0 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert_eq!(bid_rate, (12.0 * 0.3 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert!(bid_type);

    let (
        _offer_order_id,
        offer_created_by,
        offer_market,
        offer_slot,
        offer_creation,
        offer_energy,
        offer_rate,
        offer_type,
    ) = input_orders[1];
    assert_eq!(offer_created_by, parse_or_hash_bytes16("area2"));
    assert_eq!(
        offer_market,
        parse_or_hash_bytes16(market.market_id.as_str())
    );
    assert_eq!(offer_slot, 456_456);
    assert!(current_time >= offer_creation && current_time - offer_creation <= 1);
    assert_eq!(offer_energy, (1.0 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert_eq!(offer_rate, (1.0 * 0.07 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert!(!offer_type);
    Ok(())
}

#[tokio::test]
async fn test_create_input_orders_keeps_all_non_zero_facility_forecasts() -> anyhow::Result<()> {
    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let forecasts = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 5.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "missing-facility".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 7.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market, owner).await?;
    assert_eq!(orders.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_create_input_orders_skips_zero_energy_forecasts() -> anyhow::Result<()> {
    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let forecasts = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 0.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area2".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: -2.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market, owner).await?;
    assert_eq!(orders.len(), 1);
    assert!(!orders[0].7);
    Ok(())
}

#[tokio::test]
async fn test_create_input_orders_assigns_unique_order_ids_and_stable_side_mapping()
    -> anyhow::Result<()> {
    let market = test_market();
    let owner = Address::from_str("0x1000000000000000000000000000000000000001").unwrap();
    let forecasts = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 2.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area2".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: -3.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 4.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market, owner).await?;
    assert_eq!(orders.len(), 3);

    assert!(orders[0].7);
    assert!(!orders[1].7);
    assert!(orders[2].7);

    let order_ids: HashSet<[u8; 16]> = orders.iter().map(|order| order.0).collect();
    assert_eq!(order_ids.len(), orders.len());
    Ok(())
}

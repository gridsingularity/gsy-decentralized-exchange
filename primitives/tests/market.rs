use primitives::db_api_schema::market::{
    MarketChainRecord, MarketSchema, MarketType, MatchingAlgorithm,
};
use primitives::utils::{
    bytes16_to_uuid_string, derive_onchain_id, epoch_to_datetime, hex_to_bytes16,
    uuid_v4_to_bytes16,
};

const COMMUNITY_ID: &str = "Community-LIC01";

fn record() -> MarketChainRecord {
    MarketChainRecord {
        market_id: *uuid::Uuid::new_v4().as_bytes(),
        community_id: hex_to_bytes16(&derive_onchain_id(COMMUNITY_ID)).unwrap(),
        opening_time: 1_699_998_300,
        closing_time: 1_700_000_100,
        delivery_start_time: 1_700_000_100,
        delivery_end_time: 1_700_001_000,
        market_type: MarketType::Flex.to_evm(),
        matching_algorithm: MatchingAlgorithm::PayAsClear.to_evm(),
        created_at: 1_699_998_301,
    }
}

#[test]
fn converts_chain_record_to_market_schema() {
    let record = record();
    let market = record
        .clone()
        .into_schema(COMMUNITY_ID.to_string())
        .unwrap();

    assert_eq!(
        market,
        MarketSchema {
            market_id: bytes16_to_uuid_string(record.market_id),
            community_id: COMMUNITY_ID.to_string(),
            opening_time: epoch_to_datetime(1_699_998_300).unwrap(),
            closing_time: epoch_to_datetime(1_700_000_100).unwrap(),
            delivery_start_time: epoch_to_datetime(1_700_000_100).unwrap(),
            delivery_end_time: epoch_to_datetime(1_700_001_000).unwrap(),
            market_type: MarketType::Flex,
            matching_algorithm: MatchingAlgorithm::PayAsClear,
            created_at: epoch_to_datetime(1_699_998_301).unwrap(),
        }
    );
}

#[test]
fn market_id_matches_the_parsed_form() {
    let record = record();
    let market = record
        .clone()
        .into_schema(COMMUNITY_ID.to_string())
        .unwrap();

    assert_eq!(
        uuid_v4_to_bytes16(market.market_id.as_str()).ok(),
        Some(record.market_id)
    );
}

#[test]
fn rejects_unknown_market_type() {
    let record = MarketChainRecord {
        market_type: 3,
        ..record()
    };

    let error = record.into_schema(COMMUNITY_ID.to_string()).unwrap_err();

    assert!(error.to_string().contains("Unknown market type 3"));
}

#[test]
fn rejects_unknown_matching_algorithm() {
    let record = MarketChainRecord {
        matching_algorithm: 7,
        ..record()
    };

    let error = record.into_schema(COMMUNITY_ID.to_string()).unwrap_err();

    assert!(error.to_string().contains("Unknown matching algorithm 7"));
}

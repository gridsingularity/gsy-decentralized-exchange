use primitives::db_api_schema::market::{
    MarketChainRecord, MarketSchema, MarketType, MatchingAlgorithm,
};
use primitives::utils::{generate_market_id, parse_uuid_or_hex_bytes16};

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";

fn record() -> MarketChainRecord {
    MarketChainRecord {
        market_id: generate_market_id(COMMUNITY_ID, MarketType::Flex, 1_700_000_100),
        community_id: parse_uuid_or_hex_bytes16(COMMUNITY_ID).unwrap(),
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
    let market = MarketSchema::try_from(record.clone()).unwrap();

    assert_eq!(
        market,
        MarketSchema {
            market_id: format!("0x{}", hex::encode(record.market_id)),
            community_id: COMMUNITY_ID.to_string(),
            opening_time: "00000000001699998300".to_string(),
            closing_time: "00000000001700000100".to_string(),
            delivery_start_time: "00000000001700000100".to_string(),
            delivery_end_time: "00000000001700001000".to_string(),
            market_type: MarketType::Flex,
            matching_algorithm: MatchingAlgorithm::PayAsClear,
            created_at: "00000000001699998301".to_string(),
        }
    );
}

#[test]
fn market_id_matches_the_parsed_form() {
    let record = record();
    let market = MarketSchema::try_from(record.clone()).unwrap();

    assert_eq!(
        parse_uuid_or_hex_bytes16(market.market_id.as_str()),
        Some(record.market_id)
    );
}

#[test]
fn rejects_unknown_market_type() {
    let record = MarketChainRecord {
        market_type: 3,
        ..record()
    };

    let error = MarketSchema::try_from(record).unwrap_err();

    assert!(error.to_string().contains("Unknown market type 3"));
}

#[test]
fn rejects_unknown_matching_algorithm() {
    let record = MarketChainRecord {
        matching_algorithm: 7,
        ..record()
    };

    let error = MarketSchema::try_from(record).unwrap_err();

    assert!(error.to_string().contains("Unknown matching algorithm 7"));
}

//! Market schemas.
//!
//! `MarketSchema` is the canonical ontology-aligned market-opening document.

use crate::utils::{bytes16_to_hex, bytes16_to_uuid_string, timestamp_to_string_with_padding};
pub use crate::{MarketTimeSeriesGranularity, MarketType, MatchingAlgorithm};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AreaTopologySchema {
    pub area_uuid: String,
    pub name: String,
    pub area_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MarketSchema {
    pub market_id: String,
    pub community_id: String,
    pub opening_time: String,
    pub closing_time: String,
    pub delivery_start_time: String,
    pub delivery_end_time: String,
    pub market_type: MarketType,
    pub matching_algorithm: MatchingAlgorithm,
    pub created_at: String,
}

/// A market record as `MarketController` emits it in `NewMarketCreated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketChainRecord {
    pub market_id: [u8; 16],
    pub community_id: [u8; 16],
    pub opening_time: u64,
    pub closing_time: u64,
    pub delivery_start_time: u64,
    pub delivery_end_time: u64,
    pub market_type: u8,
    pub matching_algorithm: u8,
    pub created_at: u64,
}

impl TryFrom<MarketChainRecord> for MarketSchema {
    type Error = anyhow::Error;

    fn try_from(record: MarketChainRecord) -> Result<Self, Self::Error> {
        let market_type = MarketType::from_evm(record.market_type)
            .ok_or_else(|| anyhow!("Unknown market type {}", record.market_type))?;
        let matching_algorithm = MatchingAlgorithm::from_evm(record.matching_algorithm)
            .ok_or_else(|| anyhow!("Unknown matching algorithm {}", record.matching_algorithm))?;

        Ok(MarketSchema {
            market_id: bytes16_to_hex(record.market_id),
            community_id: bytes16_to_uuid_string(record.community_id),
            opening_time: timestamp_to_string_with_padding(record.opening_time),
            closing_time: timestamp_to_string_with_padding(record.closing_time),
            delivery_start_time: timestamp_to_string_with_padding(record.delivery_start_time),
            delivery_end_time: timestamp_to_string_with_padding(record.delivery_end_time),
            market_type,
            matching_algorithm,
            created_at: timestamp_to_string_with_padding(record.created_at),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MarketTimeSeriesSchema {
    pub community_id: String,
    #[serde(default)]
    pub market_ids: Option<Vec<String>>,
    pub period_from: String,
    pub period_until: String,
    pub granularity: MarketTimeSeriesGranularity,
}

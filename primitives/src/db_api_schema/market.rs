//! Market schemas.
//!
//! `MarketSchema` is the canonical ontology-aligned market-opening document.
//! Its times are stored as BSON `Date`s; `EwdsMarketDto` carries them as RFC 3339.

use crate::utils::{bytes16_to_uuid_string, epoch_to_datetime};
pub use crate::{MarketType, MatchingAlgorithm};
use anyhow::anyhow;
use bson::serde_helpers::chrono_datetime_as_bson_datetime;
use chrono::{DateTime, Utc};
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
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub opening_time: DateTime<Utc>,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub closing_time: DateTime<Utc>,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub delivery_start_time: DateTime<Utc>,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub delivery_end_time: DateTime<Utc>,
    pub market_type: MarketType,
    pub matching_algorithm: MatchingAlgorithm,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
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

impl MarketChainRecord {
    /// The caller resolves the community pseudonym through the private mapping.
    pub fn into_schema(self, offchain_community_id: String) -> anyhow::Result<MarketSchema> {
        let record = self;
        let market_type = MarketType::from_evm(record.market_type)
            .ok_or_else(|| anyhow!("Unknown market type {}", record.market_type))?;
        let matching_algorithm = MatchingAlgorithm::from_evm(record.matching_algorithm)
            .ok_or_else(|| anyhow!("Unknown matching algorithm {}", record.matching_algorithm))?;

        Ok(MarketSchema {
            market_id: bytes16_to_uuid_string(record.market_id),
            community_id: offchain_community_id,
            opening_time: epoch_to_datetime(record.opening_time)?,
            closing_time: epoch_to_datetime(record.closing_time)?,
            delivery_start_time: epoch_to_datetime(record.delivery_start_time)?,
            delivery_end_time: epoch_to_datetime(record.delivery_end_time)?,
            market_type,
            matching_algorithm,
            created_at: epoch_to_datetime(record.created_at)?,
        })
    }
}

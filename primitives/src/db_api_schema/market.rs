//! Market schemas.
//!
//! `MarketSchema` is the canonical ontology-aligned market-opening document.

pub use crate::{MarketTimeSeriesGranularity, MarketType, MatchingAlgorithm};
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

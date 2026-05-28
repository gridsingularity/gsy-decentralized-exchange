//! Market schemas.
//!
//! `MarketTopologySchema` is the active GSY community topology contract used by
//! the community client and EVM e2e tests. `MarketSchema` is the ontology-aligned
//! market-opening document from the Intelligent update.

pub use crate::MarketType;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AreaTopologySchema {
    pub area_uuid: String,
    pub name: String,
    pub area_type: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MarketTopologySchema {
    // H256-serialized to string for market id
    pub market_id: String,
    pub market_type: MarketType,
    pub community_uuid: String,
    pub community_name: String,
    pub time_slot: u32,
    pub creation_time: u32,
    pub community_areas: Vec<AreaTopologySchema>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MarketSchema {
    pub market_id: String,
    pub community_id: String,
    pub opening_time: String,
    pub closing_time: String,
    pub delivery_start_time: String,
    pub delivery_end_time: String,
    pub market_type: MarketType,
}

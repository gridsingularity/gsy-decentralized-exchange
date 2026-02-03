use serde::{Deserialize, Serialize};
use crate::MarketType;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AreaTopologySchema {
    pub area_uuid: String,
    pub name: String,
    pub area_type: String
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
    pub community_areas: Vec<AreaTopologySchema>
}

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct AreaTopologySchema {
    pub area_uuid: String,
    pub name: String
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MarketTopologySchema {
    // H256-serialized to string for market id
    pub market_id: String,
    pub community_uuid: String,
    pub community_name: String,
    pub time_slot: u32,
    pub creation_time: u32,
    pub area_uuids: Vec<AreaTopologySchema>
}

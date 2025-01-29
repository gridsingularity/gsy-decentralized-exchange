use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MarketTopologySchema {
    pub market_id: String,
    pub time_slot: u32,
    pub creation_time: u32,
    pub area_uuids: Vec<String>
}
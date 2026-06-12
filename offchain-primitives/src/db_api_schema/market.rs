use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum MarketType {
    Spot,
    Flexibility,
    Settlement,
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

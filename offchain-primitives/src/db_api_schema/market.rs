//! Market schema, as specified in D3.2 §5.3.
//!
//! Separate documents are persisted for each market type (Spot,
//! Flexibility, Settlement) and for each market opening — this lets
//! the database answer queries such as "all currently open markets" or
//! "all spot markets opened in the past 24 hours" without joins.

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

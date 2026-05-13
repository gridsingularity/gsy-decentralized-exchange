//! Trades Storage schemas, as specified in D3.2 §5.3.
//!
//! The same database also holds `Market`, `ClearingResult` and
//! `MarketRole` documents — these all relate to a market's lifecycle
//! and are colocated to allow joint queries (e.g. trades plus clearing
//! result for a given market id).

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum TradeStatus {
    Executed,
    Settled,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeSchema {
    pub trade_id: String,
    pub trade_quantity: f64,
    pub trade_price: f64,
    pub trade_timestamp: String,
    pub time_slot: String,
    pub market_id: String,
    pub trade_status: TradeStatus,
    pub buyer: String,
    pub seller: String,
    pub bid_id: String,
    pub offer_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub residual_bid_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub residual_offer_id: Option<String>,
}

/// Status of a market clearing run.
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum ClearingStatus {
    Cleared,
    Uncleared,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct ClearingResultSchema {
    pub market_id: String,
    pub clearing_status: ClearingStatus,
    pub clearing_price: f64,
    pub total_supply: f64,
    pub total_demand: f64,
    pub traded_quantity: f64,
    pub num_trades: u32,
    pub tx_hash: String,
    pub clearing_time: String,
}

/// Market participation role assigned to one or more parties (e.g.
/// Prosumer, Consumer, DSO). Used by the Client API service to
/// authorise order submissions.
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MarketRoleSchema {
    pub role_name: String,
    pub role_description: String,
    pub assigned_to: Vec<String>,
}

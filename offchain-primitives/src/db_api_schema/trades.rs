
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MarketRoleSchema {
    pub role_name: String,
    pub role_description: String,
    pub assigned_to: Vec<String>,
}

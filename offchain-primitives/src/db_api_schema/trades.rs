#![allow(non_snake_case)]

use crate::db_api_schema::orders::DbOrderSchema;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Trade status
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TradeStatus {
    Executed,
    Settled,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeParameters {
    pub selected_energy_kWh: f64,
    pub energy_rate: f64,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeSchema {
    pub trade_uuid: String,
    pub status: TradeStatus,
    pub seller: String,
    pub buyer: String,
    pub market_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub offer: DbOrderSchema,
    pub offer_hash: String,
    pub bid: DbOrderSchema,
    pub bid_hash: String,
    #[serde(default)]
    pub residual_offer_id: Option<String>,
    #[serde(default)]
    pub residual_bid_id: Option<String>,
    pub residual_offer: Option<DbOrderSchema>,
    pub residual_bid: Option<DbOrderSchema>,
    pub parameters: TradeParameters,
}

impl TradeSchema {
    pub fn eq(&self, other: &Self) -> bool {
        self.trade_uuid == other.trade_uuid
    }
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IntelligentTradeStatus {
    Matched,
    Executed,
    Settled,
    Rejected,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligentTradeSchema {
    pub trade_id: String,
    pub market_id: String,
    pub bid_id: String,
    pub buyer_id: String,
    #[serde(default)]
    pub residual_bid_id: Option<String>,
    pub offer_id: String,
    pub seller_id: String,
    #[serde(default)]
    pub residual_offer_id: Option<String>,
    pub trade_status: IntelligentTradeStatus,
    pub trade_quantity: f64,
    pub trade_price: f64,
    pub traded_at: String,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub enum IntelligentClearingStatus {
    #[serde(rename = "FINAL")]
    Final,
    #[serde(rename = "PARTIAL")]
    Partial,
    #[serde(rename = "REJECTED")]
    Rejected,
    #[serde(rename = "NO_BID")]
    NoBid,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntelligentNoBidReason {
    InvalidInputs,
    StaleInput,
    HardConstraints,
    PolicyUnavailable,
    DeadlineMissed,
    Timeout,
    OperatorDisabled,
    MarketReject,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligentClearingResultSchema {
    pub market_id: String,
    pub clearing_status: IntelligentClearingStatus,
    #[serde(default)]
    pub no_bid_reason: Option<IntelligentNoBidReason>,
    pub clearing_price: f64,
    pub total_supply: f64,
    pub total_demand: f64,
    pub traded_quantity: f64,
    pub num_trades: u32,
    pub tx_hash: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

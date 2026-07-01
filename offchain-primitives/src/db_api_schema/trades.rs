#![allow(non_snake_case)]

use crate::utils::string_to_timestamp;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TradeStatus {
    Matched,
    Executed,
    Settled,
    Rejected,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeParameters {
    pub selected_energy_kWh: f64,
    pub energy_rate: f64,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct DbTradeSchema {
    pub trade_uuid: String,
    pub status: TradeStatus,
    pub seller: String,
    pub buyer: String,
    pub market_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub offer_id: String,
    pub bid_id: String,
    pub residual_offer_id: String,
    pub residual_bid_id: String,
    pub parameters: TradeParameters,
}

impl DbTradeSchema {
    pub fn eq(&self, other: &Self) -> bool {
        self.trade_uuid == other.trade_uuid
    }
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct MarketRoleSchema {
    pub role_name: String,
    pub role_description: String,
    pub assigned_to: Vec<String>,
}



#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TradeSchema {
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
    pub trade_status: TradeStatus,
    pub trade_quantity: f64,
    pub trade_price: f64,
    pub timestamp: String,
}

impl From<TradeSchema> for DbTradeSchema {
    fn from(t: TradeSchema) -> Self {
        let time_slot = string_to_timestamp(&t.timestamp).unwrap_or(0);

        DbTradeSchema {
            trade_uuid: t.trade_id,
            status: t.trade_status,
            seller: t.seller_id,
            buyer: t.buyer_id,
            market_id: t.market_id,
            time_slot,
            creation_time: time_slot,
            offer_id: t.offer_id,
            bid_id: t.bid_id,
            residual_offer_id: t.residual_offer_id.unwrap_or_default(),
            residual_bid_id: t.residual_bid_id.unwrap_or_default(),
            parameters: TradeParameters {
                selected_energy_kWh: t.trade_quantity,
                energy_rate: t.trade_price,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub enum ClearingStatus {
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
pub enum NoBidReason {
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
pub struct ClearingResultSchema {
    pub market_id: String,
    pub clearing_status: ClearingStatus,
    #[serde(default)]
    pub no_bid_reason: Option<NoBidReason>,
    pub clearing_price: f64,
    pub total_supply: f64,
    pub total_demand: f64,
    pub trade_quantity: f64,
    pub num_trades: u32,
    pub tx_hash: String,
    #[serde(default)]
    pub clearing_time: Option<String>,
}

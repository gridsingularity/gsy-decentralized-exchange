#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

/// Trade status
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TradeStatus {
    Matched,
    Executed,
    Settled,
    Rejected,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TradeParameters {
    pub selected_energy_kWh: f64,
    pub energy_rate: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbTradeSchema {
    pub trade_uuid: String,
    pub status: TradeStatus,
    pub seller: String,
    pub buyer: String,
    pub market_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub offer_hash: String,
    pub bid_hash: String,
    #[serde(default)]
    pub residual_offer_id: Option<String>,
    #[serde(default)]
    pub residual_bid_id: Option<String>,
    pub parameters: TradeParameters,
}

impl DbTradeSchema {
    pub fn eq(&self, other: &Self) -> bool {
        self.trade_uuid == other.trade_uuid
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClearingStatus {
    Final,
    Partial,
    Rejected,
    NoBid,
}

impl ClearingStatus {
    pub fn to_evm(&self) -> u8 {
        match self {
            ClearingStatus::Final => 1,
            ClearingStatus::Partial => 2,
            ClearingStatus::Rejected => 3,
            ClearingStatus::NoBid => 4,
        }
    }

    pub fn from_evm(value: u8) -> Option<Self> {
        match value {
            1 => Some(ClearingStatus::Final),
            2 => Some(ClearingStatus::Partial),
            3 => Some(ClearingStatus::Rejected),
            4 => Some(ClearingStatus::NoBid),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ClearingResultSchema {
    pub market_id: String,
    pub clearing_status: ClearingStatus,
    #[serde(default)]
    pub no_bid_reason: Option<NoBidReason>,
    pub clearing_price: f64,
    pub total_supply: f64,
    pub total_demand: f64,
    pub traded_quantity: f64,
    pub num_trades: u32,
    pub tx_hash: String,
    pub clearing_time: u64,
}

// todo: move this to markets.rs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MarketRoleSchema {
    pub role_name: String,
    pub role_description: String,
    pub assigned_to: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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

impl Default for NoBidReason {
    fn default() -> Self {
        NoBidReason::Timeout
    }
}

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

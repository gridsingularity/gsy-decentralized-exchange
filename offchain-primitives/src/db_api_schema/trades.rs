#![allow(non_snake_case)]

use crate::db_api_schema::orders::DbOrderSchema;
use serde::{Deserialize, Serialize};

/// Trade status
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TradeStatus {
    Settled,
    Executed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TradeParameters {
    pub selected_energy_kWh: f64,
    pub energy_rate: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

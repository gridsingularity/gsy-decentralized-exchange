use serde::{Deserialize, Serialize};
use crate::db_api_schema::orders::DbOrderSchema;


/// Trade status
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TradeStatus {
    Settled,
    Executed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TradeParameters {
    pub selected_energy: f64,
    pub energy_rate: f64,
    pub trade_uuid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TradeSchema {
    pub trade_id: String,
    pub status: TradeStatus,
    pub seller: String,
    pub buyer: String,
    pub market_id: String,
    pub time_slot: u64,
    pub trade_uuid: String,
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
        self.trade_id == other.trade_id
    }

}

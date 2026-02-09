#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderEnum {
    Bid,
    Offer,
}

/// Order component struct
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbOrderComponent {

}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbOrderSchema {
    pub order_id: String,
    pub status: OrderStatus,
    pub order_type: OrderEnum,
    pub area_uuid: String,
    pub market_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kWh: f64,
    pub energy_rate: f64,
    pub created_by: String
}

/// Order status
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderStatus {
    Open,
    Executed,
    Expired,
    Deleted,
}

impl Default for OrderStatus {
    fn default() -> Self {
        Self::Open
    }
}



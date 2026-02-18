#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use subxt::utils::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum EnergyType {
    Clean,
    Battery,
    FossilFuel,
    Import,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbRequirements {
    pub trading_partner_id: Option<String>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbAttributes {
    pub trading_partner_id: Option<String>,
    pub energy_type: EnergyType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum OrderEnum {
    Bid,
    Offer,
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
    pub created_by: String,
    pub requirements: Option<DbRequirements>,
    pub attributes: Option<DbAttributes>,
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

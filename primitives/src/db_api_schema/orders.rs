#![allow(non_snake_case)]

//! Order Book Storage schemas.
//!
//! `DbOrderSchema` is the active EVM/off-chain-storage runtime order shape used by
//! matching, execution, and EWDS order query responses. Additional Intelligent
//! ontology structs are kept alongside it for topic/schema evolution without
//! breaking the current EVM integration path.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum OrderEnum {
    Bid,
    Offer,
}

pub type OrderType = OrderEnum;

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Open,
    Executed,
    Cancelled,
    Expired
}

impl Default for OrderStatus {
    fn default() -> Self {
        Self::Open
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlexibilityOrderType {
    FlexibilityOffer,
    FlexibilityBid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlexibilityOrderSchema {
    pub order_id: String,
    pub order_type: FlexibilityOrderType,
    pub flexibility_type: String,
    pub available_power: f64,
    /// ISO 8601 duration, for example `PT30M`.
    pub duration: String,
    pub activation_window_start: String,
    pub activation_window_end: String,
    pub price: f64,
    pub currency: String,
    pub created_by: String,
    pub from_asset: String,
}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum EnergyType {
    #[serde(rename = "GREEN")]
    Green,
    #[serde(rename = "PV")]
    Pv,
    #[serde(rename = "HYDRO")]
    Hydro,
    #[serde(rename = "BIOMASS")]
    Biomass,
    #[serde(rename = "BATTERY")]
    Battery,
    #[serde(rename = "NONE")]
    None,
}

pub fn energy_type_to_contract(energy_type: &EnergyType) -> u8 {
    match energy_type {
        EnergyType::None => 0,
        EnergyType::Green => 1,
        EnergyType::Pv => 2,
        EnergyType::Hydro => 3,
        EnergyType::Biomass => 4,
        EnergyType::Battery => 5,
    }
}

fn energy_type_from_contract(value: u8) -> Option<EnergyType> {
    match value {
        0 => Some(EnergyType::Nonw),
        1 => Some(EnergyType::Green),
        2 => Some(EnergyType::Pv),
        3 => Some(EnergyType::Hydro),
        4 => Some(EnergyType::Biomass),
        5 => Some(EnergyType::Battery),
    }
}


pub fn order_energy_source_preference_to_contract(order: &DbOrderSchema) -> u8 {
    order
        .requirements
        .as_ref()
        .and_then(|requirements| requirements.energy_type.as_ref())
        .map(energy_type_to_contract)
        .unwrap_or(energy_type_to_contract(&EnergyType::None))
}

pub fn attribute_energy_type_to_contract(order: &DbOrderSchema) -> u8 {
    order
        .attributes
        .as_ref()
        .map(|attributes| energy_type_to_contract(&attributes.energy_type))
        .unwrap_or(energy_type_to_contract(&EnergyType::None))
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct DbRequirements {
    pub trading_partner_id: Option<String>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct DbAttributes {
    pub trading_partner_id: Option<String>,
    pub energy_type: EnergyType,
}
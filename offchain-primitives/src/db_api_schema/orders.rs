#![allow(non_snake_case)]

//! Order Book Storage schemas.
//!
//! `DbOrderSchema` is the active EVM/off-chain-storage runtime order shape used by
//! matching, execution, and EWDS order query responses. Additional Intelligent
//! ontology structs are kept alongside it for topic/schema evolution without
//! breaking the current EVM integration path.

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result, Error};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct DbRequirements {
    pub trading_partner_id: Option<String>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct DbAttributes {
    pub trading_partner_id: Option<String>,
    pub energy_type: EnergyType,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct DbOrderSchema {
    pub order_id: String,
    pub status: OrderStatus,
    pub order_type: OrderType,
    pub area_uuid: String,
    pub market_id: String,
    #[serde(default)]
    pub nonce: Option<u64>,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kWh: f64,
    pub energy_rate: f64,
    pub created_by: String,
    pub requirements: Option<DbRequirements>,
    pub attributes: Option<DbAttributes>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlexibilityOrderType {
    FlexibilityOffer,
    FlexibilityBid,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq, PartialOrd)]
pub enum OrderType {
    Bid,
    Offer,
}
pub fn order_type_to_string(order_type: &OrderType) -> &'static str {
    match order_type {
        OrderType::Bid => "bid",
        OrderType::Offer => "offer",
    }
}

pub fn string_to_order_type(value: &str) -> Result<OrderType, Error> {
    match value.to_ascii_lowercase().as_str() {
        "bid" => Ok(OrderType::Bid),
        "offer" => Ok(OrderType::Offer),
        _ => Err(anyhow!("unsupported order type '{}'", value)),
    }
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq, PartialOrd)]
pub enum OrderStatus {
    Submitted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Rejected,
    Executed,
}

pub fn order_status_to_string(status: &OrderStatus) -> &'static str {
    match status {
        OrderStatus::Submitted => "submitted",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Expired => "expired",
        OrderStatus::Rejected => "rejected",
        OrderStatus::Executed => "executed",
    }
}

pub fn string_to_order_status(value: &str) -> Result<OrderStatus> {
    match value.to_ascii_lowercase().as_str() {
        "submitted" => Ok(OrderStatus::Submitted),
        "partially_filled" => Ok(OrderStatus::PartiallyFilled),
        "filled" => Ok(OrderStatus::Filled),
        "cancelled" => Ok(OrderStatus::Cancelled),
        "expired" => Ok(OrderStatus::Expired),
        "rejected" => Ok(OrderStatus::Rejected),
        "executed" => Ok(OrderStatus::Executed),
        _ => Err(anyhow!("unsupported order status '{}'", value)),
    }
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq, PartialOrd)]
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
    #[serde(rename = "GREY")]
    Grey,
}

pub fn energy_type_to_string(energy_type: Option<EnergyType>) -> &'static str {
    match energy_type {
        Some(EnergyType::Green) => "green",
        Some(EnergyType::Pv) => "pv",
        Some(EnergyType::Hydro) => "hydro",
        Some(EnergyType::Biomass) => "biomass",
        Some(EnergyType::Battery) => "battery",
        Some(EnergyType::Grey) => "grey",
        None => "None",
    }
}

pub fn string_to_energy_type(value: &str) -> Result<EnergyType> {
    match value.to_ascii_lowercase().as_str() {
        "green" => Ok(EnergyType::Green),
        "pv" => Ok(EnergyType::Pv),
        "hydro" => Ok(EnergyType::Hydro),
        "biomass" => Ok(EnergyType::Biomass),
        "battery" => Ok(EnergyType::Battery),
        "grey" => Ok(EnergyType::Grey),
        _ => Err(anyhow!("unsupported energy type '{}'", value)),
    }
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct OrderSchema {
    pub order_id: String,
    pub market_id: String,
    pub order_type: OrderType,
    pub order_status: OrderStatus,
    pub time_slot: String,
    pub quantity: f64,
    pub price_limit: f64,
    #[serde(default)]
    pub energy_source_preference: Option<EnergyType>,
    #[serde(default)]
    pub energy_type: Option<EnergyType>,
    pub created_by: String,
    pub creation_time: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub cancellation_reason: Option<String>,
    #[serde(default)]
    pub preferred_trading_partner: Option<String>,
}

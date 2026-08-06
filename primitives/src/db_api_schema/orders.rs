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

/// Order status.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum IntelligentOrderType {
    Bid,
    Offer,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum IntelligentOrderStatus {
    Submitted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Rejected,
    Executed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum IntelligentEnergyType {
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct DbRequirements {
    pub trading_partner_id: Option<String>,
    pub energy_type: Option<IntelligentEnergyType>,
    pub preferred_energy_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct DbAttributes {
    pub trading_partner_id: Option<String>,
    pub energy_type: IntelligentEnergyType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligentOrderSchema {
    pub order_id: String,
    pub market_id: String,
    pub order_type: IntelligentOrderType,
    pub order_status: IntelligentOrderStatus,
    pub time_slot: String,
    pub quantity: f64,
    pub price_limit: f64,
    #[serde(default)]
    pub energy_source_preference: Option<IntelligentEnergyType>,
    #[serde(default)]
    pub energy_type: Option<IntelligentEnergyType>,
    pub created_by: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub cancellation_reason: Option<String>,
    #[serde(default)]
    pub preferred_trading_partner: Option<String>,
}

#![allow(non_snake_case)]

//! Order Book Storage schemas.
//!
//! `DbOrderSchema` is the active EVM/off-chain-storage runtime order shape used by
//! matching, execution, and EWDS order query responses. Additional Intelligent
//! ontology structs are kept alongside it for topic/schema evolution without
//! breaking the current EVM integration path.

use serde::{Deserialize, Serialize};

use crate::utils::{bytes16_to_hex, parse_or_hash_bytes16, NODE_FLOAT_SCALING_FACTOR};

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
    Submitted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Rejected,
    Executed,
}
impl Default for OrderStatus {
    fn default() -> Self {
        Self::Submitted
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

pub fn energy_type_from_contract(value: u8) -> Option<EnergyType> {
    match value {
        0 => Some(EnergyType::None),
        1 => Some(EnergyType::Green),
        2 => Some(EnergyType::Pv),
        3 => Some(EnergyType::Hydro),
        4 => Some(EnergyType::Biomass),
        5 => Some(EnergyType::Battery),
        _ => None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractOrderMetadata {
    pub energy_source_preference: u8,
    pub energy_type: u8,
    pub preferred_trading_partner: [u8; 16],
    pub preferred_energy_rate: u64,
    pub trading_partner: [u8; 16],
}

pub fn order_metadata_to_contract(
    requirements: Option<&DbRequirements>,
    attributes: Option<&DbAttributes>,
) -> ContractOrderMetadata {
    ContractOrderMetadata {
        energy_source_preference: requirements
            .and_then(|value| value.energy_type.as_ref())
            .map(energy_type_to_contract)
            .unwrap_or_default(),
        energy_type: attributes
            .map(|value| energy_type_to_contract(&value.energy_type))
            .unwrap_or_default(),
        preferred_trading_partner: requirements
            .and_then(|value| value.trading_partner_id.as_deref())
            .map(parse_or_hash_bytes16)
            .unwrap_or_default(),
        preferred_energy_rate: requirements
            .and_then(|value| value.preferred_energy_rate)
            .map(|rate| (rate * NODE_FLOAT_SCALING_FACTOR).round() as u64)
            .unwrap_or_default(),
        trading_partner: attributes
            .and_then(|value| value.trading_partner_id.as_deref())
            .map(parse_or_hash_bytes16)
            .unwrap_or_default(),
    }
}

pub fn order_metadata_from_contract(
    metadata: ContractOrderMetadata,
) -> (Option<DbRequirements>, Option<DbAttributes>) {
    let preferred_trading_partner = bytes16_to_optional_hex(metadata.preferred_trading_partner);
    let preferred_energy_type = non_empty_energy_type(metadata.energy_source_preference);
    let preferred_energy_rate = (metadata.preferred_energy_rate != 0)
        .then_some(metadata.preferred_energy_rate as f64 / NODE_FLOAT_SCALING_FACTOR);
    let requirements = if preferred_trading_partner.is_some()
        || preferred_energy_type.is_some()
        || preferred_energy_rate.is_some()
    {
        Some(DbRequirements {
            trading_partner_id: preferred_trading_partner,
            energy_type: preferred_energy_type,
            preferred_energy_rate,
        })
    } else {
        None
    };

    let trading_partner = bytes16_to_optional_hex(metadata.trading_partner);
    let attribute_energy_type = non_empty_energy_type(metadata.energy_type);
    let attributes = if trading_partner.is_some() || attribute_energy_type.is_some() {
        Some(DbAttributes {
            trading_partner_id: trading_partner,
            energy_type: attribute_energy_type.unwrap_or(EnergyType::None),
        })
    } else {
        None
    };

    (requirements, attributes)
}

fn bytes16_to_optional_hex(value: [u8; 16]) -> Option<String> {
    (value != [0; 16]).then(|| bytes16_to_hex(value))
}

fn non_empty_energy_type(value: u8) -> Option<EnergyType> {
    energy_type_from_contract(value).filter(|energy_type| *energy_type != EnergyType::None)
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

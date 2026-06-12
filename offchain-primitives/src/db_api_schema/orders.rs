use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Bid,
    Offer,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct DbOrderSchema {
    pub order_id: String,
    pub order_type: OrderType,
    pub quantity: f64,
    pub price_limit: f64,
    pub time_slot: String,
    pub market_id: String,
    pub order_status: OrderStatus,
    pub creation_time: String,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub energy_source_preference: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub energy_type: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub area_uuid: Option<String>,
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
    /// ISO 8601 duration (e.g. "PT30M").
    pub duration: String,
    pub activation_window_start: String,
    pub activation_window_end: String,
    pub price: f64,
    pub currency: String,
    pub created_by: String,
    pub from_asset: String,
}

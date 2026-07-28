use super::EwdsOperation;
use crate::db_api_schema::orders::{
    DbAttributes, DbOrderSchema, DbRequirements, IntelligentEnergyType, OrderEnum, OrderStatus,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsInboundMessage {
    pub payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsRequestEnvelope {
    #[serde(alias = "request_id")]
    pub request_id: String,
    pub operation: EwdsOperation,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsSendMessageDto {
    pub fqcn: String,
    pub topic_name: String,
    pub topic_version: String,
    pub topic_owner: String,
    pub transaction_id: String,
    pub payload: String,
    pub anonymous_recipient: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsMessageDto {
    pub payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsResponseEnvelope<T> {
    pub request_id: String,
    pub success: bool,
    pub data: Vec<T>,
    pub error: Option<EwdsErrorPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsQueryResponse<T> {
    #[serde(alias = "request_id")]
    pub request_id: String,
    pub success: bool,
    pub data: Option<Vec<T>>,
    pub error: Option<EwdsErrorPayload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EwdsErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsOrderDto {
    pub order_id: String,
    pub market_id: String,
    pub order_type: String,
    pub status: String,
    pub area_uuid: String,
    #[serde(default)]
    pub nonce: Option<u64>,
    pub time_slot: u64,
    pub creation_time: u64,
    pub quantity: f64,
    pub price_limit: f64,
    pub created_by: String,
    #[serde(default)]
    pub requirements: Option<EwdsRequirementsDto>,
    #[serde(default)]
    pub attributes: Option<EwdsAttributesDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsRequirementsDto {
    #[serde(default)]
    pub trading_partner_id: Option<String>,
    #[serde(default)]
    pub energy_type: Option<IntelligentEnergyType>,
    #[serde(default)]
    pub preferred_energy_rate: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsAttributesDto {
    #[serde(default)]
    pub trading_partner_id: Option<String>,
    pub energy_type: IntelligentEnergyType,
}

impl From<DbOrderSchema> for EwdsOrderDto {
    fn from(order: DbOrderSchema) -> Self {
        Self {
            order_id: order.order_id,
            market_id: order.market_id,
            order_type: order_type_to_ewds(&order.order_type).to_string(),
            status: order_status_to_ewds(&order.status).to_string(),
            area_uuid: order.area_uuid,
            nonce: order.nonce,
            time_slot: order.time_slot,
            creation_time: order.creation_time,
            quantity: order.energy_kWh,
            price_limit: order.energy_rate,
            created_by: order.created_by,
            requirements: order.requirements.map(|requirements| EwdsRequirementsDto {
                trading_partner_id: requirements.trading_partner_id,
                energy_type: requirements.energy_type,
                preferred_energy_rate: requirements.preferred_energy_rate,
            }),
            attributes: order.attributes.map(|attributes| EwdsAttributesDto {
                trading_partner_id: attributes.trading_partner_id,
                energy_type: attributes.energy_type,
            }),
        }
    }
}

impl TryFrom<EwdsOrderDto> for DbOrderSchema {
    type Error = anyhow::Error;

    fn try_from(order: EwdsOrderDto) -> Result<Self> {
        let requirements = match order.requirements {
            Some(requirements) => Some(DbRequirements {
                trading_partner_id: requirements.trading_partner_id,
                energy_type: requirements.energy_type,
                preferred_energy_rate: requirements.preferred_energy_rate,
            }),
            None => None,
        };

        let attributes = match order.attributes {
            Some(attributes) => Some(DbAttributes {
                trading_partner_id: attributes.trading_partner_id,
                energy_type: attributes.energy_type,
            }),
            None => None,
        };

        Ok(Self {
            order_id: order.order_id,
            status: order_status_from_ewds(order.status.as_str())?,
            order_type: order_type_from_ewds(order.order_type.as_str())?,
            area_uuid: order.area_uuid,
            market_id: order.market_id,
            nonce: order.nonce,
            time_slot: order.time_slot,
            creation_time: order.creation_time,
            energy_kWh: order.quantity,
            energy_rate: order.price_limit,
            created_by: order.created_by,
            requirements,
            attributes,
        })
    }
}

fn order_type_to_ewds(order_type: &OrderEnum) -> &'static str {
    match order_type {
        OrderEnum::Bid => "bid",
        OrderEnum::Offer => "offer",
    }
}

fn order_status_to_ewds(status: &OrderStatus) -> &'static str {
    match status {
        OrderStatus::Open => "open",
        OrderStatus::Executed => "executed",
        OrderStatus::Expired => "expired",
        OrderStatus::Deleted => "deleted",
    }
}

fn order_type_from_ewds(value: &str) -> Result<OrderEnum> {
    match value.to_ascii_lowercase().as_str() {
        "bid" => Ok(OrderEnum::Bid),
        "offer" => Ok(OrderEnum::Offer),
        _ => Err(anyhow!("unsupported EWDS order type '{}'", value)),
    }
}

fn order_status_from_ewds(value: &str) -> Result<OrderStatus> {
    match value.to_ascii_lowercase().as_str() {
        "open" => Ok(OrderStatus::Open),
        "executed" => Ok(OrderStatus::Executed),
        "expired" => Ok(OrderStatus::Expired),
        "deleted" => Ok(OrderStatus::Deleted),
        _ => Err(anyhow!("unsupported EWDS order status '{}'", value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "order-id".to_string(),
            status: OrderStatus::Open,
            order_type: OrderEnum::Bid,
            area_uuid: "area-id".to_string(),
            market_id: "market-id".to_string(),
            nonce: Some(7),
            time_slot: 10,
            creation_time: 9,
            energy_kWh: 4.5,
            energy_rate: 12.0,
            created_by: "actor-id".to_string(),
            requirements: Some(DbRequirements {
                trading_partner_id: Some("partner-id".to_string()),
                energy_type: Some(IntelligentEnergyType::Green),
                preferred_energy_rate: Some(11.0),
            }),
            attributes: None,
        }
    }

    #[test]
    fn order_conversion_round_trips_through_ewds_dto() {
        let expected = order();

        let actual = DbOrderSchema::try_from(EwdsOrderDto::from(expected.clone()))
            .expect("EWDS order should convert back to DB schema");

        assert_eq!(actual, expected);
    }
}

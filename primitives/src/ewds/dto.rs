use super::EwdsOperation;
use crate::db_api_schema::orders::{
    DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus,
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
    pub order_status: String,
    pub time_slot: u64,
    pub quantity: f64,
    pub price_limit: f64,
    pub energy_source_preference: Option<String>,
    pub energy_type: Option<String>,
    pub created_by: String,
    pub creation_time: u64,
    pub updated_at: Option<u64>,
    pub reject_reason: Option<String>,
    pub preferred_trading_partner: Option<String>
}

impl From<DbOrderSchema> for EwdsOrderDto {
    fn from(order: DbOrderSchema) -> Self {
        Self {
            order_id: order.order_id,
            market_id: order.market_id,
            order_type: order_type_to_ewds(&order.order_type).to_string(),
            order_status: order_status_to_ewds(&order.status).to_string(),
            time_slot: order.time_slot,
            quantity: order.energy_kWh,
            price_limit: order.energy_rate,
            energy_source_preference: order
                .requirements
                .as_ref()
                .and_then(|r| r.energy_type.as_ref())
                .map(|et| energy_type_to_ewds(et).to_string()),
            energy_type: Some(order
                .attributes
                .as_ref()
                .map(|a| energy_type_to_ewds(&a.energy_type).to_string())
                .unwrap_or_else(|| "GREY".to_string())),
            created_by: order.created_by,
            creation_time: order.creation_time,
            updated_at: Some(order.creation_time),
            reject_reason: None,
            preferred_trading_partner: order
                .requirements
                .as_ref()
                .and_then(|r| r.trading_partner_id.clone()),
        }
    }
}

impl TryFrom<EwdsOrderDto> for DbOrderSchema {
    type Error = anyhow::Error;

    fn try_from(order: EwdsOrderDto) -> Result<Self> {
        let requirements = match order.energy_source_preference {
            Some(pref) => Some(DbRequirements {
                trading_partner_id: order.preferred_trading_partner.clone(),
                energy_type: Some(energy_type_from_ewds(&pref)?),
                preferred_energy_rate: Some(order.price_limit),
            }),
            None => None,
        };

        let attributes = match order.preferred_trading_partner {
            Some(pref) => Some(DbAttributes {
                trading_partner_id: Some(pref),
                energy_type: match order.energy_type {
                    Some(ref et) => energy_type_from_ewds(et)?,
                    None => EnergyType::Grey,
                },
            }),
            None => None,
        };

        Ok(Self {
            order_id: order.order_id,
            status: order_status_from_ewds(order.order_status.as_str())?,
            order_type: order_type_from_ewds(order.order_type.as_str())?,
            area_uuid: order.created_by.clone(), // todo: we need to remove this from DbOrderSchema
            market_id: order.market_id,
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
        OrderStatus::Submitted => "submitted",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Expired => "expired",
        OrderStatus::Rejected => "rejected",
        OrderStatus::Executed => "executed",
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
        "submitted" => Ok(OrderStatus::Submitted),
        "partially_filled" => Ok(OrderStatus::PartiallyFilled),
        "filled" => Ok(OrderStatus::Filled),
        "cancelled" => Ok(OrderStatus::Cancelled),
        "expired" => Ok(OrderStatus::Expired),
        "rejected" => Ok(OrderStatus::Rejected),
        "executed" => Ok(OrderStatus::Executed),
        _ => Err(anyhow!("unsupported EWDS order status '{}'", value)),
    }
}

fn energy_type_to_ewds(energy_type: &EnergyType) -> &'static str {
    match energy_type {
        EnergyType::Green => "GREEN",
        EnergyType::Pv => "PV",
        EnergyType::Hydro => "HYDRO",
        EnergyType::Biomass => "BIOMASS",
        EnergyType::Battery => "BATTERY",
        EnergyType::Grey => "GREY",
    }
}

fn energy_type_from_ewds(value: &str) -> Result<EnergyType> {
    match value.to_ascii_uppercase().as_str() {
        "GREEN" => Ok(EnergyType::Green),
        "PV" => Ok(EnergyType::Pv),
        "HYDRO" => Ok(EnergyType::Hydro),
        "BIOMASS" => Ok(EnergyType::Biomass),
        "BATTERY" => Ok(EnergyType::Battery),
        "GREY" => Ok(EnergyType::Grey),
        _ => Err(anyhow!("unsupported EWDS energy type '{}'", value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "order-id".to_string(),
            status: OrderStatus::Submitted,
            order_type: OrderEnum::Bid,
            area_uuid: "actor-id".to_string(),
            market_id: "market-id".to_string(),
            time_slot: 10,
            creation_time: 9,
            energy_kWh: 4.5,
            energy_rate: 12.0,
            created_by: "actor-id".to_string(),
            requirements: Some(DbRequirements {
                trading_partner_id: Some("partner-id".to_string()),
                energy_type: Some(EnergyType::Green),
                preferred_energy_rate: Some(12.0),
            }),
            attributes: None,
        }
    }

    #[test]
    fn db_to_ewds_maps_fields() {
        let dto = EwdsOrderDto::from(order());

        assert_eq!(dto.order_id, "order-id");
        assert_eq!(dto.market_id, "market-id");
        assert_eq!(dto.order_type, "bid");
        assert_eq!(dto.order_status, "submitted");
        assert_eq!(dto.time_slot, 10);
        assert_eq!(dto.quantity, 4.5);
        assert_eq!(dto.price_limit, 12.0);
        assert_eq!(dto.energy_source_preference.as_deref(), Some("GREEN"));
        assert_eq!(dto.energy_type.as_deref(), Some("GREY")); // no attributes -> default
        assert_eq!(dto.preferred_trading_partner.as_deref(), Some("partner-id"));
        assert_eq!(dto.created_by, "actor-id");
    }

    #[test]
    fn ewds_to_db_maps_fields() {
        let db = DbOrderSchema::try_from(EwdsOrderDto::from(order()))
            .expect("EWDS order should convert to DB schema");

        assert_eq!(db.order_id, "order-id");
        assert_eq!(db.market_id, "market-id");
        assert_eq!(db.order_type, OrderEnum::Bid);
        assert_eq!(db.status, OrderStatus::Submitted);
        assert_eq!(db.time_slot, 10);
        assert_eq!(db.energy_kWh, 4.5);
        assert_eq!(db.energy_rate, 12.0);
        assert_eq!(db.created_by, "actor-id");

        // requirements rebuilt from energy_source_preference + preferred_trading_partner
        let req = db.requirements.expect("requirements present");
        assert_eq!(req.trading_partner_id.as_deref(), Some("partner-id"));
        assert_eq!(req.energy_type, Some(EnergyType::Green));
        assert_eq!(req.preferred_energy_rate, Some(12.0));

        // attributes rebuilt from preferred_trading_partner + energy_type ("GREY")
        let attr = db.attributes.expect("attributes present");
        assert_eq!(attr.trading_partner_id.as_deref(), Some("partner-id"));
        assert_eq!(attr.energy_type, EnergyType::Grey);
    }

    #[test]
    fn energy_type_round_trips() {
        for et in [
            EnergyType::Green,
            EnergyType::Pv,
            EnergyType::Hydro,
            EnergyType::Biomass,
            EnergyType::Battery,
            EnergyType::Grey,
        ] {
            let s = energy_type_to_ewds(&et);
            assert_eq!(energy_type_from_ewds(s).unwrap(), et);
        }
    }

    #[test]
    fn unknown_energy_type_is_error() {
        assert!(energy_type_from_ewds("PLUTONIUM").is_err());
    }
}
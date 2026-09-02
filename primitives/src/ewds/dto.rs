use super::EwdsOperation;
use crate::db_api_schema::{
    market::{MarketSchema, MarketType, MatchingAlgorithm},
    orders::{DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus},
    trades::{
        ClearingResultSchema, ClearingStatus, DbTradeSchema, NoBidReason, TradeParameters,
        TradeStatus,
    },
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub preferred_trading_partner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_energy_rate: Option<f64>,
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
            energy_type: Some(
                order
                    .attributes
                    .as_ref()
                    .map(|a| energy_type_to_ewds(&a.energy_type).to_string())
                    .unwrap_or_else(|| "NONE".to_string()),
            ),
            created_by: order.created_by,
            creation_time: order.creation_time,
            updated_at: Some(order.creation_time),
            reject_reason: None,
            preferred_trading_partner: order
                .requirements
                .as_ref()
                .and_then(|r| r.trading_partner_id.clone()),
            preferred_energy_rate: order
                .requirements
                .as_ref()
                .and_then(|r| r.preferred_energy_rate),
        }
    }
}

impl TryFrom<EwdsOrderDto> for DbOrderSchema {
    type Error = anyhow::Error;

    fn try_from(order: EwdsOrderDto) -> Result<Self> {
        let requirements = if order.energy_source_preference.is_some()
            || order.preferred_trading_partner.is_some()
            || order.preferred_energy_rate.is_some()
        {
            Some(DbRequirements {
                trading_partner_id: order.preferred_trading_partner.clone(),
                energy_type: match order.energy_source_preference {
                    Some(ref pref) => Some(energy_type_from_ewds(pref)?),
                    None => None,
                },
                preferred_energy_rate: order.preferred_energy_rate.or_else(|| {
                    order
                        .preferred_trading_partner
                        .as_ref()
                        .map(|_| order.price_limit)
                }),
            })
        } else {
            None
        };

        let attributes = match order.energy_type {
            Some(ref et) => Some(DbAttributes {
                trading_partner_id: None,
                energy_type: energy_type_from_ewds(et)?,
            }),
            None => None,
        };

        Ok(Self {
            order_id: order.order_id,
            status: order_status_from_ewds(order.order_status.as_str())?,
            order_type: order_type_from_ewds(order.order_type.as_str())?,
            area_uuid: order.created_by.clone(),
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

pub fn order_type_to_ewds(order_type: &OrderEnum) -> &'static str {
    match order_type {
        OrderEnum::Bid => "bid",
        OrderEnum::Offer => "offer",
    }
}

pub fn order_status_to_ewds(status: &OrderStatus) -> &'static str {
    match status {
        OrderStatus::Submitted => "submitted",
        OrderStatus::PartiallyFilled => "partially_filled",
        OrderStatus::Filled => "filled",
        OrderStatus::Executed => "executed",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Expired => "expired",
        OrderStatus::Rejected => "rejected",
    }
}

fn order_type_from_ewds(value: &str) -> Result<OrderEnum> {
    match value.to_ascii_lowercase().as_str() {
        "bid" => Ok(OrderEnum::Bid),
        "offer" => Ok(OrderEnum::Offer),
        _ => Err(anyhow!("unsupported EWDS order type '{}'", value)),
    }
}

pub fn order_status_from_ewds(value: &str) -> Result<OrderStatus> {
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

pub fn energy_type_to_ewds(energy_type: &EnergyType) -> &'static str {
    match energy_type {
        EnergyType::Green => "GREEN",
        EnergyType::Pv => "PV",
        EnergyType::Hydro => "HYDRO",
        EnergyType::Biomass => "BIOMASS",
        EnergyType::Battery => "BATTERY",
        EnergyType::None => "NONE",
    }
}

pub fn energy_type_from_ewds(value: &str) -> Result<EnergyType> {
    match value.to_ascii_uppercase().as_str() {
        "GREEN" => Ok(EnergyType::Green),
        "PV" => Ok(EnergyType::Pv),
        "HYDRO" => Ok(EnergyType::Hydro),
        "BIOMASS" => Ok(EnergyType::Biomass),
        "BATTERY" => Ok(EnergyType::Battery),
        "NONE" => Ok(EnergyType::None),
        _ => Err(anyhow!("unsupported EWDS energy type '{}'", value)),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EwdsTradeDto {
    pub trade_id: String,
    pub market_id: String,
    pub bid_id: String,
    pub buyer_id: String,
    #[serde(default)]
    pub residual_bid_id: Option<String>,
    pub offer_id: String,
    pub seller_id: String,
    #[serde(default)]
    pub residual_offer_id: Option<String>,
    pub trade_status: String,
    pub trade_quantity: f64,
    pub trade_price: f64,
    pub timestamp: u64,
}

fn trade_status_to_ewds(status: &TradeStatus) -> &'static str {
    match status {
        TradeStatus::Matched => "matched",
        TradeStatus::Executed => "executed",
        TradeStatus::Settled => "settled",
        TradeStatus::Rejected => "rejected",
    }
}

fn trade_status_from_ewds(value: &str) -> Result<TradeStatus> {
    match value.to_ascii_lowercase().as_str() {
        "matched" => Ok(TradeStatus::Matched),
        "executed" => Ok(TradeStatus::Executed),
        "settled" => Ok(TradeStatus::Settled),
        "rejected" => Ok(TradeStatus::Rejected),
        _ => Err(anyhow!("unsupported EWDS trade status '{}'", value)),
    }
}

impl From<DbTradeSchema> for EwdsTradeDto {
    fn from(trade: DbTradeSchema) -> Self {
        Self {
            trade_id: trade.trade_uuid,
            market_id: trade.market_id,
            bid_id: trade.bid_hash,
            buyer_id: trade.buyer,
            residual_bid_id: trade.residual_bid_id,
            offer_id: trade.offer_hash,
            seller_id: trade.seller,
            residual_offer_id: trade.residual_offer_id,
            trade_status: trade_status_to_ewds(&trade.status).to_string(),
            trade_quantity: trade.parameters.selected_energy_kWh,
            trade_price: trade.parameters.energy_rate,
            timestamp: trade.time_slot,
        }
    }
}

impl TryFrom<EwdsTradeDto> for DbTradeSchema {
    type Error = anyhow::Error;

    fn try_from(trade: EwdsTradeDto) -> Result<Self> {
        Ok(Self {
            trade_uuid: trade.trade_id,
            status: trade_status_from_ewds(&trade.trade_status)?,
            seller: trade.seller_id,
            buyer: trade.buyer_id,
            market_id: trade.market_id,
            time_slot: trade.timestamp,
            creation_time: trade.timestamp,
            offer_hash: trade.offer_id,
            bid_hash: trade.bid_id,
            residual_offer_id: trade.residual_offer_id,
            residual_bid_id: trade.residual_bid_id,
            parameters: TradeParameters {
                selected_energy_kWh: trade.trade_quantity,
                energy_rate: trade.trade_price,
            },
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EwdsClearingResultDto {
    pub market_id: String,
    pub clearing_status: String,
    #[serde(default)]
    pub no_bid_reason: Option<String>,
    pub clearing_price: f64,
    pub total_supply: f64,
    pub total_demand: f64,
    pub trade_quantity: f64,
    pub num_trades: u32,
    pub tx_hash: String,
    pub created_at: u64,
}

impl ClearingStatus {
    fn as_wire(&self) -> &'static str {
        match self {
            ClearingStatus::Final => "final",
            ClearingStatus::Partial => "partial",
            ClearingStatus::Rejected => "rejected",
            ClearingStatus::NoBid => "no_bid",
        }
    }
}

impl FromStr for ClearingStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "final" => Ok(ClearingStatus::Final),
            "partial" => Ok(ClearingStatus::Partial),
            "rejected" => Ok(ClearingStatus::Rejected),
            "no_bid" => Ok(ClearingStatus::NoBid),
            other => Err(anyhow!("unknown clearing status: {other}")),
        }
    }
}

impl NoBidReason {
    fn as_wire(&self) -> &'static str {
        match self {
            NoBidReason::InvalidInputs => "invalid_inputs",
            NoBidReason::StaleInput => "stale_input",
            NoBidReason::HardConstraints => "hard_constraints",
            NoBidReason::PolicyUnavailable => "policy_unavailable",
            NoBidReason::DeadlineMissed => "deadline_missed",
            NoBidReason::Timeout => "timeout",
            NoBidReason::OperatorDisabled => "operator_disabled",
            NoBidReason::MarketReject => "market_reject",
        }
    }
}

impl FromStr for NoBidReason {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "invalid_inputs" => Ok(NoBidReason::InvalidInputs),
            "stale_input" => Ok(NoBidReason::StaleInput),
            "hard_constraints" => Ok(NoBidReason::HardConstraints),
            "policy_unavailable" => Ok(NoBidReason::PolicyUnavailable),
            "deadline_missed" => Ok(NoBidReason::DeadlineMissed),
            "timeout" => Ok(NoBidReason::Timeout),
            "operator_disabled" => Ok(NoBidReason::OperatorDisabled),
            "market_reject" => Ok(NoBidReason::MarketReject),
            other => Err(anyhow!("unknown no_bid_reason: {other}")),
        }
    }
}

impl From<ClearingResultSchema> for EwdsClearingResultDto {
    fn from(s: ClearingResultSchema) -> Self {
        EwdsClearingResultDto {
            market_id: s.market_id,
            clearing_status: s.clearing_status.as_wire().to_string(),
            no_bid_reason: s.no_bid_reason.map(|r| r.as_wire().to_string()),
            clearing_price: s.clearing_price,
            total_supply: s.total_supply,
            total_demand: s.total_demand,
            trade_quantity: s.traded_quantity,
            num_trades: s.num_trades,
            tx_hash: s.tx_hash,
            created_at: s.clearing_time,
        }
    }
}

impl TryFrom<EwdsClearingResultDto> for ClearingResultSchema {
    type Error = anyhow::Error;
    fn try_from(d: EwdsClearingResultDto) -> Result<Self> {
        Ok(ClearingResultSchema {
            market_id: d.market_id,
            clearing_status: ClearingStatus::from_str(&d.clearing_status)?,
            no_bid_reason: d
                .no_bid_reason
                .as_deref()
                .map(NoBidReason::from_str)
                .transpose()?,
            clearing_price: d.clearing_price,
            total_supply: d.total_supply,
            total_demand: d.total_demand,
            traded_quantity: d.trade_quantity,
            num_trades: d.num_trades,
            tx_hash: d.tx_hash,
            clearing_time: d.created_at,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EwdsMarketDto {
    pub market_id: String,
    pub community_id: String,
    pub opening_time: String,
    pub closing_time: String,
    pub delivery_start_time: String,
    pub delivery_end_time: String,
    pub market_type: MarketType,
    pub matching_algorithm: MatchingAlgorithm,
    pub created_at: String,
}

impl From<MarketSchema> for EwdsMarketDto {
    fn from(m: MarketSchema) -> Self {
        EwdsMarketDto {
            market_id: m.market_id,
            community_id: m.community_id,
            opening_time: m.opening_time,
            closing_time: m.closing_time,
            delivery_start_time: m.delivery_start_time,
            delivery_end_time: m.delivery_end_time,
            market_type: m.market_type,
            matching_algorithm: m.matching_algorithm,
            created_at: m.created_at,
        }
    }
}

impl From<EwdsMarketDto> for MarketSchema {
    fn from(d: EwdsMarketDto) -> Self {
        MarketSchema {
            market_id: d.market_id,
            community_id: d.community_id,
            opening_time: d.opening_time,
            closing_time: d.closing_time,
            delivery_start_time: d.delivery_start_time,
            delivery_end_time: d.delivery_end_time,
            market_type: d.market_type,
            matching_algorithm: d.matching_algorithm,
            created_at: d.created_at,
        }
    }
}

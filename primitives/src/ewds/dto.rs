use super::{EwdsEventType, EwdsOperation};
use crate::db_api_schema::{
    grid_topology::{EnergyCommunitySchema, FacilitySchema, SiteSchema},
    ids::IdMappingSchema,
    market::{MarketSchema, MarketType, MatchingAlgorithm},
    orders::{DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus},
    profiles::{MeasurementSchema, TimeseriesSchema},
    trades::{
        ClearingResultSchema, ClearingStatus, DbTradeSchema, NoBidReason, TradeParameters,
        TradeStatus,
    },
};

use crate::utils::{normalize_uuid, opt_rfc3339_to_datetime, rfc3339_to_datetime};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A message as the gateway returns it from `GET /api/v2/messages`. Only `payload` is
/// guaranteed; the metadata is optional so that a gateway which leaves it out still parses.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsInboundMessage {
    pub payload: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub topic_name: Option<String>,
    #[serde(default)]
    pub topic_owner: Option<String>,
    #[serde(default)]
    pub topic_version: Option<String>,
    #[serde(default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    /// When the sender published the message, in nanoseconds since the Unix epoch.
    #[serde(default)]
    pub timestamp_nanos: Option<u64>,
}

impl EwdsInboundMessage {
    /// How long before `now` the message was published, if the gateway returned its timestamp.
    /// A timestamp after `now`, e.g. from clock skew, counts as zero.
    pub fn age_at(&self, now: SystemTime) -> Option<Duration> {
        let published = UNIX_EPOCH + Duration::from_nanos(self.timestamp_nanos?);
        Some(now.duration_since(published).unwrap_or_default())
    }
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

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct EwdsSendMessageResponse {
    pub recipients: EwdsDeliverySummary,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct EwdsDeliverySummary {
    pub failed: u32,
    pub sent: u32,
    pub total: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsResponseEnvelope<T> {
    pub request_id: String,
    pub success: bool,
    pub data: Vec<T>,
    pub error: Option<EwdsErrorPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EwdsEventEnvelope<T> {
    pub event_id: String,
    pub event_type: EwdsEventType,
    pub occurred_at: String,
    pub data: T,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FacilityMarketValidationRequest {
    pub facility_id: String,
    pub market_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FacilityMarketValidationResult {
    pub facility_id: String,
    pub market_id: String,
    pub eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<FacilityMarketValidationReason>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FacilityMarketValidationReason {
    UnknownMarket,
    UnknownFacility,
    UnknownCommunity,
    UnknownSite,
    FacilityNotListedInSite,
    SiteNotInCommunity,
    #[serde(other)]
    Unknown,
}

impl FacilityMarketValidationReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownMarket => "unknown_market",
            Self::UnknownFacility => "unknown_facility",
            Self::UnknownCommunity => "unknown_community",
            Self::UnknownSite => "unknown_site",
            Self::FacilityNotListedInSite => "facility_not_listed_in_site",
            Self::SiteNotInCommunity => "site_not_in_community",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EwdsOrderDto {
    pub order_id: String,
    pub market_id: String,
    pub order_type: String,
    pub order_status: String,
    pub time_slot: String,
    pub quantity: f64,
    pub price_limit: f64,
    pub energy_source_preference: Option<String>,
    pub energy_type: Option<String>,
    pub created_by: String,
    pub creation_time: String,
    pub updated_at: Option<String>,
    pub reject_reason: Option<String>,
    pub preferred_trading_partner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_energy_rate: Option<f64>,
}

/// The wire form of a community, on EWDS and REST, in camelCase. `validFrom` and `validUntil` are RFC 3339
/// strings. A `validFrom` sent to GSY is ignored: GSY sets it to the time it receives the entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EwdsCommunityDto {
    pub community_id: String,
    pub community_name: String,
    pub sites: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EwdsSiteDto {
    pub site_id: String,
    pub site_name: String,
    pub site_description: String,
    pub facilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EwdsFacilityDto {
    pub facility_id: String,
    pub facility_name: String,
    pub site_id: String,
    pub owner_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
}

/// Parses the validity times of a topology entry. A missing `valid_from` is the oldest possible
/// time.
fn validate_times(
    valid_from: Option<String>,
    valid_until: Option<String>,
) -> Result<(DateTime<Utc>, Option<DateTime<Utc>>)> {
    let valid_from = match valid_from {
        Some(valid_from) => rfc3339_to_datetime(&valid_from).context("invalid valid_from")?,
        None => DateTime::UNIX_EPOCH,
    };
    let valid_until = valid_until
        .as_deref()
        .map(rfc3339_to_datetime)
        .transpose()
        .context("invalid valid_until")?;
    Ok((valid_from, valid_until))
}

impl From<EnergyCommunitySchema> for EwdsCommunityDto {
    fn from(community: EnergyCommunitySchema) -> Self {
        Self {
            community_id: community.community_id,
            community_name: community.community_name,
            sites: community.sites,
            valid_from: Some(community.valid_from.to_rfc3339()),
            valid_until: community.valid_until.map(|time| time.to_rfc3339()),
        }
    }
}

impl TryFrom<EwdsCommunityDto> for EnergyCommunitySchema {
    type Error = anyhow::Error;

    fn try_from(community: EwdsCommunityDto) -> Result<Self> {
        let (valid_from, valid_until) =
            validate_times(community.valid_from, community.valid_until)?;
        Ok(Self {
            community_id: community.community_id,
            community_name: community.community_name,
            sites: community.sites,
            valid_from,
            valid_until,
        })
    }
}

impl From<SiteSchema> for EwdsSiteDto {
    fn from(site: SiteSchema) -> Self {
        Self {
            site_id: site.site_id,
            site_name: site.site_name,
            site_description: site.site_description,
            facilities: site.facilities,
            valid_from: Some(site.valid_from.to_rfc3339()),
            valid_until: site.valid_until.map(|time| time.to_rfc3339()),
        }
    }
}

impl TryFrom<EwdsSiteDto> for SiteSchema {
    type Error = anyhow::Error;

    fn try_from(site: EwdsSiteDto) -> Result<Self> {
        let (valid_from, valid_until) = validate_times(site.valid_from, site.valid_until)?;
        Ok(Self {
            site_id: site.site_id,
            site_name: site.site_name,
            site_description: site.site_description,
            facilities: site.facilities,
            valid_from,
            valid_until,
        })
    }
}

impl From<FacilitySchema> for EwdsFacilityDto {
    fn from(facility: FacilitySchema) -> Self {
        Self {
            facility_id: facility.facility_id,
            facility_name: facility.facility_name,
            site_id: facility.site_id,
            owner_id: facility.owner_id,
            valid_from: Some(facility.valid_from.to_rfc3339()),
            valid_until: facility.valid_until.map(|time| time.to_rfc3339()),
        }
    }
}

impl TryFrom<EwdsFacilityDto> for FacilitySchema {
    type Error = anyhow::Error;

    fn try_from(facility: EwdsFacilityDto) -> Result<Self> {
        let (valid_from, valid_until) = validate_times(facility.valid_from, facility.valid_until)?;
        Ok(Self {
            facility_id: facility.facility_id,
            facility_name: facility.facility_name,
            site_id: facility.site_id,
            owner_id: facility.owner_id,
            valid_from,
            valid_until,
        })
    }
}

impl From<DbOrderSchema> for EwdsOrderDto {
    fn from(order: DbOrderSchema) -> Self {
        let preferred_trading_partner = order
            .requirements
            .as_ref()
            .and_then(|requirements| requirements.trading_partner_id.clone());

        Self {
            order_id: order.order_id,
            market_id: order.market_id,
            order_type: order_type_to_ewds(&order.order_type).to_string(),
            order_status: order_status_to_ewds(&order.status).to_string(),
            time_slot: order.time_slot.to_rfc3339(),
            quantity: order.energy_kWh,
            price_limit: order.energy_rate,
            energy_source_preference: order
                .requirements
                .as_ref()
                .and_then(|r| r.energy_type.as_ref())
                .map(|et| energy_type_to_ewds(et).to_string()),
            energy_type: order
                .attributes
                .as_ref()
                .map(|a| energy_type_to_ewds(&a.energy_type).to_string()),
            created_by: order.created_by,
            creation_time: order.creation_time.to_rfc3339(),
            updated_at: order.status_updated_at.map(|time| time.to_rfc3339()),
            reject_reason: None,
            preferred_trading_partner,
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
        let order_type = order_type_from_ewds(order.order_type.as_str())?;
        let requirements = if order.energy_source_preference.is_some()
            || order.preferred_energy_rate.is_some()
            || order.preferred_trading_partner.is_some()
        {
            Some(DbRequirements {
                trading_partner_id: order.preferred_trading_partner,
                energy_type: match order.energy_source_preference {
                    Some(ref pref) => Some(energy_type_from_ewds(pref)?),
                    None => None,
                },
                // Left empty when not sent: each order falls back to its own normal rate.
                preferred_energy_rate: order.preferred_energy_rate,
            })
        } else {
            None
        };

        let attributes = match order.energy_type {
            Some(ref energy_type) => Some(DbAttributes {
                energy_type: energy_type_from_ewds(energy_type)?,
            }),
            None => None,
        };

        Ok(Self {
            order_id: order.order_id,
            status: order_status_from_ewds(order.order_status.as_str())?,
            status_updated_at: opt_rfc3339_to_datetime(order.updated_at.as_deref())?,
            order_type,
            market_id: order.market_id,
            time_slot: rfc3339_to_datetime(&order.time_slot)?,
            creation_time: rfc3339_to_datetime(&order.creation_time)?,
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
    pub timestamp: String,
}

fn trade_status_to_ewds(status: &TradeStatus) -> &'static str {
    match status {
        TradeStatus::Matched => "matched",
        TradeStatus::Executed => "executed",
        TradeStatus::Settled => "settled",
        TradeStatus::Penalized => "penalized",
    }
}

fn trade_status_from_ewds(value: &str) -> Result<TradeStatus> {
    match value.to_ascii_lowercase().as_str() {
        "matched" => Ok(TradeStatus::Matched),
        "executed" => Ok(TradeStatus::Executed),
        "settled" => Ok(TradeStatus::Settled),
        "penalized" => Ok(TradeStatus::Penalized),
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
            timestamp: trade.creation_time.to_rfc3339(),
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
            creation_time: rfc3339_to_datetime(&trade.timestamp)?,
            offer_hash: trade.offer_id,
            bid_hash: trade.bid_id,
            residual_offer_id: trade.residual_offer_id,
            residual_bid_id: trade.residual_bid_id,
            parameters: TradeParameters {
                selected_energy_kWh: trade.trade_quantity,
                energy_rate: trade.trade_price,
            },
            status_updated_at: None,
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
    pub created_at: String,
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
            created_at: s.clearing_time.to_rfc3339(),
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
            clearing_time: rfc3339_to_datetime(&d.created_at)?,
        })
    }
}

/// A market in the form of `int.market.schema.v1.json`, as returned by
/// `markets.query` and published in `market.event`: UUID market ID and
/// RFC 3339 times. Market IDs use the same canonical UUID text in storage.
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

impl TryFrom<MarketSchema> for EwdsMarketDto {
    type Error = anyhow::Error;

    fn try_from(m: MarketSchema) -> Result<Self> {
        Ok(EwdsMarketDto {
            market_id: normalize_uuid(&m.market_id)?,
            community_id: m.community_id,
            opening_time: m.opening_time.to_rfc3339(),
            closing_time: m.closing_time.to_rfc3339(),
            delivery_start_time: m.delivery_start_time.to_rfc3339(),
            delivery_end_time: m.delivery_end_time.to_rfc3339(),
            market_type: m.market_type,
            matching_algorithm: m.matching_algorithm,
            created_at: m.created_at.to_rfc3339(),
        })
    }
}

impl TryFrom<EwdsMarketDto> for MarketSchema {
    type Error = anyhow::Error;

    fn try_from(d: EwdsMarketDto) -> Result<Self> {
        Ok(MarketSchema {
            market_id: normalize_uuid(&d.market_id)?,
            community_id: d.community_id,
            opening_time: rfc3339_to_datetime(&d.opening_time)?,
            closing_time: rfc3339_to_datetime(&d.closing_time)?,
            delivery_start_time: rfc3339_to_datetime(&d.delivery_start_time)?,
            delivery_end_time: rfc3339_to_datetime(&d.delivery_end_time)?,
            market_type: d.market_type,
            matching_algorithm: d.matching_algorithm,
            created_at: rfc3339_to_datetime(&d.created_at)?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EwdsMeasurementDto {
    pub facility_id: String,
    pub community_uuid: String,
    pub time_slot: String,
    pub creation_time: String,
    pub energy_kwh: f64,
}

impl From<MeasurementSchema> for EwdsMeasurementDto {
    fn from(schema: MeasurementSchema) -> Self {
        Self {
            facility_id: schema.facility_id,
            community_uuid: schema.community_uuid,
            time_slot: schema.time_slot.to_rfc3339(),
            creation_time: schema.creation_time.to_rfc3339(),
            energy_kwh: schema.energy_kwh,
        }
    }
}

impl TryFrom<EwdsMeasurementDto> for MeasurementSchema {
    type Error = anyhow::Error;

    fn try_from(measurement: EwdsMeasurementDto) -> Result<Self> {
        Ok(Self {
            facility_id: measurement.facility_id,
            community_uuid: measurement.community_uuid,
            time_slot: rfc3339_to_datetime(&measurement.time_slot)?,
            creation_time: rfc3339_to_datetime(&measurement.creation_time)?,
            energy_kwh: measurement.energy_kwh,
        })
    }
}

/// A timeseries value on the HTTP API; `timestamp` is RFC 3339.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TimeseriesDto {
    pub measurement_point: String,
    pub timestamp: String,
    pub value: f64,
}

impl From<TimeseriesSchema> for TimeseriesDto {
    fn from(value: TimeseriesSchema) -> Self {
        Self {
            measurement_point: value.measurement_point,
            timestamp: value.timestamp.to_rfc3339(),
            value: value.value,
        }
    }
}

impl TryFrom<TimeseriesDto> for TimeseriesSchema {
    type Error = anyhow::Error;

    fn try_from(value: TimeseriesDto) -> Result<Self> {
        Ok(Self {
            measurement_point: value.measurement_point,
            timestamp: rfc3339_to_datetime(&value.timestamp)?,
            value: value.value,
        })
    }
}

/// An off-chain → on-chain ID mapping as returned by `POST /ids` and `ids.query`;
/// `creation_time` is RFC 3339.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IdMappingDto {
    pub offchain_id: String,
    pub onchain_id: String,
    pub creation_time: String,
}

impl From<IdMappingSchema> for IdMappingDto {
    fn from(mapping: IdMappingSchema) -> Self {
        Self {
            offchain_id: mapping.offchain_id,
            onchain_id: mapping.onchain_id,
            creation_time: mapping.creation_time.to_rfc3339(),
        }
    }
}

impl TryFrom<IdMappingDto> for IdMappingSchema {
    type Error = anyhow::Error;

    fn try_from(mapping: IdMappingDto) -> Result<Self> {
        Ok(Self {
            offchain_id: mapping.offchain_id,
            onchain_id: mapping.onchain_id,
            creation_time: rfc3339_to_datetime(&mapping.creation_time)?,
        })
    }
}

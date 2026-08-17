//! Pure, synchronous derivation of `LocalOriginRecord`s from `Executed` trades, the
//! community topology and measurements. No I/O; §7 of the plan is the field-by-field
//! source of truth. A trade that cannot yield a valid record is skipped with one log
//! line — never a panic, never a partial record.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};

use crate::certificates::config::PILOT;
use crate::certificates::schema::{
    AssetClass, AttributeProvenance, BeneficiaryAndClaim, ConsumptionAsset, DataCompleteness,
    DataRecordClass, DeliveryScope, EnergyUnit, FlowDirection, LocalOriginRecord, MeasurementProvenance,
    ProductionAsset, RecordIdentity, RecordLocation, RecordTimeAndQuantity, RecordType, SourceOfRecord,
    TradeAndDeliveryReference, TradeStatusAtIssuance,
};

/// Every market's `community_areas` collapsed into one entry per `area_hash`, safe
/// because `area_hash` is `deterministic_area_hash(community_name, area_name)` and
/// therefore market-invariant.
pub(crate) struct Topology<'a> {
    area_by_hash: HashMap<&'a str, (&'a AreaTopologySchema, &'a str)>,
    community_by_id_hash: HashMap<String, &'a str>,
}

impl<'a> Topology<'a> {
    pub(crate) fn build(markets: &'a [MarketTopologySchema]) -> Self {
        let mut area_by_hash = HashMap::new();
        let mut community_by_id_hash = HashMap::new();
        for market in markets {
            community_by_id_hash
                .entry(h256_to_string(community_id_from_uuid(&market.community_uuid)))
                .or_insert(market.community_name.as_str());
            for area in &market.community_areas {
                area_by_hash
                    .entry(area.area_hash.as_str())
                    .or_insert((area, market.community_name.as_str()));
            }
        }
        Topology { area_by_hash, community_by_id_hash }
    }
}

/// Keyed by `(area_hash, time_slot)` — the same join the execution engine uses.
pub(crate) fn measurement_index(
    measurements: &[MeasurementSchema],
) -> HashMap<(&str, u64), &MeasurementSchema> {
    measurements
        .iter()
        .map(|measurement| ((measurement.area_hash.as_str(), measurement.time_slot), measurement))
        .collect()
}

/// `EV`, `BOILER` and `AREA` have no corresponding `AssetClass` and are never emitted.
pub(crate) fn asset_class_of(area_type: &AssetType) -> Option<AssetClass> {
    match area_type {
        AssetType::PV => Some(AssetClass::Pv),
        AssetType::BATTERY => Some(AssetClass::Battery),
        AssetType::HEAT_PUMP => Some(AssetClass::HeatPump),
        AssetType::SMART_METER | AssetType::GRID_METER => Some(AssetClass::MeteringPoint),
        AssetType::EV | AssetType::BOILER | AssetType::AREA => None,
    }
}

/// `None` when `time_slot` is not a representable unix timestamp, or when the interval
/// end overflows. A stored `time_slot` is always sane, so this is unreachable in
/// practice — but the caller skips the trade rather than panicking, because a read-side
/// projection must not fail a whole request for one bad row (§5).
pub fn interval_bounds_utc(time_slot: u64, duration_s: u64) -> Option<(String, String)> {
    let start = DateTime::<Utc>::from_timestamp(i64::try_from(time_slot).ok()?, 0)?;
    let end_secs = i64::try_from(time_slot.checked_add(duration_s)?).ok()?;
    let end = DateTime::<Utc>::from_timestamp(end_secs, 0)?;
    Some((start.to_rfc3339(), end.to_rfc3339()))
}

/// `f64::round()` is half-away-from-zero, which is half-up for the non-negative
/// quantities certified here.
pub fn round_half_up_2dp(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Names the execution cycle that promoted the trade: `exec:<date>:slot<n>`, where
/// `n = (time_slot mod 86400) / 900` — the fifteen-minute slot of the day, UTC.
/// `None` on an unrepresentable `time_slot`, for the same reason as
/// [`interval_bounds_utc`].
pub fn delivery_verification_reference(time_slot: u64) -> Option<String> {
    let date = DateTime::<Utc>::from_timestamp(i64::try_from(time_slot).ok()?, 0)?;
    let slot = (time_slot % 86400) / 900;
    Some(format!("exec:{}:slot{}", date.format("%Y-%m-%d"), slot))
}

pub fn build_local_origin_records(
    trades: Vec<TradeSchema>,
    markets: &[MarketTopologySchema],
    measurements: &[MeasurementSchema],
) -> Vec<LocalOriginRecord> {
    let topology = Topology::build(markets);
    let measurements_by_area_slot = measurement_index(measurements);
    let mut records = Vec::with_capacity(trades.len());

    for trade in trades {
        // `trade_and_delivery.trade_status_at_issuance` is unconditionally
        // `delivery_verified` (§1, rule 11); the route already filters to `Executed`
        // trades, but the builder re-checks so it stays correct if ever called with an
        // unfiltered set.
        if trade.status != TradeStatus::Executed {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                status = ?trade.status,
                "skipping trade: not Executed"
            );
            continue;
        }

        let seller_hash = trade.offer.offer_component.area_uuid.as_str();
        let Some(&(seller_area, seller_community)) = topology.area_by_hash.get(seller_hash) else {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                seller_area_hash = seller_hash,
                "skipping trade: seller area not found in any market topology"
            );
            continue;
        };

        if !matches!(asset_class_of(&seller_area.area_type), Some(AssetClass::Pv)) {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                "skipping trade: seller asset class is not PV"
            );
            continue;
        }

        if trade.time_slot % PILOT.interval_duration_s != 0 {
            tracing::warn!(
                trade_uuid = %trade.trade_uuid,
                time_slot = trade.time_slot,
                "skipping trade: time_slot is not on an interval boundary"
            );
            continue;
        }

        if trade.parameters.selected_energy <= 0.0 {
            tracing::warn!(
                trade_uuid = %trade.trade_uuid,
                selected_energy = trade.parameters.selected_energy,
                "skipping trade: selected_energy is not positive"
            );
            continue;
        }

        let Some(&production_measurement) =
            measurements_by_area_slot.get(&(seller_hash, trade.time_slot))
        else {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                seller_area_hash = seller_hash,
                time_slot = trade.time_slot,
                "skipping trade: no production measurement for seller area at slot"
            );
            continue;
        };

        let buyer_hash = trade.bid.bid_component.area_uuid.as_str();
        let (consumption_asset_id, community_id_consumption, consumption_metering_point_id, consumption_asset_class) =
            match topology.area_by_hash.get(buyer_hash) {
                Some(&(buyer_area, buyer_community)) => (
                    buyer_area.name.clone(),
                    Some(buyer_community),
                    Some(buyer_area.area_uuid.clone()),
                    asset_class_of(&buyer_area.area_type).unwrap_or(AssetClass::MeteringPoint),
                ),
                None => match topology.community_by_id_hash.get(buyer_hash) {
                    Some(&buyer_community) => {
                        (buyer_community.to_string(), Some(buyer_community), None, AssetClass::MeteringPoint)
                    }
                    None => (buyer_hash.to_string(), None, None, AssetClass::MeteringPoint),
                },
            };

        let delivery_scope = match community_id_consumption {
            Some(community) if community == seller_community => DeliveryScope::IntraCommunity,
            Some(_) => DeliveryScope::InterCommunity,
            None => DeliveryScope::SupplierOfftake,
        };

        let (Some((interval_start, interval_end)), Some(delivery_reference)) = (
            interval_bounds_utc(trade.time_slot, PILOT.interval_duration_s),
            delivery_verification_reference(trade.time_slot),
        ) else {
            tracing::warn!(
                trade_uuid = %trade.trade_uuid,
                time_slot = trade.time_slot,
                "skipping trade: time_slot is not a representable timestamp"
            );
            continue;
        };

        let flow_direction = if production_measurement.energy_kwh < 0.0 {
            FlowDirection::Export
        } else {
            FlowDirection::Import
        };

        let measurement_recorded_at = production_measurement.creation_time.max(
            trade.status_updated_at.unwrap_or(trade.creation_time),
        );

        records.push(LocalOriginRecord {
            identity: RecordIdentity {
                record_type: RecordType::LocalOriginRecord,
                site_id: PILOT.site_id.clone(),
            },
            time_and_quantity: RecordTimeAndQuantity {
                interval_start,
                interval_end,
                interval_duration_s: PILOT.interval_duration_s,
                source_slot_timestamp: trade.time_slot,
                energy_quantity: round_half_up_2dp(trade.parameters.selected_energy),
                energy_unit: EnergyUnit::KWh,
                rounding_rule: PILOT.rounding_rule.clone(),
                loss_adjustment: None,
            },
            production_asset: ProductionAsset {
                production_asset_id: seller_area.name.clone(),
                asset_registry_reference: None,
                metering_point_id: None,
                asset_class: AssetClass::Pv,
                rated_power: None,
            },
            consumption_asset: ConsumptionAsset {
                consumption_asset_id,
                asset_registry_reference: None,
                metering_point_id: None,
                asset_class: consumption_asset_class,
            },
            location: RecordLocation {
                municipality_code: PILOT.municipality_code.clone(),
                grid_operator_id: PILOT.grid_operator_id.clone(),
                grid_level: PILOT.grid_level,
                community_id_origin: Some(seller_community.to_string()),
                community_id_consumption: community_id_consumption.map(|community| community.to_string()),
                delivery_scope,
            },
            measurement_provenance: MeasurementProvenance {
                measurement_id: format!(
                    "{}:{}:e_generation",
                    seller_community, production_measurement.area_uuid
                ),
                measuring_sensor_id: production_measurement.area_uuid.clone(),
                property_measured: PILOT.property_measured.clone(),
                flow_direction,
                data_provider_id: PILOT.data_provider_id.clone(),
                data_completeness: DataCompleteness::Complete,
                source_of_record: SourceOfRecord::Platform,
                data_record_class: DataRecordClass::Measurement,
                measurement_recorded_at,
            },
            attribute_provenance: AttributeProvenance {
                support_scheme_status: None,
                storage_mediated_flag: false,
            },
            beneficiary_and_claim: BeneficiaryAndClaim {
                owner_id: seller_area.area_uuid.clone(),
                consumption_metering_point_id,
                facility_id: None,
            },
            trade_and_delivery: TradeAndDeliveryReference {
                trade_reference: vec![trade.trade_uuid.clone()],
                trade_hash: vec![trade.trade_uuid],
                trade_status_at_issuance: Some(TradeStatusAtIssuance::DeliveryVerified),
                delivery_verification_reference: Some(delivery_reference),
            },
        });
    }

    records
}

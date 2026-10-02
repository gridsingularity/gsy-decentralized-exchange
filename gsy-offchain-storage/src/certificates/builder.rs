//! Pure, synchronous derivation of `LocalOriginRecord`s from `Executed` trades, the grid
//! topology and measurements. No I/O: the caller fetches every input (see
//! [`crate::certificates::query`]). A trade that cannot yield a valid record is skipped with
//! one log line — never a panic, never a partial record.
//!
//! # Identification
//!
//! * A trade party (`seller` / `buyer`) is the on-chain owner hash
//!   `bytes16_to_hex(create_encrypted_bytes16_from_string(owner_id))`, resolved to the
//!   owner's facility (one facility per owner, DD-409).
//! * A measurement point's `asset_name` names a **facility**. It is matched against
//!   `FacilitySchema::facility_id` first — that is what the community client and the
//!   `/measurements` compatibility route write into `asset_name`, and what the execution
//!   engine's `facility_owner_mapping` keys on — and against `facility_name` as a fallback.
//! * An asset's `facility_name` is matched the same way (id first, then name), because
//!   ontology imports populate it with the facility's name while other writers use the id.
//!
//! # Sign convention of measurement values
//!
//! A point's contribution to the facility's net export at a slot is:
//! * a **negative** value is always an export of `|value|` — the legacy writers (community
//!   client, `/measurements`) store one signed value per facility, negative for export, and
//!   derive the point's `direction` from that sign;
//! * a **non-negative** value is a magnitude in the point's `direction`: `Export` adds it,
//!   `Import` subtracts it.
//!
//! Net export = Σexport − Σimport over the facility's `Measurement` points at the slot.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use primitives::certificates::{
    AssetClass, AttributeProvenance, BeneficiaryAndClaim, ConsumptionAsset, DataCompleteness,
    DataRecordClass, DeliveryScope, EnergyUnit, FlowDirection, LocalOriginRecord,
    MeasurementProvenance, ProductionAsset, RecordIdentity, RecordLocation, RecordTimeAndQuantity,
    RecordType, SourceOfRecord, TradeAndDeliveryReference, TradeStatusAtIssuance,
};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, AssetType, EnergyCommunitySchema, FacilitySchema,
};
use primitives::db_api_schema::profiles::{
    FlowDirection as PointDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeStatus};
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};

use crate::certificates::config::PILOT;

/// Everything the builder reads besides the trades, already fetched by the caller.
#[derive(Debug, Default, Clone, Copy)]
pub struct CertificateInputs<'a> {
    pub facilities: &'a [FacilitySchema],
    pub communities: &'a [EnergyCommunitySchema],
    pub assets: &'a [AssetSchema],
    pub measurement_points: &'a [MeasurementPointSchema],
    pub timeseries: &'a [TimeseriesSchema],
}

/// The on-chain party hash of an off-chain id, as written into `DbTradeSchema::seller` /
/// `buyer` (and, for a community acting as a party, its `community_id`).
pub fn party_hash(offchain_id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(offchain_id))
}

/// Facilities and communities indexed the ways the builder looks them up.
struct Topology<'a> {
    /// Owner hash → facility (DD-409: one facility per owner; the lowest `facility_id` wins
    /// should that ever be violated, so the choice is deterministic).
    facility_by_party: HashMap<String, &'a FacilitySchema>,
    /// `facility_id` and `facility_name` → facility; ids take precedence over names.
    facility_by_alias: HashMap<&'a str, &'a FacilitySchema>,
    /// `site_id` → community whose `sites` contains it.
    community_by_site: HashMap<&'a str, &'a EnergyCommunitySchema>,
    /// Community party hash → community, for a buyer that is a community rather than a facility.
    community_by_party: HashMap<String, &'a EnergyCommunitySchema>,
    /// `facility_id`s owning at least one `AssetType::PV` asset.
    pv_facilities: HashSet<&'a str>,
}

impl<'a> Topology<'a> {
    fn build(inputs: &CertificateInputs<'a>) -> Self {
        let mut facilities: Vec<&FacilitySchema> = inputs.facilities.iter().collect();
        facilities.sort_by(|a, b| a.facility_id.cmp(&b.facility_id));

        let mut facility_by_party = HashMap::new();
        let mut facility_by_alias = HashMap::new();
        for facility in &facilities {
            facility_by_party
                .entry(party_hash(&facility.owner_id))
                .or_insert(*facility);
            facility_by_alias
                .entry(facility.facility_id.as_str())
                .or_insert(*facility);
        }
        for facility in &facilities {
            facility_by_alias
                .entry(facility.facility_name.as_str())
                .or_insert(*facility);
        }

        let mut communities: Vec<&EnergyCommunitySchema> = inputs.communities.iter().collect();
        communities.sort_by(|a, b| a.community_id.cmp(&b.community_id));
        let mut community_by_site = HashMap::new();
        let mut community_by_party = HashMap::new();
        for community in communities {
            community_by_party
                .entry(party_hash(&community.community_id))
                .or_insert(community);
            for site in &community.sites {
                community_by_site.entry(site.as_str()).or_insert(community);
            }
        }

        let pv_facilities = inputs
            .assets
            .iter()
            .filter(|asset| asset.asset_type == AssetType::PV)
            .filter_map(|asset| facility_by_alias.get(asset.facility_name.as_str()))
            .map(|facility| facility.facility_id.as_str())
            .collect();

        Topology {
            facility_by_party,
            facility_by_alias,
            community_by_site,
            community_by_party,
            pv_facilities,
        }
    }

    fn community_of(&self, facility: &FacilitySchema) -> Option<&'a EnergyCommunitySchema> {
        self.community_by_site
            .get(facility.site_id.as_str())
            .copied()
    }
}

/// One facility's measurement evidence at one slot.
#[derive(Debug)]
struct SlotEvidence<'a> {
    export_kwh: f64,
    import_kwh: f64,
    /// The point contributing the largest export, with that export and its timeseries
    /// timestamp (lowest `measurement_id` on ties).
    export_point: Option<(&'a MeasurementPointSchema, f64, u64)>,
}

impl SlotEvidence<'_> {
    fn net_export_kwh(&self) -> f64 {
        self.export_kwh - self.import_kwh
    }
}

/// `(export, import)` contribution of one value; see the module docs for the convention.
fn split_flow(direction: &PointDirection, value: f64) -> (f64, f64) {
    if value < 0.0 {
        return (-value, 0.0);
    }
    match direction {
        PointDirection::Export => (value, 0.0),
        PointDirection::Import => (0.0, value),
    }
}

/// Keyed by `(facility_id, time_slot)`. Only `Measurement` points whose `asset_name`
/// resolves to a facility, and only timeseries values with a numeric timestamp, count.
fn map_measurements<'a>(
    topology: &Topology<'a>,
    inputs: &CertificateInputs<'a>,
) -> HashMap<(&'a str, u64), SlotEvidence<'a>> {
    let points: HashMap<&str, (&MeasurementPointSchema, &FacilitySchema)> = inputs
        .measurement_points
        .iter()
        .filter(|point| point.point_type == MeasurementPointType::Measurement)
        .filter_map(|point| {
            let facility = topology.facility_by_alias.get(point.asset_name.as_str())?;
            Some((point.measurement_id.as_str(), (point, *facility)))
        })
        .collect();

    let mut seen: HashSet<(&str, u64)> = HashSet::new();
    let mut index: HashMap<(&str, u64), SlotEvidence> = HashMap::new();
    for value in inputs.timeseries {
        let Some(&(point, facility)) = points.get(value.measurement_point.as_str()) else {
            continue;
        };
        let Ok(timestamp) = value.timestamp.trim().parse::<u64>() else {
            continue;
        };
        // (measurement_point, timestamp) is unique in storage; guard anyway so a duplicate
        // in the input can never double-count.
        if !seen.insert((point.measurement_id.as_str(), timestamp)) {
            continue;
        }
        let (export, import) = split_flow(&point.direction, value.value);
        let evidence = index
            .entry((facility.facility_id.as_str(), timestamp))
            .or_insert(SlotEvidence {
                export_kwh: 0.0,
                import_kwh: 0.0,
                export_point: None,
            });
        evidence.export_kwh += export;
        evidence.import_kwh += import;
        if export > 0.0 {
            let better = match evidence.export_point {
                None => true,
                Some((current, current_export, _)) => {
                    export > current_export
                        || (export == current_export
                            && point.measurement_id < current.measurement_id)
                }
            };
            if better {
                evidence.export_point = Some((point, export, timestamp));
            }
        }
    }
    index
}

/// Slack when comparing a sale against the remaining net export, absorbing float error
/// from summing kWh values.
const ALLOCATION_TOLERANCE_KWH: f64 = 1e-9;

/// Net-export allocation, keyed by `(seller facility_id, time_slot, trade_uuid)`.
///
/// Per seller facility and slot with a positive net export `E`, `E` is consumed by that
/// facility's `Executed` sales of the slot in `(creation_time, trade_uuid)` order. A sale
/// that fits in what is left of `E` is allocated; the first one that does not fit ends the
/// walk, so no later sale is ever allocated from a partial remainder (strict time priority).
fn net_export_allocation<'a>(
    topology: &Topology<'a>,
    evidence: &HashMap<(&'a str, u64), SlotEvidence<'a>>,
    slot_executed: &'a [DbTradeSchema],
) -> HashSet<(&'a str, u64, &'a str)> {
    let mut sales_by_facility_slot: HashMap<(&str, u64), Vec<&DbTradeSchema>> = HashMap::new();
    // A duplicated `trade_uuid` must not consume the net export twice; the first copy wins.
    let mut seen_uuids: HashSet<&str> = HashSet::new();
    for trade in slot_executed
        .iter()
        .filter(|trade| trade.status == TradeStatus::Executed)
    {
        if !seen_uuids.insert(trade.trade_uuid.as_str()) {
            continue;
        }
        let Some(facility) = topology.facility_by_party.get(trade.seller.as_str()) else {
            continue;
        };
        sales_by_facility_slot
            .entry((facility.facility_id.as_str(), trade.time_slot))
            .or_default()
            .push(trade);
    }

    let mut allocated = HashSet::new();
    for ((facility_id, time_slot), mut sales) in sales_by_facility_slot {
        let Some(slot_evidence) = evidence.get(&(facility_id, time_slot)) else {
            continue;
        };
        sales.sort_by(|a, b| {
            (a.creation_time, a.trade_uuid.as_str()).cmp(&(b.creation_time, b.trade_uuid.as_str()))
        });

        let mut remaining = slot_evidence.net_export_kwh().max(0.0);
        for sale in sales {
            let energy = sale.parameters.selected_energy_kWh.max(0.0);
            if energy > remaining + ALLOCATION_TOLERANCE_KWH {
                break;
            }
            remaining -= energy;
            allocated.insert((facility_id, time_slot, sale.trade_uuid.as_str()));
        }
    }
    allocated
}

/// `None` when `time_slot` is not a representable unix timestamp, or when the interval
/// end overflows. A stored `time_slot` is always sane, so this is unreachable in
/// practice — but the caller skips the trade rather than panicking, because a read-side
/// projection must not fail a whole request for one bad row.
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
/// `n = (time_slot mod 86400) / duration_s` — the interval of the day, UTC.
/// `None` on an unrepresentable `time_slot`, for the same reason as
/// [`interval_bounds_utc`]. `duration_s` must be non-zero; the pilot config guarantees it.
pub fn delivery_verification_reference(time_slot: u64, duration_s: u64) -> Option<String> {
    let date = DateTime::<Utc>::from_timestamp(i64::try_from(time_slot).ok()?, 0)?;
    let slot = (time_slot % 86400) / duration_s;
    Some(format!("exec:{}:slot{}", date.format("%Y-%m-%d"), slot))
}

/// [`build_local_origin_records_with_allocation`] with `trades` also serving as the set the
/// net-export allocation runs over. Exact when `trades` holds every `Executed` trade of
/// its slots.
pub fn build_local_origin_records(
    trades: Vec<DbTradeSchema>,
    inputs: &CertificateInputs,
) -> Vec<LocalOriginRecord> {
    let slot_executed = trades.clone();
    build_local_origin_records_with_allocation(trades, &slot_executed, inputs)
}

/// Emits records for `selected` only. `slot_executed` must hold every `Executed` trade of
/// the slots `selected` covers: a facility's net-export allocation runs over all of them, so
/// whether a trade is certified does not depend on which others were selected.
pub fn build_local_origin_records_with_allocation(
    selected_trades: Vec<DbTradeSchema>,
    trades_for_selected_slots: &[DbTradeSchema],
    inputs: &CertificateInputs,
) -> Vec<LocalOriginRecord> {
    let topology = Topology::build(inputs);
    let measurements = map_measurements(&topology, inputs);
    let facility_net_export_per_slot =
        net_export_allocation(&topology, &measurements, trades_for_selected_slots);
    let mut records = Vec::with_capacity(selected_trades.len());

    for trade in selected_trades {
        // `trade_status_at_issuance` is unconditionally `delivery_verified`; the query
        // already filters to `Executed` trades, but the builder re-checks so it stays
        // correct if ever called with an unfiltered set.
        if trade.status != TradeStatus::Executed {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                status = ?trade.status,
                "skipping trade: not Executed"
            );
            continue;
        }

        let Some(&seller_facility) = topology.facility_by_party.get(trade.seller.as_str()) else {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                seller = %trade.seller,
                "skipping trade: seller does not resolve to a facility"
            );
            continue;
        };
        let seller_facility_id = seller_facility.facility_id.as_str();

        if !topology.pv_facilities.contains(seller_facility_id) {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                facility_id = seller_facility_id,
                "skipping trade: seller facility has no PV asset"
            );
            continue;
        }

        if trade.time_slot % PILOT.interval_duration_s != 0 {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                time_slot = trade.time_slot,
                "skipping trade: time_slot is not on an interval boundary"
            );
            continue;
        }

        let energy = trade.parameters.selected_energy_kWh;
        if energy.is_nan() || energy <= 0.0 {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                selected_energy_kWh = energy,
                "skipping trade: selected energy is not positive"
            );
            continue;
        }

        let Some(slot_evidence) = measurements.get(&(seller_facility_id, trade.time_slot)) else {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                facility_id = seller_facility_id,
                time_slot = trade.time_slot,
                "skipping trade: no measurement for seller facility at slot"
            );
            continue;
        };

        let net_export_kwh = slot_evidence.net_export_kwh();
        let Some((export_point, _, measurement_timestamp)) =
            slot_evidence.export_point.filter(|_| net_export_kwh > 0.0)
        else {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                facility_id = seller_facility_id,
                net_export_kwh,
                time_slot = trade.time_slot,
                "skipping trade: seller facility is not net exporting at slot"
            );
            continue;
        };

        if !facility_net_export_per_slot.contains(&(
            seller_facility_id,
            trade.time_slot,
            trade.trade_uuid.as_str(),
        )) {
            tracing::info!(
                trade_uuid = %trade.trade_uuid,
                facility_id = seller_facility_id,
                net_export_kwh,
                time_slot = trade.time_slot,
                "skipping trade: sale not covered by the facility's net export"
            );
            continue;
        }

        let (Some((interval_start, interval_end)), Some(delivery_reference)) = (
            interval_bounds_utc(trade.time_slot, PILOT.interval_duration_s),
            delivery_verification_reference(trade.time_slot, PILOT.interval_duration_s),
        ) else {
            tracing::warn!(
                trade_uuid = %trade.trade_uuid,
                time_slot = trade.time_slot,
                "skipping trade: time_slot is not a representable timestamp"
            );
            continue;
        };

        let seller_community = topology.community_of(seller_facility);
        let buyer_facility = topology
            .facility_by_party
            .get(trade.buyer.as_str())
            .copied();
        let (consumption_asset_id, buyer_community) = match buyer_facility {
            Some(facility) => (
                facility.facility_id.clone(),
                topology.community_of(facility),
            ),
            None => match topology.community_by_party.get(trade.buyer.as_str()) {
                Some(&community) => (community.community_id.clone(), Some(community)),
                None => (trade.buyer.clone(), None),
            },
        };

        let delivery_scope = match (buyer_community, seller_community) {
            (Some(buyer), Some(seller)) if buyer.community_id == seller.community_id => {
                DeliveryScope::IntraCommunity
            }
            (Some(_), _) => DeliveryScope::InterCommunity,
            (None, _) => DeliveryScope::SupplierOfftake,
        };

        let property_measured = if export_point.property_measured.trim().is_empty() {
            PILOT.property_measured.clone()
        } else {
            export_point.property_measured.clone()
        };

        records.push(LocalOriginRecord {
            identity: RecordIdentity {
                record_type: RecordType::LocalOriginRecord,
                // The seller facility's site, the one `community_id_origin` resolves through.
                site_id: seller_facility.site_id.clone(),
            },
            time_and_quantity: RecordTimeAndQuantity {
                interval_start,
                interval_end,
                interval_duration_s: PILOT.interval_duration_s,
                source_slot_timestamp: trade.time_slot,
                energy_quantity: round_half_up_2dp(energy),
                energy_unit: EnergyUnit::KWh,
                rounding_rule: PILOT.rounding_rule.clone(),
                loss_adjustment: None,
            },
            production_asset: ProductionAsset {
                production_asset_id: seller_facility.facility_id.clone(),
                asset_registry_reference: None,
                metering_point_id: Some(seller_facility.facility_id.clone()),
                asset_class: AssetClass::Pv,
                rated_power: None,
            },
            consumption_asset: ConsumptionAsset {
                consumption_asset_id,
                asset_registry_reference: None,
                metering_point_id: None,
                asset_class: AssetClass::MeteringPoint,
            },
            location: RecordLocation {
                municipality_code: PILOT.municipality_code.clone(),
                grid_operator_id: PILOT.grid_operator_id.clone(),
                grid_level: PILOT.grid_level,
                community_id_origin: seller_community
                    .map(|community| community.community_id.clone()),
                community_id_consumption: buyer_community
                    .map(|community| community.community_id.clone()),
                delivery_scope,
            },
            measurement_provenance: MeasurementProvenance {
                measurement_id: export_point.measurement_id.clone(),
                measuring_sensor_id: seller_facility.facility_id.clone(),
                property_measured,
                flow_direction: FlowDirection::Export,
                data_provider_id: PILOT.data_provider_id.clone(),
                data_completeness: DataCompleteness::Complete,
                source_of_record: SourceOfRecord::Platform,
                data_record_class: DataRecordClass::Measurement,
                // v2 timeseries carry no separate arrival time, so this is the value's
                // timestamp. The query windows on the trade's `status_updated_at`, not on this.
                measurement_recorded_at: measurement_timestamp,
            },
            attribute_provenance: AttributeProvenance {
                support_scheme_status: None,
                storage_mediated_flag: false,
            },
            beneficiary_and_claim: BeneficiaryAndClaim {
                owner_id: trade.seller.clone(),
                consumption_metering_point_id: buyer_facility
                    .map(|facility| facility.facility_id.clone()),
                facility_id: Some(seller_facility.facility_id.clone()),
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

/// Deterministic order so repeated or adjacent queries return a stable sequence.
pub fn sort_records(records: &mut [LocalOriginRecord]) {
    records.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
}

fn sort_key(record: &LocalOriginRecord) -> (u64, u64, &str, &str) {
    (
        record.measurement_provenance.measurement_recorded_at,
        record.time_and_quantity.source_slot_timestamp,
        record.production_asset.production_asset_id.as_str(),
        record
            .trade_and_delivery
            .trade_reference
            .first()
            .map(String::as_str)
            .unwrap_or_default(),
    )
}

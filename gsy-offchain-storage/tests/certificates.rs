//! Builder cases for `local_origin_record`s, ported from fedecom to v2 fixtures.
//! Pure: no database. `GOO_*` env vars must stay unset so the pilot defaults apply.

use std::collections::HashSet;

use gsy_offchain_storage::certificates::builder::{
    build_local_origin_records, build_local_origin_records_with_allocation,
    delivery_verification_reference, interval_bounds_utc, round_half_up_2dp, sort_records,
    CertificateInputs,
};
use primitives::certificates::{
    AssetClass, DeliveryScope, FlowDirection, LocalOriginRecord, TradeStatusAtIssuance,
};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, AssetType, EnergyCommunitySchema, FacilitySchema,
};
use primitives::db_api_schema::profiles::{
    FlowDirection as PointDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeParameters, TradeStatus};
use primitives::utils::{
    bytes16_to_hex, create_encrypted_bytes16_from_string, timestamp_to_string_with_padding,
};

const SLOT: u64 = 1_778_754_600; // 2026-05-14T10:30:00Z, on a 900s boundary

const COMMUNITY: &str = "community-1";
const OTHER_COMMUNITY: &str = "community-2";

const SELLER_FACILITY: &str = "fac-seller";
const SELLER_FACILITY_NAME: &str = "Seller House";
const SELLER_OWNER: &str = "owner-seller";
const BUYER_FACILITY: &str = "fac-buyer";
const BUYER_OWNER: &str = "owner-buyer";
const REMOTE_FACILITY: &str = "fac-remote";
const REMOTE_OWNER: &str = "owner-remote";

const EXPORT_POINT: &str = "Measurement:community-1:fac-seller:export";
const IMPORT_POINT: &str = "Measurement:community-1:fac-seller:import";

fn hash(offchain_id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(offchain_id))
}

fn facility(
    facility_id: &str,
    facility_name: &str,
    site_id: &str,
    owner_id: &str,
) -> FacilitySchema {
    FacilitySchema {
        facility_id: facility_id.to_string(),
        facility_name: facility_name.to_string(),
        site_id: site_id.to_string(),
        owner_id: owner_id.to_string(),
    }
}

fn community(community_id: &str, sites: &[&str]) -> EnergyCommunitySchema {
    EnergyCommunitySchema {
        community_id: community_id.to_string(),
        community_name: format!("{community_id}-name"),
        sites: sites.iter().map(|site| site.to_string()).collect(),
    }
}

fn asset(uuid: &str, asset_type: AssetType, facility_name: &str) -> AssetSchema {
    AssetSchema {
        asset_type,
        uuid: uuid.to_string(),
        asset_name: uuid.to_string(),
        facility_name: facility_name.to_string(),
        creation_time: 1,
        installed_power: Some(10.0),
        asset_subtype: None,
        technology_type: None,
        phase_connection: None,
        energy_capacity: None,
        maximum_soc: None,
        minimum_soc: None,
        roundtrip_efficiency: None,
        target_service: None,
        grid_connection_type: None,
        max_rated_current: None,
        has_smart_meter: None,
        tariff_name: None,
    }
}

fn point(
    measurement_id: &str,
    asset_name: &str,
    direction: PointDirection,
    point_type: MeasurementPointType,
) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type,
        measurement_id: measurement_id.to_string(),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction,
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: asset_name.to_string(),
        datasource_name: Some(COMMUNITY.to_string()),
    }
}

fn value(measurement_point: &str, time_slot: u64, value: f64) -> TimeseriesSchema {
    TimeseriesSchema {
        measurement_point: measurement_point.to_string(),
        timestamp: timestamp_to_string_with_padding(time_slot),
        value,
    }
}

/// Owned fixture data; [`Fixture::inputs`] borrows it as the builder's input.
struct Fixture {
    facilities: Vec<FacilitySchema>,
    communities: Vec<EnergyCommunitySchema>,
    assets: Vec<AssetSchema>,
    points: Vec<MeasurementPointSchema>,
    timeseries: Vec<TimeseriesSchema>,
}

impl Fixture {
    /// Seller and buyer in `community-1`, a remote facility in `community-2`; the seller
    /// has a PV asset (linked by facility name) and one export and one import point.
    fn new() -> Self {
        Fixture {
            facilities: vec![
                facility(
                    SELLER_FACILITY,
                    SELLER_FACILITY_NAME,
                    "site-1",
                    SELLER_OWNER,
                ),
                facility(BUYER_FACILITY, "Buyer House", "site-1", BUYER_OWNER),
                facility(REMOTE_FACILITY, "Remote House", "site-2", REMOTE_OWNER),
            ],
            communities: vec![
                community(COMMUNITY, &["site-1"]),
                community(OTHER_COMMUNITY, &["site-2"]),
            ],
            assets: vec![
                asset("pv-1", AssetType::PV, SELLER_FACILITY_NAME),
                asset("battery-remote", AssetType::Battery, "Remote House"),
            ],
            points: vec![
                point(
                    EXPORT_POINT,
                    SELLER_FACILITY,
                    PointDirection::Export,
                    MeasurementPointType::Measurement,
                ),
                point(
                    IMPORT_POINT,
                    SELLER_FACILITY,
                    PointDirection::Import,
                    MeasurementPointType::Measurement,
                ),
            ],
            timeseries: Vec::new(),
        }
    }

    /// The default fixture with the seller exporting `export_kwh` and importing
    /// `import_kwh` at `SLOT`.
    fn exporting(export_kwh: f64, import_kwh: f64) -> Self {
        let mut fixture = Self::new();
        fixture.timeseries = vec![
            value(EXPORT_POINT, SLOT, export_kwh),
            value(IMPORT_POINT, SLOT, import_kwh),
        ];
        fixture
    }

    fn inputs(&self) -> CertificateInputs<'_> {
        CertificateInputs {
            facilities: &self.facilities,
            communities: &self.communities,
            assets: &self.assets,
            measurement_points: &self.points,
            timeseries: &self.timeseries,
        }
    }

    fn build(&self, trades: Vec<DbTradeSchema>) -> Vec<LocalOriginRecord> {
        build_local_origin_records(trades, &self.inputs())
    }
}

fn trade(
    trade_uuid: &str,
    seller_owner: &str,
    buyer: &str,
    time_slot: u64,
    energy_kwh: f64,
    status: TradeStatus,
    creation_time: u64,
) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: trade_uuid.to_string(),
        status,
        seller: hash(seller_owner),
        buyer: buyer.to_string(),
        market_id: "0xmarket".to_string(),
        time_slot,
        creation_time,
        offer_hash: format!("{trade_uuid}-offer"),
        bid_hash: format!("{trade_uuid}-bid"),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: energy_kwh,
            energy_rate: 0.2,
        },
        status_updated_at: Some(SLOT + 1800),
    }
}

/// An `Executed` sale of the seller facility to the buyer facility at `SLOT`.
fn sale(trade_uuid: &str, energy_kwh: f64, creation_time: u64) -> DbTradeSchema {
    trade(
        trade_uuid,
        SELLER_OWNER,
        &hash(BUYER_OWNER),
        SLOT,
        energy_kwh,
        TradeStatus::Executed,
        creation_time,
    )
}

fn trade_references(records: &[LocalOriginRecord]) -> Vec<&str> {
    records
        .iter()
        .map(|record| record.trade_and_delivery.trade_reference[0].as_str())
        .collect()
}

fn keys_of(value: &serde_json::Value) -> HashSet<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

fn key_set(keys: &[&str]) -> HashSet<String> {
    keys.iter().map(|k| k.to_string()).collect()
}

#[test]
fn one_covered_sale_yields_one_record_with_the_full_key_set_and_mapping() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let records = fixture.build(vec![sale("trade-1", 3.0, 100)]);
    assert_eq!(records.len(), 1);

    let value = serde_json::to_value(&records[0]).unwrap();
    assert_eq!(
        keys_of(&value),
        key_set(&[
            "identity",
            "time_and_quantity",
            "production_asset",
            "consumption_asset",
            "location",
            "measurement_provenance",
            "attribute_provenance",
            "beneficiary_and_claim",
            "trade_and_delivery",
        ])
    );
    assert_eq!(
        keys_of(&value["production_asset"]),
        key_set(&[
            "production_asset_id",
            "asset_registry_reference",
            "metering_point_id",
            "asset_class",
            "rated_power",
        ])
    );
    assert_eq!(
        keys_of(&value["beneficiary_and_claim"]),
        key_set(&["owner_id", "consumption_metering_point_id", "facility_id"])
    );
    assert!(value["production_asset"]["rated_power"].is_null());
    assert!(value["production_asset"]["asset_registry_reference"].is_null());
    assert!(value["consumption_asset"]["metering_point_id"].is_null());
    assert!(value["time_and_quantity"]["loss_adjustment"].is_null());
    assert!(value["attribute_provenance"]["support_scheme_status"].is_null());

    let record = &records[0];
    assert_eq!(record.identity.site_id, "ch-aem-lic-goo-poc");
    assert_eq!(record.time_and_quantity.source_slot_timestamp, SLOT);
    assert_eq!(record.time_and_quantity.interval_duration_s, 900);
    assert_eq!(record.time_and_quantity.energy_quantity, 3.0);
    assert_eq!(record.time_and_quantity.rounding_rule, "half_up_2dp");

    assert_eq!(record.production_asset.production_asset_id, SELLER_FACILITY);
    assert_eq!(
        record.production_asset.metering_point_id.as_deref(),
        Some(SELLER_FACILITY)
    );
    assert_eq!(record.production_asset.asset_class, AssetClass::Pv);
    assert!(record.production_asset.rated_power.is_none());

    assert_eq!(
        record.consumption_asset.consumption_asset_id,
        BUYER_FACILITY
    );
    assert_eq!(
        record.consumption_asset.asset_class,
        AssetClass::MeteringPoint
    );

    assert_eq!(record.location.municipality_code, "5226");
    assert_eq!(record.location.grid_operator_id, "AEM");
    assert_eq!(record.location.grid_level, 7);

    let provenance = &record.measurement_provenance;
    assert_eq!(provenance.measurement_id, EXPORT_POINT);
    assert_eq!(provenance.measuring_sensor_id, SELLER_FACILITY);
    assert_eq!(provenance.property_measured, "energy_measured");
    assert_eq!(provenance.flow_direction, FlowDirection::Export);
    assert_eq!(provenance.data_provider_id, "did:example:aem-metering");
    assert_eq!(provenance.measurement_recorded_at, SLOT);

    assert!(!record.attribute_provenance.storage_mediated_flag);

    let claim = &record.beneficiary_and_claim;
    assert_eq!(claim.owner_id, hash(SELLER_OWNER));
    assert_eq!(
        claim.consumption_metering_point_id.as_deref(),
        Some(hash(BUYER_OWNER).as_str())
    );
    assert_eq!(claim.facility_id.as_deref(), Some(SELLER_FACILITY));

    let delivery = &record.trade_and_delivery;
    assert_eq!(delivery.trade_reference, vec!["trade-1".to_string()]);
    assert_eq!(delivery.trade_hash, vec!["trade-1".to_string()]);
    assert_eq!(
        delivery.trade_status_at_issuance,
        Some(TradeStatusAtIssuance::DeliveryVerified)
    );
    assert_eq!(
        delivery.delivery_verification_reference.as_deref(),
        Some("exec:2026-05-14:slot42")
    );
}

#[test]
fn only_executed_trades_yield_records() {
    let fixture = Fixture::exporting(4.0, 0.0);
    for status in [
        TradeStatus::Settled,
        TradeStatus::Matched,
        TradeStatus::Rejected,
    ] {
        let t = trade(
            "trade-1",
            SELLER_OWNER,
            &hash(BUYER_OWNER),
            SLOT,
            3.0,
            status.clone(),
            100,
        );
        assert!(
            fixture.build(vec![t]).is_empty(),
            "{status:?} is not Executed"
        );
    }
}

#[test]
fn non_executed_trades_neither_yield_records_nor_consume_net_export() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let settled = trade(
        "trade-a",
        SELLER_OWNER,
        &hash(BUYER_OWNER),
        SLOT,
        3.0,
        TradeStatus::Settled,
        100,
    );
    let executed = sale("trade-b", 3.0, 200);
    let records = fixture.build(vec![settled, executed]);
    assert_eq!(trade_references(&records), vec!["trade-b"]);
}

#[test]
fn unresolved_seller_yields_no_record_and_does_not_panic() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let t = trade(
        "trade-1",
        "owner-nobody",
        &hash(BUYER_OWNER),
        SLOT,
        3.0,
        TradeStatus::Executed,
        100,
    );
    assert!(fixture.build(vec![t]).is_empty());
}

#[test]
fn non_pv_seller_facility_yields_no_record() {
    // Only a battery.
    let mut fixture = Fixture::exporting(4.0, 0.0);
    fixture.assets = vec![asset("battery-1", AssetType::Battery, SELLER_FACILITY_NAME)];
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());

    // No assets at all.
    fixture.assets.clear();
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());

    // A PV asset on another facility does not count.
    fixture.assets = vec![asset("pv-other", AssetType::PV, "Buyer House")];
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());
}

#[test]
fn a_pv_asset_linked_by_facility_id_also_makes_the_facility_eligible() {
    let mut fixture = Fixture::exporting(4.0, 0.0);
    fixture.assets = vec![asset("pv-1", AssetType::PV, SELLER_FACILITY)];
    assert_eq!(fixture.build(vec![sale("trade-1", 3.0, 100)]).len(), 1);
}

#[test]
fn measurement_points_named_by_facility_name_are_evidence_too() {
    let mut fixture = Fixture::exporting(4.0, 0.0);
    for point in &mut fixture.points {
        point.asset_name = SELLER_FACILITY_NAME.to_string();
    }
    let records = fixture.build(vec![sale("trade-1", 3.0, 100)]);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].production_asset.production_asset_id,
        SELLER_FACILITY
    );
}

#[test]
fn off_boundary_time_slot_yields_no_record() {
    let mut fixture = Fixture::new();
    fixture.timeseries = vec![value(EXPORT_POINT, SLOT + 1, 4.0)];
    let t = trade(
        "trade-1",
        SELLER_OWNER,
        &hash(BUYER_OWNER),
        SLOT + 1,
        3.0,
        TradeStatus::Executed,
        100,
    );
    assert!(fixture.build(vec![t]).is_empty());
}

#[test]
fn non_positive_energy_yields_no_record() {
    let fixture = Fixture::exporting(4.0, 0.0);
    for energy in [0.0, -1.0, f64::NAN] {
        assert!(
            fixture.build(vec![sale("trade-1", energy, 100)]).is_empty(),
            "{energy} kWh is not certifiable"
        );
    }
}

#[test]
fn no_seller_measurement_at_the_slot_yields_no_record() {
    // No timeseries at all.
    let fixture = Fixture::new();
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());

    // A measurement at another slot only.
    let mut fixture = Fixture::new();
    fixture.timeseries = vec![value(EXPORT_POINT, SLOT + 900, 4.0)];
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());

    // A forecast point is not a measurement.
    let mut fixture = Fixture::new();
    fixture.points = vec![point(
        "Forecast:community-1:fac-seller",
        SELLER_FACILITY,
        PointDirection::Export,
        MeasurementPointType::Forecast,
    )];
    fixture.timeseries = vec![value("Forecast:community-1:fac-seller", SLOT, 4.0)];
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());

    // The buyer's measurement is not the seller's evidence.
    let mut fixture = Fixture::new();
    fixture.points = vec![point(
        "Measurement:community-1:fac-buyer",
        BUYER_FACILITY,
        PointDirection::Export,
        MeasurementPointType::Measurement,
    )];
    fixture.timeseries = vec![value("Measurement:community-1:fac-buyer", SLOT, 4.0)];
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());
}

#[test]
fn a_facility_that_is_not_net_exporting_yields_no_record() {
    for (export_kwh, import_kwh) in [(2.0, 3.0), (2.0, 2.0), (0.0, 1.0), (0.0, 0.0)] {
        let fixture = Fixture::exporting(export_kwh, import_kwh);
        assert!(
            fixture.build(vec![sale("trade-1", 0.5, 100)]).is_empty(),
            "export {export_kwh} / import {import_kwh} is not a net export"
        );
    }
}

#[test]
fn net_export_is_export_minus_import() {
    // 5 exported, 2 imported: 3 kWh net.
    let fixture = Fixture::exporting(5.0, 2.0);
    assert_eq!(fixture.build(vec![sale("trade-1", 3.0, 100)]).len(), 1);
    assert!(fixture.build(vec![sale("trade-1", 3.5, 100)]).is_empty());
}

#[test]
fn a_negative_value_is_an_export_whatever_the_point_direction() {
    // Legacy writers store one signed value per facility, negative for export.
    let mut fixture = Fixture::new();
    fixture.points = vec![point(
        "Measurement:community-1:fac-seller",
        SELLER_FACILITY,
        PointDirection::Import,
        MeasurementPointType::Measurement,
    )];
    fixture.timeseries = vec![value("Measurement:community-1:fac-seller", SLOT, -4.0)];
    let records = fixture.build(vec![sale("trade-1", 3.0, 100)]);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].measurement_provenance.measurement_id,
        "Measurement:community-1:fac-seller"
    );

    // And a positive value on that import point is an import.
    fixture.timeseries = vec![value("Measurement:community-1:fac-seller", SLOT, 4.0)];
    assert!(fixture.build(vec![sale("trade-1", 3.0, 100)]).is_empty());
}

#[test]
fn the_largest_export_point_is_the_provenance() {
    let mut fixture = Fixture::new();
    fixture.points.push(point(
        "Measurement:community-1:fac-seller:export-2",
        SELLER_FACILITY,
        PointDirection::Export,
        MeasurementPointType::Measurement,
    ));
    fixture.points[2].property_measured = String::new();
    fixture.timeseries = vec![
        value(EXPORT_POINT, SLOT, 1.0),
        value("Measurement:community-1:fac-seller:export-2", SLOT, 3.0),
    ];
    let records = fixture.build(vec![sale("trade-1", 4.0, 100)]);
    assert_eq!(records.len(), 1);
    let provenance = &records[0].measurement_provenance;
    assert_eq!(
        provenance.measurement_id,
        "Measurement:community-1:fac-seller:export-2"
    );
    // An empty `property_measured` falls back to `GOO_PROPERTY_MEASURED`.
    assert_eq!(provenance.property_measured, "measurement#active_energy");
}

#[test]
fn net_export_is_allocated_in_strict_time_priority() {
    let fixture = Fixture::exporting(1.5, 0.0);

    // Supplied latest-first: the allocation orders by creation time, not input order.
    let two = vec![sale("trade-b", 1.0, 200), sale("trade-a", 1.0, 100)];
    assert_eq!(
        trade_references(&fixture.build(two.clone())),
        vec!["trade-a"]
    );

    // 0.4 would fit in the 0.5 left after trade-a, but trade-b did not fit before it.
    let mut three = two;
    three.push(sale("trade-c", 0.4, 300));
    assert_eq!(trade_references(&fixture.build(three)), vec!["trade-a"]);
}

#[test]
fn equal_creation_times_are_ordered_by_trade_uuid() {
    let fixture = Fixture::exporting(1.5, 0.0);
    let trades = vec![sale("trade-b", 1.0, 100), sale("trade-a", 1.0, 100)];
    assert_eq!(trade_references(&fixture.build(trades)), vec!["trade-a"]);
}

#[test]
fn a_sale_filling_the_net_export_exactly_is_allocated() {
    let fixture = Fixture::exporting(1.5, 0.0);
    let trades = vec![sale("trade-a", 1.0, 100), sale("trade-b", 0.5, 200)];
    assert_eq!(
        trade_references(&fixture.build(trades)),
        vec!["trade-a", "trade-b"]
    );
}

#[test]
fn a_partially_covered_sale_is_not_certified() {
    let fixture = Fixture::exporting(2.0, 0.0);
    assert!(fixture.build(vec![sale("trade-1", 2.5, 100)]).is_empty());
}

#[test]
fn sales_of_other_facilities_do_not_consume_the_seller_net_export() {
    let mut fixture = Fixture::exporting(1.0, 0.0);
    fixture
        .assets
        .push(asset("pv-remote", AssetType::PV, "Remote House"));
    let remote_sale = trade(
        "trade-a",
        REMOTE_OWNER,
        &hash(BUYER_OWNER),
        SLOT,
        5.0,
        TradeStatus::Executed,
        50,
    );
    let records = fixture.build(vec![remote_sale, sale("trade-b", 1.0, 100)]);
    // The remote facility has no measurement; the seller's 1 kWh is still free.
    assert_eq!(trade_references(&records), vec!["trade-b"]);
}

#[test]
fn allocation_does_not_depend_on_which_trades_are_selected() {
    let fixture = Fixture::exporting(1.5, 0.0);
    let inputs = fixture.inputs();
    let earlier = sale("trade-a", 1.0, 100);
    let later = sale("trade-b", 1.0, 200);
    let slot_executed = vec![earlier.clone(), later.clone()];

    let both =
        build_local_origin_records_with_allocation(slot_executed.clone(), &slot_executed, &inputs);
    let only_earlier =
        build_local_origin_records_with_allocation(vec![earlier], &slot_executed, &inputs);
    let only_later =
        build_local_origin_records_with_allocation(vec![later.clone()], &slot_executed, &inputs);

    assert_eq!(trade_references(&both), vec!["trade-a"]);
    assert_eq!(only_earlier, both);
    assert!(only_later.is_empty());

    // Without the earlier trade in `slot_executed` the later one alone would fit.
    let alone = build_local_origin_records(vec![later], &inputs);
    assert_eq!(trade_references(&alone), vec!["trade-b"]);
}

#[test]
fn same_community_trade_is_intra_community() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let records = fixture.build(vec![sale("trade-1", 3.0, 100)]);
    let location = &records[0].location;
    assert_eq!(location.delivery_scope, DeliveryScope::IntraCommunity);
    assert_eq!(location.community_id_origin.as_deref(), Some(COMMUNITY));
    assert_eq!(
        location.community_id_consumption.as_deref(),
        Some(COMMUNITY)
    );
}

#[test]
fn cross_community_trade_is_inter_community() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let t = trade(
        "trade-1",
        SELLER_OWNER,
        &hash(REMOTE_OWNER),
        SLOT,
        3.0,
        TradeStatus::Executed,
        100,
    );
    let records = fixture.build(vec![t]);
    let record = &records[0];
    assert_eq!(
        record.location.delivery_scope,
        DeliveryScope::InterCommunity
    );
    assert_eq!(
        record.location.community_id_origin.as_deref(),
        Some(COMMUNITY)
    );
    assert_eq!(
        record.location.community_id_consumption.as_deref(),
        Some(OTHER_COMMUNITY)
    );
    assert_eq!(
        record.consumption_asset.consumption_asset_id,
        REMOTE_FACILITY
    );
}

#[test]
fn a_community_buyer_is_named_by_its_community_id() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let t = trade(
        "trade-1",
        SELLER_OWNER,
        &hash(OTHER_COMMUNITY),
        SLOT,
        3.0,
        TradeStatus::Executed,
        100,
    );
    let records = fixture.build(vec![t]);
    let record = &records[0];
    assert_eq!(
        record.consumption_asset.consumption_asset_id,
        OTHER_COMMUNITY
    );
    assert_eq!(
        record.location.delivery_scope,
        DeliveryScope::InterCommunity
    );
    assert_eq!(
        record.location.community_id_consumption.as_deref(),
        Some(OTHER_COMMUNITY)
    );
    // Not a facility, so no consumption metering point.
    assert!(record
        .beneficiary_and_claim
        .consumption_metering_point_id
        .is_none());
}

#[test]
fn unresolvable_buyer_is_supplier_offtake() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let t = trade(
        "trade-1",
        SELLER_OWNER,
        "0xno_such_party",
        SLOT,
        3.0,
        TradeStatus::Executed,
        100,
    );
    let records = fixture.build(vec![t]);
    let record = &records[0];
    assert_eq!(
        record.location.delivery_scope,
        DeliveryScope::SupplierOfftake
    );
    assert!(record.location.community_id_consumption.is_none());
    assert_eq!(
        record.consumption_asset.consumption_asset_id,
        "0xno_such_party"
    );
    assert!(record
        .beneficiary_and_claim
        .consumption_metering_point_id
        .is_none());
}

#[test]
fn energy_quantity_rounds_half_up_to_two_decimal_places() {
    assert_eq!(round_half_up_2dp(0.125), 0.13);
    assert_eq!(round_half_up_2dp(1.125), 1.13);

    let fixture = Fixture::exporting(4.0, 0.0);
    let records = fixture.build(vec![sale("trade-1", 1.125, 100)]);
    assert_eq!(records[0].time_and_quantity.energy_quantity, 1.13);
}

#[test]
fn interval_bounds_span_exactly_the_interval_duration_in_rfc3339_utc() {
    let (start, end) = interval_bounds_utc(SLOT, 900).unwrap();
    assert_eq!(start, "2026-05-14T10:30:00+00:00");
    assert_eq!(end, "2026-05-14T10:45:00+00:00");
}

#[test]
fn delivery_verification_reference_names_the_slot_of_the_day() {
    // 12:30 UTC = 45000s past midnight = slot 50.
    let noon_thirty = SLOT - (SLOT % 86400) + 12 * 3600 + 30 * 60;
    assert_eq!(
        delivery_verification_reference(noon_thirty).as_deref(),
        Some("exec:2026-05-14:slot50")
    );
}

#[test]
fn unrepresentable_time_slot_yields_no_record_and_does_not_panic() {
    let absurd = (u64::MAX / 900) * 900;
    assert_eq!(interval_bounds_utc(absurd, 900), None);
    assert_eq!(delivery_verification_reference(absurd), None);

    let mut fixture = Fixture::new();
    fixture.timeseries = vec![value(EXPORT_POINT, absurd, 4.0)];
    let t = trade(
        "trade-1",
        SELLER_OWNER,
        &hash(BUYER_OWNER),
        absurd,
        3.0,
        TradeStatus::Executed,
        100,
    );
    assert!(fixture.build(vec![t]).is_empty());
}

#[test]
fn records_sum_to_the_delivery_verified_energy_only() {
    let fixture = Fixture::exporting(4.0, 0.0);
    let executed = sale("trade-executed", 3.0, 100);
    let settled = trade(
        "trade-settled",
        SELLER_OWNER,
        &hash(BUYER_OWNER),
        SLOT,
        2.0,
        TradeStatus::Settled,
        50,
    );
    let records = fixture.build(vec![executed, settled]);
    let total: f64 = records
        .iter()
        .map(|r| r.time_and_quantity.energy_quantity)
        .sum();
    assert_eq!(total, 3.0);
}

#[test]
fn sort_records_is_deterministic() {
    let mut fixture = Fixture::exporting(4.0, 0.0);
    fixture
        .timeseries
        .push(value(EXPORT_POINT, SLOT + 900, 4.0));
    let later_slot = trade(
        "trade-z",
        SELLER_OWNER,
        &hash(BUYER_OWNER),
        SLOT + 900,
        1.0,
        TradeStatus::Executed,
        100,
    );
    let mut records = fixture.build(vec![
        later_slot,
        sale("trade-b", 1.0, 200),
        sale("trade-a", 1.0, 100),
    ]);
    sort_records(&mut records);
    assert_eq!(
        trade_references(&records),
        vec!["trade-a", "trade-b", "trade-z"]
    );
}

// --- Window validation (D7) -------------------------------------------

mod window {
    use gsy_offchain_storage::certificates::query::{validate_window, GooWindow, MAX_WINDOW_S};

    #[test]
    fn start_time_is_required() {
        assert!(validate_window(None, None).is_err());
        assert!(validate_window(None, Some(900)).is_err());
    }

    #[test]
    fn end_time_defaults_to_start_plus_900() {
        assert_eq!(
            validate_window(Some(1_000), None),
            Ok(GooWindow {
                start_time: 1_000,
                end_time: 1_900
            })
        );
        // Saturates instead of overflowing.
        assert_eq!(
            validate_window(Some(u64::MAX), None).map(|w| w.end_time),
            Ok(u64::MAX)
        );
    }

    #[test]
    fn a_window_of_exactly_900_seconds_or_less_is_accepted() {
        assert_eq!(MAX_WINDOW_S, 900);
        assert!(validate_window(Some(1_000), Some(1_900)).is_ok());
        assert!(validate_window(Some(1_000), Some(1_000)).is_ok());
    }

    #[test]
    fn end_before_start_or_a_wider_window_is_rejected() {
        assert!(validate_window(Some(1_000), Some(999)).is_err());
        assert!(validate_window(Some(1_000), Some(1_901)).is_err());
    }
}

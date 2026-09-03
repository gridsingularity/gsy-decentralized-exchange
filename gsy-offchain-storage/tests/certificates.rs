use std::collections::HashSet;

use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};
use gsy_offchain_storage::certificates::builder::{
    build_local_origin_records, delivery_verification_reference, interval_bounds_utc, round_half_up_2dp,
};
use gsy_offchain_storage::certificates::schema::*;

const COMMUNITY_UUID: &str = "11111111-1111-1111-1111-111111111111";
const COMMUNITY_NAME: &str = "LugaggiaInnovationCommunity";
const SELLER_HASH: &str = "0xseller_area_hash";
const SELLER_UUID: &str = "seller-uuid-1";
const SELLER_NAME: &str = "LIC01PV";
const BUYER_HASH: &str = "0xbuyer_area_hash";
const BUYER_UUID: &str = "buyer-uuid-1";
const BUYER_NAME: &str = "LIC01SM";
const SLOT: u64 = 1778754600; // 2026-05-14T10:30:00Z, on a 900s boundary

fn area(area_uuid: &str, name: &str, area_type: AssetType, area_hash: &str) -> AreaTopologySchema {
    AreaTopologySchema {
        area_uuid: area_uuid.to_string(),
        name: name.to_string(),
        area_type,
        area_hash: area_hash.to_string(),
    }
}

fn market(community_uuid: &str, community_name: &str, areas: Vec<AreaTopologySchema>) -> MarketTopologySchema {
    MarketTopologySchema {
        market_id: "market-1".to_string(),
        community_uuid: community_uuid.to_string(),
        community_name: community_name.to_string(),
        time_slot: SLOT as u32,
        creation_time: 0,
        community_areas: areas,
    }
}

fn default_topology() -> Vec<MarketTopologySchema> {
    vec![market(
        COMMUNITY_UUID,
        COMMUNITY_NAME,
        vec![
            area(SELLER_UUID, SELLER_NAME, AssetType::PV, SELLER_HASH),
            area(BUYER_UUID, BUYER_NAME, AssetType::SMART_METER, BUYER_HASH),
        ],
    )]
}

fn measurement(area_uuid: &str, area_hash: &str, community_uuid: &str, time_slot: u64, creation_time: u64, energy_kwh: f64) -> MeasurementSchema {
    MeasurementSchema {
        area_uuid: area_uuid.to_string(),
        area_hash: area_hash.to_string(),
        community_uuid: community_uuid.to_string(),
        time_slot,
        creation_time,
        energy_kwh,
    }
}

fn default_production_measurement() -> MeasurementSchema {
    measurement("FLEXO-LIC-LIC01PV-1", SELLER_HASH, COMMUNITY_UUID, SLOT, 1000, -3.0)
}

fn order_component(area_uuid: &str) -> DbOrderComponent {
    DbOrderComponent {
        area_uuid: area_uuid.to_string(),
        market_id: "market-1".to_string(),
        time_slot: SLOT,
        creation_time: 0,
        energy: 3.0,
        energy_rate: 10.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn trade(
    trade_uuid: &str,
    seller_hash: &str,
    buyer_hash: &str,
    time_slot: u64,
    selected_energy: f64,
    status: TradeStatus,
    creation_time: u64,
    status_updated_at: Option<u64>,
) -> TradeSchema {
    TradeSchema {
        _id: trade_uuid.to_string(),
        status,
        seller: "seller-account".to_string(),
        buyer: "buyer-account".to_string(),
        market_id: "market-1".to_string(),
        time_slot,
        trade_uuid: trade_uuid.to_string(),
        creation_time,
        status_updated_at,
        offer: DbOffer {
            seller: "seller-account".to_string(),
            nonce: 1,
            offer_component: order_component(seller_hash),
        },
        offer_hash: "0xoffer".to_string(),
        bid: DbBid {
            buyer: "buyer-account".to_string(),
            nonce: 1,
            bid_component: order_component(buyer_hash),
        },
        bid_hash: "0xbid".to_string(),
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy,
            energy_rate: 10.0,
            trade_uuid: trade_uuid.to_string(),
        },
    }
}

fn default_trade(trade_uuid: &str) -> TradeSchema {
    trade(
        trade_uuid,
        SELLER_HASH,
        BUYER_HASH,
        SLOT,
        3.0,
        TradeStatus::Executed,
        900,
        Some(1200),
    )
}

fn keys_of(value: &serde_json::Value) -> HashSet<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

fn key_set(keys: &[&str]) -> HashSet<String> {
    keys.iter().map(|k| k.to_string()).collect()
}

#[test]
fn one_trade_with_a_measurement_yields_one_record_with_the_full_key_set() {
    let records = build_local_origin_records(
        vec![default_trade("trade-1")],
        &default_topology(),
        &[default_production_measurement()],
    );
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
        keys_of(&value["identity"]),
        key_set(&["record_type", "site_id"])
    );
    assert_eq!(
        keys_of(&value["time_and_quantity"]),
        key_set(&[
            "interval_start",
            "interval_end",
            "interval_duration_s",
            "source_slot_timestamp",
            "energy_quantity",
            "energy_unit",
            "rounding_rule",
            "loss_adjustment",
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
        keys_of(&value["consumption_asset"]),
        key_set(&[
            "consumption_asset_id",
            "asset_registry_reference",
            "metering_point_id",
            "asset_class",
        ])
    );
    assert_eq!(
        keys_of(&value["location"]),
        key_set(&[
            "municipality_code",
            "grid_operator_id",
            "grid_level",
            "community_id_origin",
            "community_id_consumption",
            "delivery_scope",
        ])
    );
    assert_eq!(
        keys_of(&value["measurement_provenance"]),
        key_set(&[
            "measurement_id",
            "measuring_sensor_id",
            "property_measured",
            "flow_direction",
            "data_provider_id",
            "data_completeness",
            "source_of_record",
            "data_record_class",
            "measurement_recorded_at",
        ])
    );
    assert_eq!(
        keys_of(&value["attribute_provenance"]),
        key_set(&["support_scheme_status", "storage_mediated_flag"])
    );
    assert_eq!(
        keys_of(&value["beneficiary_and_claim"]),
        key_set(&["owner_id", "consumption_metering_point_id", "facility_id"])
    );
    assert_eq!(
        keys_of(&value["trade_and_delivery"]),
        key_set(&[
            "trade_reference",
            "trade_hash",
            "trade_status_at_issuance",
            "delivery_verification_reference",
        ])
    );

    assert!(value["production_asset"]["asset_registry_reference"].is_null());
    assert!(value["production_asset"]["metering_point_id"].is_null());
    assert!(value["production_asset"]["rated_power"].is_null());
    assert!(value["time_and_quantity"]["loss_adjustment"].is_null());
    assert!(value["attribute_provenance"]["support_scheme_status"].is_null());
    assert!(value["beneficiary_and_claim"]["facility_id"].is_null());
}

#[test]
fn two_buyers_from_one_pv_asset_in_one_slot_yield_two_records() {
    let other_buyer_hash = "0xother_buyer_area_hash";
    let mut topology = default_topology();
    topology[0]
        .community_areas
        .push(area("buyer-uuid-2", "LIC02SM", AssetType::SMART_METER, other_buyer_hash));

    let trades = vec![
        default_trade("trade-1"),
        trade(
            "trade-2",
            SELLER_HASH,
            other_buyer_hash,
            SLOT,
            1.5,
            TradeStatus::Executed,
            900,
            Some(1200),
        ),
    ];
    let records = build_local_origin_records(trades, &topology, &[default_production_measurement()]);

    assert_eq!(records.len(), 2);
    assert_ne!(
        records[0].consumption_asset.consumption_asset_id,
        records[1].consumption_asset.consumption_asset_id
    );
    for record in &records {
        assert_eq!(record.trade_and_delivery.trade_reference.len(), 1);
    }
}

#[test]
fn no_production_measurement_yields_no_record() {
    let records = build_local_origin_records(vec![default_trade("trade-1")], &default_topology(), &[]);
    assert!(records.is_empty());
}

#[test]
fn only_executed_trades_yield_records() {
    for status in [TradeStatus::Settled, TradeStatus::Penalized] {
        let t = trade("trade-1", SELLER_HASH, BUYER_HASH, SLOT, 3.0, status, 900, Some(1200));
        let records = build_local_origin_records(vec![t], &default_topology(), &[default_production_measurement()]);
        assert!(records.is_empty());
    }
}

#[test]
fn energy_quantity_rounds_half_up_to_two_decimal_places() {
    assert_eq!(round_half_up_2dp(0.125), 0.13);
    assert_eq!(round_half_up_2dp(1.125), 1.13);

    let t = trade("trade-1", SELLER_HASH, BUYER_HASH, SLOT, 1.125, TradeStatus::Executed, 900, Some(1200));
    let records = build_local_origin_records(vec![t], &default_topology(), &[default_production_measurement()]);
    assert_eq!(records[0].time_and_quantity.energy_quantity, 1.13);
}

/// `measurement_recorded_at` is the measurement's own arrival time and nothing else — it is
/// pure provenance, and does not blend in any trade timestamp. The endpoint windows on the
/// trade's `status_updated_at` instead, so this field is free to say only what the spec says
/// it says.
#[test]
fn measurement_recorded_at_is_the_measurement_creation_time() {
    // The trade's own timestamps are set far above the measurement's on purpose: if either
    // leaked into this field, these assertions would report them instead of 5000.
    let m = measurement("m1", SELLER_HASH, COMMUNITY_UUID, SLOT, 5000, -3.0);
    let t = trade("trade-1", SELLER_HASH, BUYER_HASH, SLOT, 3.0, TradeStatus::Executed, 900, Some(99_000));
    let records = build_local_origin_records(vec![t], &default_topology(), &[m]);
    assert_eq!(records[0].measurement_provenance.measurement_recorded_at, 5000);

    // Unaffected by a trade with no status change time recorded.
    let m = measurement("m1", SELLER_HASH, COMMUNITY_UUID, SLOT, 5000, -3.0);
    let t = trade("trade-1", SELLER_HASH, BUYER_HASH, SLOT, 3.0, TradeStatus::Executed, 77_000, None);
    let records = build_local_origin_records(vec![t], &default_topology(), &[m]);
    assert_eq!(records[0].measurement_provenance.measurement_recorded_at, 5000);
}

#[test]
fn flow_direction_follows_the_measurement_sign() {
    let export = measurement("m1", SELLER_HASH, COMMUNITY_UUID, SLOT, 1000, -3.0);
    let records = build_local_origin_records(
        vec![default_trade("trade-1")],
        &default_topology(),
        &[export],
    );
    assert_eq!(records[0].measurement_provenance.flow_direction, FlowDirection::Export);

    let import = measurement("m1", SELLER_HASH, COMMUNITY_UUID, SLOT, 1000, 3.0);
    let records = build_local_origin_records(
        vec![default_trade("trade-1")],
        &default_topology(),
        &[import],
    );
    assert_eq!(records[0].measurement_provenance.flow_direction, FlowDirection::Import);
}

#[test]
fn measuring_sensor_id_is_the_measurement_area_uuid() {
    let m = measurement("FLEXO-LIC-LIC01PV-1", SELLER_HASH, COMMUNITY_UUID, SLOT, 1000, -3.0);
    let records = build_local_origin_records(vec![default_trade("trade-1")], &default_topology(), &[m]);
    assert_eq!(records[0].measurement_provenance.measuring_sensor_id, "FLEXO-LIC-LIC01PV-1");
    assert_ne!(records[0].measurement_provenance.measuring_sensor_id, SELLER_HASH);
    assert_ne!(records[0].measurement_provenance.measuring_sensor_id, SELLER_UUID);
}

#[test]
fn same_community_trade_is_intra_community() {
    let records = build_local_origin_records(
        vec![default_trade("trade-1")],
        &default_topology(),
        &[default_production_measurement()],
    );
    let location = &records[0].location;
    assert_eq!(location.delivery_scope, DeliveryScope::IntraCommunity);
    assert_eq!(location.community_id_origin, location.community_id_consumption);
    assert_eq!(location.community_id_origin.as_deref(), Some(COMMUNITY_NAME));
}

#[test]
fn cross_community_trade_is_inter_community_via_community_hash() {
    let other_community_uuid = "22222222-2222-2222-2222-222222222222";
    let other_community_name = "OtherCommunity";
    let mut topology = default_topology();
    topology.push(market(other_community_uuid, other_community_name, vec![]));

    let buyer_community_hash = h256_to_string(community_id_from_uuid(other_community_uuid));
    let t = trade(
        "trade-1",
        SELLER_HASH,
        &buyer_community_hash,
        SLOT,
        3.0,
        TradeStatus::Executed,
        900,
        Some(1200),
    );
    let records = build_local_origin_records(vec![t], &topology, &[default_production_measurement()]);

    let location = &records[0].location;
    assert_eq!(location.delivery_scope, DeliveryScope::InterCommunity);
    assert_eq!(location.community_id_origin.as_deref(), Some(COMMUNITY_NAME));
    assert_eq!(location.community_id_consumption.as_deref(), Some(other_community_name));
    assert_ne!(location.community_id_origin, location.community_id_consumption);
}

#[test]
fn unresolvable_buyer_is_supplier_offtake() {
    let t = trade(
        "trade-1",
        SELLER_HASH,
        "0xno_such_area_or_community",
        SLOT,
        3.0,
        TradeStatus::Executed,
        900,
        Some(1200),
    );
    let records = build_local_origin_records(vec![t], &default_topology(), &[default_production_measurement()]);

    let location = &records[0].location;
    assert_eq!(location.delivery_scope, DeliveryScope::SupplierOfftake);
    assert!(location.community_id_consumption.is_none());
}

#[test]
fn non_pv_seller_yields_no_record() {
    for (asset_type, hash) in [(AssetType::BATTERY, "0xbattery_hash"), (AssetType::SMART_METER, "0xsm_hash")] {
        let mut topology = default_topology();
        topology[0].community_areas[0] = area(SELLER_UUID, SELLER_NAME, asset_type, hash);
        let t = trade("trade-1", hash, BUYER_HASH, SLOT, 3.0, TradeStatus::Executed, 900, Some(1200));
        let m = measurement("m1", hash, COMMUNITY_UUID, SLOT, 1000, -3.0);
        let records = build_local_origin_records(vec![t], &topology, &[m]);
        assert!(records.is_empty());
    }
}

#[test]
fn off_boundary_time_slot_yields_no_record() {
    let t = trade("trade-1", SELLER_HASH, BUYER_HASH, SLOT + 1, 3.0, TradeStatus::Executed, 900, Some(1200));
    let m = measurement("m1", SELLER_HASH, COMMUNITY_UUID, SLOT + 1, 1000, -3.0);
    let records = build_local_origin_records(vec![t], &default_topology(), &[m]);
    assert!(records.is_empty());
}

#[test]
fn unknown_seller_area_yields_no_record_and_does_not_panic() {
    let t = trade("trade-1", "0xnowhere", BUYER_HASH, SLOT, 3.0, TradeStatus::Executed, 900, Some(1200));
    let records = build_local_origin_records(vec![t], &default_topology(), &[default_production_measurement()]);
    assert!(records.is_empty());
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

/// An unrepresentable `time_slot` must skip the trade, not take the whole request down
/// with it: a read-side projection never fails a request for one bad row (§5).
#[test]
fn unrepresentable_time_slot_yields_no_record_and_does_not_panic() {
    let absurd = (u64::MAX / 900) * 900;
    assert_eq!(absurd % 900, 0, "must clear the interval-boundary check to reach the formatter");
    assert_eq!(interval_bounds_utc(absurd, 900), None);
    assert_eq!(delivery_verification_reference(absurd), None);

    let t = trade("trade-1", SELLER_HASH, BUYER_HASH, absurd, 3.0, TradeStatus::Executed, 900, Some(1200));
    let m = measurement("m1", SELLER_HASH, COMMUNITY_UUID, absurd, 1000, -4.0);
    assert!(build_local_origin_records(vec![t], &default_topology(), &[m]).is_empty());
}

#[test]
fn rule_13_records_sum_to_the_delivery_verified_energy_only() {
    let executed = trade("trade-executed", SELLER_HASH, BUYER_HASH, SLOT, 3.0, TradeStatus::Executed, 900, Some(1200));
    let penalized = trade("trade-penalized", SELLER_HASH, BUYER_HASH, SLOT, 2.0, TradeStatus::Penalized, 900, Some(1200));
    let m = measurement("m1", SELLER_HASH, COMMUNITY_UUID, SLOT, 1000, -4.0);

    let records = build_local_origin_records(vec![executed, penalized], &default_topology(), &[m]);

    assert_eq!(records.len(), 1);
    let total: f64 = records.iter().map(|r| r.time_and_quantity.energy_quantity).sum();
    assert_eq!(total, 3.0);
    assert!(total <= 4.0);
}

#[test]
fn every_enum_serialises_to_the_spec_spelling() {
    assert_eq!(serde_json::to_value(RecordType::LocalOriginRecord).unwrap(), "local_origin_record");
    assert_eq!(serde_json::to_value(EnergyUnit::KWh).unwrap(), "kWh");
    assert_eq!(serde_json::to_value(AssetClass::Pv).unwrap(), "PV");
    assert_eq!(serde_json::to_value(AssetClass::Battery).unwrap(), "Battery");
    assert_eq!(serde_json::to_value(AssetClass::HeatPump).unwrap(), "HeatPump");
    assert_eq!(serde_json::to_value(AssetClass::MeteringPoint).unwrap(), "MeteringPoint");
    assert_eq!(serde_json::to_value(FlowDirection::Import).unwrap(), "import");
    assert_eq!(serde_json::to_value(FlowDirection::Export).unwrap(), "export");
    assert_eq!(serde_json::to_value(DeliveryScope::IntraCommunity).unwrap(), "intra_community");
    assert_eq!(serde_json::to_value(DeliveryScope::InterCommunity).unwrap(), "inter_community");
    assert_eq!(serde_json::to_value(DeliveryScope::SupplierOfftake).unwrap(), "supplier_offtake");
    assert_eq!(serde_json::to_value(SupportSchemeStatus::FeedInTariff).unwrap(), "feed_in_tariff");
    assert_eq!(serde_json::to_value(SupportSchemeStatus::OneOffPayment).unwrap(), "one_off_payment");
    assert_eq!(serde_json::to_value(SupportSchemeStatus::WaitingList).unwrap(), "waiting_list");
    assert_eq!(serde_json::to_value(SupportSchemeStatus::None_).unwrap(), "none");
    assert_eq!(serde_json::to_value(DataCompleteness::Complete).unwrap(), "complete");
    assert_eq!(serde_json::to_value(DataCompleteness::Substituted).unwrap(), "substituted");
    assert_eq!(serde_json::to_value(DataCompleteness::Incomplete).unwrap(), "incomplete");
    assert_eq!(serde_json::to_value(SourceOfRecord::Platform).unwrap(), "platform");
    assert_eq!(serde_json::to_value(SourceOfRecord::OperatorValidated).unwrap(), "operator_validated");
    assert_eq!(serde_json::to_value(DataRecordClass::Measurement).unwrap(), "measurement");
    assert_eq!(serde_json::to_value(DataRecordClass::Forecast).unwrap(), "forecast");
    assert_eq!(serde_json::to_value(TradeStatusAtIssuance::Settled).unwrap(), "settled");
    assert_eq!(serde_json::to_value(TradeStatusAtIssuance::DeliveryVerified).unwrap(), "delivery_verified");
}

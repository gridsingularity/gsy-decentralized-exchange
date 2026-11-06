use primitives::certificates::{
    AssetClass, AttributeProvenance, BeneficiaryAndClaim, ConsumptionAsset, DataCompleteness,
    DataRecordClass, DeliveryScope, EnergyUnit, FlowDirection, LocalOriginRecord,
    MeasurementProvenance, ProductionAsset, RecordIdentity, RecordLocation, RecordTimeAndQuantity,
    RecordType, SourceOfRecord, SupportSchemeStatus, TradeAndDeliveryReference,
    TradeStatusAtIssuance,
};
use serde_json::{json, Value};

fn record() -> LocalOriginRecord {
    LocalOriginRecord {
        identity: RecordIdentity {
            record_type: RecordType::LocalOriginRecord,
            site_id: "site".to_string(),
        },
        time_and_quantity: RecordTimeAndQuantity {
            interval_start: "2026-05-14T10:30:00+00:00".to_string(),
            interval_end: "2026-05-14T10:45:00+00:00".to_string(),
            interval_duration_s: 900,
            source_slot_timestamp: 1_778_754_600,
            energy_quantity: 1.25,
            energy_unit: EnergyUnit::KWh,
            rounding_rule: "half_up_2dp".to_string(),
            loss_adjustment: None,
        },
        production_asset: ProductionAsset {
            production_asset_id: "facility-a".to_string(),
            asset_registry_reference: None,
            metering_point_id: Some("facility-a".to_string()),
            asset_class: AssetClass::Pv,
            rated_power: None,
        },
        consumption_asset: ConsumptionAsset {
            consumption_asset_id: "facility-b".to_string(),
            asset_registry_reference: None,
            metering_point_id: None,
            asset_class: AssetClass::MeteringPoint,
        },
        location: RecordLocation {
            municipality_code: "5226".to_string(),
            grid_operator_id: "AEM".to_string(),
            grid_level: 7,
            community_id_origin: Some("c1".to_string()),
            community_id_consumption: Some("c1".to_string()),
            delivery_scope: DeliveryScope::IntraCommunity,
        },
        measurement_provenance: MeasurementProvenance {
            measurement_id: "m1".to_string(),
            measuring_sensor_id: "facility-a".to_string(),
            property_measured: "energy_measured".to_string(),
            flow_direction: FlowDirection::Export,
            data_provider_id: "did:example:aem-metering".to_string(),
            data_completeness: DataCompleteness::Complete,
            source_of_record: SourceOfRecord::Platform,
            data_record_class: DataRecordClass::Measurement,
            measurement_recorded_at: 1_778_754_600,
        },
        attribute_provenance: AttributeProvenance {
            support_scheme_status: None,
            storage_mediated_flag: false,
        },
        beneficiary_and_claim: BeneficiaryAndClaim {
            owner_id: "0xseller".to_string(),
            consumption_metering_point_id: Some("0xbuyer".to_string()),
            facility_id: Some("facility-a".to_string()),
        },
        trade_and_delivery: TradeAndDeliveryReference {
            trade_reference: vec!["t1".to_string()],
            trade_hash: vec!["t1".to_string()],
            trade_status_at_issuance: Some(TradeStatusAtIssuance::DeliveryVerified),
            delivery_verification_reference: Some("exec:2026-05-14:slot42".to_string()),
        },
    }
}

/// Number of leaf fields of a JSON value (a null or array counts as one leaf).
fn leaf_count(value: &Value) -> usize {
    match value {
        Value::Object(map) => map.values().map(leaf_count).sum(),
        _ => 1,
    }
}

#[test]
fn a_record_has_the_43_fields() {
    let value = serde_json::to_value(record()).unwrap();
    assert_eq!(leaf_count(&value), 43);
}

#[test]
fn a_record_round_trips_through_json() {
    let original = record();
    let value = serde_json::to_value(&original).unwrap();
    let parsed: LocalOriginRecord = serde_json::from_value(value).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn unset_optionals_serialise_as_null_not_absent() {
    let value = serde_json::to_value(record()).unwrap();
    assert_eq!(value["production_asset"]["rated_power"], Value::Null);
    assert_eq!(value["time_and_quantity"]["loss_adjustment"], Value::Null);
    assert_eq!(
        value["attribute_provenance"]["support_scheme_status"],
        Value::Null
    );
    assert!(value["production_asset"]
        .as_object()
        .unwrap()
        .contains_key("rated_power"));
}

#[test]
fn every_enum_serialises_to_the_spec_spelling() {
    let cases: Vec<(Value, &str)> = vec![
        (json!(RecordType::LocalOriginRecord), "local_origin_record"),
        (json!(EnergyUnit::KWh), "kWh"),
        (json!(AssetClass::Pv), "PV"),
        (json!(AssetClass::Battery), "Battery"),
        (json!(AssetClass::HeatPump), "HeatPump"),
        (json!(AssetClass::MeteringPoint), "MeteringPoint"),
        (json!(FlowDirection::Import), "import"),
        (json!(FlowDirection::Export), "export"),
        (json!(DeliveryScope::IntraCommunity), "intra_community"),
        (json!(DeliveryScope::InterCommunity), "inter_community"),
        (json!(DeliveryScope::SupplierOfftake), "supplier_offtake"),
        (json!(SupportSchemeStatus::FeedInTariff), "feed_in_tariff"),
        (json!(SupportSchemeStatus::OneOffPayment), "one_off_payment"),
        (json!(SupportSchemeStatus::WaitingList), "waiting_list"),
        (json!(SupportSchemeStatus::None_), "none"),
        (json!(DataCompleteness::Complete), "complete"),
        (json!(DataCompleteness::Substituted), "substituted"),
        (json!(DataCompleteness::Incomplete), "incomplete"),
        (json!(SourceOfRecord::Platform), "platform"),
        (
            json!(SourceOfRecord::OperatorValidated),
            "operator_validated",
        ),
        (json!(DataRecordClass::Measurement), "measurement"),
        (json!(DataRecordClass::Forecast), "forecast"),
        (json!(TradeStatusAtIssuance::Settled), "settled"),
        (
            json!(TradeStatusAtIssuance::DeliveryVerified),
            "delivery_verified",
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(value, json!(expected));
    }
}

//! Annex A `local_origin_record` shape (`openapi.yaml:1271-1811`). Read-side API types
//! only — `Serialize` here, no `Encode`/`Decode`, no MongoDB.

use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
pub struct LocalOriginRecord {
    pub identity: RecordIdentity,
    pub time_and_quantity: RecordTimeAndQuantity,
    pub production_asset: ProductionAsset,
    pub consumption_asset: ConsumptionAsset,
    pub location: RecordLocation,
    pub measurement_provenance: MeasurementProvenance,
    pub attribute_provenance: AttributeProvenance,
    pub beneficiary_and_claim: BeneficiaryAndClaim,
    pub trade_and_delivery: TradeAndDeliveryReference,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct RecordIdentity {
    pub record_type: RecordType,
    pub site_id: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct RecordTimeAndQuantity {
    pub interval_start: String,
    pub interval_end: String,
    pub interval_duration_s: u64,
    pub source_slot_timestamp: u64,
    pub energy_quantity: f64,
    pub energy_unit: EnergyUnit,
    pub rounding_rule: String,
    pub loss_adjustment: Option<f64>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ProductionAsset {
    pub production_asset_id: String,
    pub asset_registry_reference: Option<String>,
    pub metering_point_id: Option<String>,
    pub asset_class: AssetClass,
    pub rated_power: Option<RatedPower>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ConsumptionAsset {
    pub consumption_asset_id: String,
    pub asset_registry_reference: Option<String>,
    pub metering_point_id: Option<String>,
    pub asset_class: AssetClass,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct RecordLocation {
    pub municipality_code: String,
    pub grid_operator_id: String,
    pub grid_level: u32,
    pub community_id_origin: Option<String>,
    pub community_id_consumption: Option<String>,
    pub delivery_scope: DeliveryScope,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct MeasurementProvenance {
    pub measurement_id: String,
    pub measuring_sensor_id: String,
    pub property_measured: String,
    pub flow_direction: FlowDirection,
    pub data_provider_id: String,
    pub data_completeness: DataCompleteness,
    pub source_of_record: SourceOfRecord,
    pub data_record_class: DataRecordClass,
    pub measurement_recorded_at: u64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct AttributeProvenance {
    pub support_scheme_status: Option<SupportSchemeStatus>,
    pub storage_mediated_flag: bool,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct BeneficiaryAndClaim {
    pub owner_id: String,
    pub consumption_metering_point_id: Option<String>,
    pub facility_id: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct TradeAndDeliveryReference {
    pub trade_reference: Vec<String>,
    pub trade_hash: Vec<String>,
    pub trade_status_at_issuance: Option<TradeStatusAtIssuance>,
    pub delivery_verification_reference: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct RatedPower {
    pub value: f64,
    pub unit: String,
}

/// Only the variant this service emits is declared — a `flexibility_activation_record`
/// is never built here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecordType {
    #[serde(rename = "local_origin_record")]
    LocalOriginRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EnergyUnit {
    #[serde(rename = "kWh")]
    KWh,
}

/// The DEX has no controllable thermal or generic metering-point production asset, but
/// `MeteringPoint` is still emitted on the consumption side (`asset_class_of`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AssetClass {
    #[serde(rename = "PV")]
    Pv,
    #[serde(rename = "Battery")]
    Battery,
    #[serde(rename = "HeatPump")]
    HeatPump,
    #[serde(rename = "MeteringPoint")]
    MeteringPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlowDirection {
    #[serde(rename = "import")]
    Import,
    #[serde(rename = "export")]
    Export,
}

/// `grid_export` is declared by the spec but not here — the DEX has no grid-export path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeliveryScope {
    #[serde(rename = "intra_community")]
    IntraCommunity,
    #[serde(rename = "inter_community")]
    InterCommunity,
    #[serde(rename = "supplier_offtake")]
    SupplierOfftake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SupportSchemeStatus {
    #[serde(rename = "feed_in_tariff")]
    FeedInTariff,
    #[serde(rename = "one_off_payment")]
    OneOffPayment,
    #[serde(rename = "waiting_list")]
    WaitingList,
    #[serde(rename = "none")]
    None_,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DataCompleteness {
    #[serde(rename = "complete")]
    Complete,
    #[serde(rename = "substituted")]
    Substituted,
    #[serde(rename = "incomplete")]
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SourceOfRecord {
    #[serde(rename = "platform")]
    Platform,
    #[serde(rename = "operator_validated")]
    OperatorValidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DataRecordClass {
    #[serde(rename = "measurement")]
    Measurement,
    #[serde(rename = "forecast")]
    Forecast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TradeStatusAtIssuance {
    #[serde(rename = "settled")]
    Settled,
    #[serde(rename = "delivery_verified")]
    DeliveryVerified,
}

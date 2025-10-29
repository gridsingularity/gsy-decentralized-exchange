use actix_web::{HttpResponse, Responder};
use actix_web::web::Json;
use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use gsy_offchain_primitives::db_api_schema::profiles::{
    PVMeasurementSchema, BatteryMeasurementSchema, SmartMeterMeasurementSchema,
    TransformerMeasurementSchema};
use crate::db::DbRef;


#[derive(Deserialize, Serialize, Encode, Decode, Clone)]
#[serde(untagged)]
pub enum AssetMeasurementInput {
    MeasurementPV(PVMeasurementSchema),
    MeasurementBattery(BatteryMeasurementSchema),
    MeasurementSmartMeter(SmartMeterMeasurementSchema),
    MeasurementTransformer(TransformerMeasurementSchema),
}

pub async fn post_asset_measurements(
    measurements: Json<Vec<AssetMeasurementInput>>,
    db: DbRef,
) -> impl Responder {
    let mut pv_data: Vec<PVMeasurementSchema> = Vec::new();
    let mut smart_meter_data: Vec<SmartMeterMeasurementSchema> = Vec::new();
    let mut battery_data: Vec<BatteryMeasurementSchema> = Vec::new();
    let mut transformer_data: Vec<TransformerMeasurementSchema> = Vec::new();
    tracing::error!("ENTERED ENDPOINT");
    for measurement in measurements.to_vec() {
        match measurement.clone() {
            AssetMeasurementInput::MeasurementPV(schema) => {
                pv_data.push(schema);
            }
            AssetMeasurementInput::MeasurementBattery(schema) => {
                battery_data.push(schema);
            }
            AssetMeasurementInput::MeasurementSmartMeter(schema) => {
                smart_meter_data.push(schema);
            }
            AssetMeasurementInput::MeasurementTransformer(schema) => {
                transformer_data.push(schema);
            }
        }
    }

    match db.get_ref().pv_measurements().insert_measurements(pv_data).await {
        Ok(_ids) => (),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    match db.get_ref().battery_measurements().insert_measurements(battery_data).await {
        Ok(_ids) => (),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    match db.get_ref().smart_meter_measurements().insert_measurements(smart_meter_data).await {
        Ok(_ids) => (),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    match db.get_ref().transformer_measurements().insert_measurements(transformer_data).await {
        Ok(_ids) => (),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    HttpResponse::Ok().finish()
}

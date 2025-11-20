use actix_web::{HttpResponse, Responder};
use actix_web::web::{Json, Query};
use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use gsy_offchain_primitives::db_api_schema::profiles::{
    PVMeasurementSchema, BatteryMeasurementSchema, SmartMeterMeasurementSchema,
    TransformerMeasurementSchema};
use crate::db::DbRef;
use crate::routes::MarketParameters;

#[derive(Deserialize, Serialize, Encode, Decode, Clone)]
#[serde(untagged)]
pub enum AssetMeasurementInput {
    MeasurementPV(PVMeasurementSchema),
    MeasurementBattery(BatteryMeasurementSchema),
    MeasurementSmartMeter(SmartMeterMeasurementSchema),
    MeasurementTransformer(TransformerMeasurementSchema),
}

#[derive(Deserialize)]
pub struct AssetMeasurementParameters {
    area_uuid: String,
    start_time: Option<u32>,
    end_time: Option<u32>,
}

pub async fn post_asset_measurements(
    measurements: Json<Vec<AssetMeasurementInput>>,
    db: DbRef,
) -> impl Responder {
    let mut pv_data: Vec<PVMeasurementSchema> = Vec::new();
    let mut smart_meter_data: Vec<SmartMeterMeasurementSchema> = Vec::new();
    let mut battery_data: Vec<BatteryMeasurementSchema> = Vec::new();
    let mut transformer_data: Vec<TransformerMeasurementSchema> = Vec::new();
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

    if pv_data.len() > 0 {
        match db.get_ref().pv_measurements().insert_measurements(pv_data).await {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    if battery_data.len() > 0 {
        match db.get_ref().battery_measurements().insert_measurements(battery_data).await {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    if smart_meter_data.len() > 0 {
        match db.get_ref().smart_meter_measurements().insert_measurements(smart_meter_data).await {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    if transformer_data.len() > 0 {
        match db.get_ref().transformer_measurements().insert_measurements(transformer_data).await {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    HttpResponse::Ok().finish()
}


pub async fn get_asset_measurements(db: DbRef, params: Query<AssetMeasurementParameters>) -> impl Responder {
    match db.get_ref().pv_measurements().get_measurements(
        params.area_uuid.clone(), params.start_time, params.end_time,
    ).await {
        Ok(pv_data) => {
            HttpResponse::Ok().json(pv_data)
        }
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
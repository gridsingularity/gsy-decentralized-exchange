use crate::db::asset_measurements_service::GetMeasurements;
use crate::db::DbRef;
use actix_web::web::{Json, Query};
use actix_web::{HttpResponse, Responder};
use codec::{Decode, Encode};
use gsy_offchain_primitives::db_api_schema::profiles::{
    BatteryMeasurementSchema, PVMeasurementSchema, SmartMeterMeasurementSchema,
    TransformerMeasurementSchema,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Encode, Decode, Clone)]
#[serde(untagged)]
pub enum AssetMeasurementInput {
    MeasurementPV(PVMeasurementSchema),
    MeasurementBattery(BatteryMeasurementSchema),
    MeasurementSmartMeter(SmartMeterMeasurementSchema),
    MeasurementTransformer(TransformerMeasurementSchema),
}

#[derive(Deserialize, Clone)]
pub struct AssetMeasurementParameters {
    community_uuid: String,
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
        match db
            .get_ref()
            .pv_measurements()
            .insert_measurements(pv_data)
            .await
        {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    if battery_data.len() > 0 {
        match db
            .get_ref()
            .battery_measurements()
            .insert_measurements(battery_data)
            .await
        {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    if smart_meter_data.len() > 0 {
        match db
            .get_ref()
            .smart_meter_measurements()
            .insert_measurements(smart_meter_data)
            .await
        {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    if transformer_data.len() > 0 {
        match db
            .get_ref()
            .transformer_measurements()
            .insert_measurements(transformer_data)
            .await
        {
            Ok(_ids) => (),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        };
    }

    HttpResponse::Ok().finish()
}

async fn get_asset_measurements_for_type<
    T: Send + Sync + Serialize + 'static + serde::de::DeserializeOwned + std::fmt::Debug,
>(
    db_document: &(dyn GetMeasurements<T> + Sync),
    params: Query<AssetMeasurementParameters>,
) -> HttpResponse {
    match db_document
        .get_measurements(params.area_uuid.clone(), params.start_time, params.end_time)
        .await
    {
        Ok(pv_data) => {
            println!("{:?}", pv_data);
            HttpResponse::Ok().json(pv_data)
        }
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn get_asset_measurements(
    db: DbRef,
    params: Query<AssetMeasurementParameters>,
) -> HttpResponse {
    let markets = match db
        .get_ref()
        .markets()
        .get_community_market(params.community_uuid.clone(), None, None)
        .await
    {
        Ok(markets) if !markets.is_empty() => markets,
        _ => return HttpResponse::NotFound().finish(),
    };
    let _first_market = markets.first().unwrap();
    let area_type = _first_market
        .community_areas
        .iter()
        .find(|area| area.area_uuid == params.area_uuid)
        .unwrap()
        .area_type
        .clone();
    if area_type == "PV" {
        get_asset_measurements_for_type(&db.get_ref().pv_measurements(), params.clone()).await
    } else if area_type == "SmartMeter" {
        get_asset_measurements_for_type(&db.get_ref().smart_meter_measurements(), params.clone())
            .await
    } else if area_type == "Battery" {
        get_asset_measurements_for_type(&db.get_ref().battery_measurements(), params.clone()).await
    } else if area_type == "Transformer" {
        get_asset_measurements_for_type(&db.get_ref().transformer_measurements(), params.clone())
            .await
    } else {
        HttpResponse::NotImplemented().body(format!(
            "Measurements for area type '{}' not implemented yet",
            area_type
        ))
    }
}

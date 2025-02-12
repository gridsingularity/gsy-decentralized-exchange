use actix_web::{web::Json, HttpResponse, Responder, web::Query};
use crate::db::DbRef;
use gsy_offchain_primitives::db_api_schema::profiles::{MeasurementSchema, ForecastSchema};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProfilesParameters {
    area_uuid: Option<String>,
    start_time: Option<u32>,
    end_time: Option<u32>,
}

pub async fn post_measurements(
    measurements: Json<Vec<MeasurementSchema>>,
    db: DbRef,
) -> impl Responder {
    match db.get_ref().measurements().insert_measurements(measurements.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

pub async fn get_measurements(db: DbRef, query_params: Query<ProfilesParameters>) -> impl Responder {
    let measurements_service = db.get_ref().measurements();
    match measurements_service.filter_measurements(
            query_params.area_uuid.clone(),
            query_params.start_time,
            query_params.end_time).await {
        Ok(measurements) => HttpResponse::Ok().json(measurements),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        },
    }
}

pub async fn post_forecasts(
    forecasts: Json<Vec<ForecastSchema>>,
    db: DbRef,
) -> impl Responder {
    match db.get_ref().forecasts().insert_forecasts(forecasts.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

pub async fn get_forecasts(db: DbRef, query_params: Query<ProfilesParameters>) -> impl Responder {
    let forecasts_service = db.get_ref().forecasts();
    match forecasts_service.filter_forecasts(
            query_params.area_uuid.clone(),
            query_params.start_time,
            query_params.end_time).await {
        Ok(measurements) => HttpResponse::Ok().json(measurements),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        },
    }
}
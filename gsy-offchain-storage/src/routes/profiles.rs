use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MeasurementPointQuery {
    asset_name: Option<String>,
    #[serde(rename = "type")]
    point_type: Option<MeasurementPointType>,
}

pub async fn post_measurement_points(
    points: Json<Vec<MeasurementPointSchema>>,
    db: DbRef,
) -> impl Responder {
    match db
        .get_ref()
        .measurement_points()
        .insert_points(points.to_vec())
        .await
    {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_measurement_points(
    db: DbRef,
    query: Query<MeasurementPointQuery>,
) -> impl Responder {
    match db
        .get_ref()
        .measurement_points()
        .filter_points(query.asset_name.clone(), query.point_type.clone())
        .await
    {
        Ok(points) => HttpResponse::Ok().json(points),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[derive(Deserialize)]
pub struct TimeseriesQuery {
    measurement_point: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
}

pub async fn post_timeseries(
    timeseries: Json<Vec<TimeseriesSchema>>,
    db: DbRef,
) -> impl Responder {
    match db
        .get_ref()
        .timeseries()
        .insert_values(timeseries.to_vec())
        .await
    {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_timeseries(db: DbRef, query: Query<TimeseriesQuery>) -> impl Responder {
    match db
        .get_ref()
        .timeseries()
        .filter_values(
            query.measurement_point.clone(),
            query.start_time.clone(),
            query.end_time.clone(),
        )
        .await
    {
        Ok(values) => HttpResponse::Ok().json(values),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use anyhow::Result;
use primitives::db_api_schema::profiles::{
    FlowDirection, ForecastSchema, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use primitives::utils::timestamp_to_string_with_padding;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct ProfilesParameters {
    facility_id: Option<String>,
    start_time: Option<u64>,
    end_time: Option<u64>,
}

fn profile_measurement_id(
    point_type: MeasurementPointType,
    community_uuid: &str,
    facility_id: &str,
) -> String {
    let prefix = match point_type {
        MeasurementPointType::Measurement => "measurement",
        MeasurementPointType::Forecast => "forecast",
    };
    format!("{prefix}:{community_uuid}:{facility_id}")
}

fn parse_timeseries_timestamp(timestamp: &str) -> Option<u64> {
    timestamp.parse::<u64>().ok()
}

fn flow_direction(value: f64) -> FlowDirection {
    if value >= 0.0 {
        FlowDirection::Import
    } else {
        FlowDirection::Export
    }
}

fn measurement_point_from_measurement(measurement: &MeasurementSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Measurement,
            measurement.community_uuid.as_str(),
            measurement.facility_id.as_str(),
        ),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(measurement.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: measurement.facility_id.clone(),
        datasource_name: Some(measurement.community_uuid.clone()),
    }
}

fn measurement_timeseries(measurement: &MeasurementSchema) -> TimeseriesSchema {
    TimeseriesSchema {
        measurement_point: profile_measurement_id(
            MeasurementPointType::Measurement,
            measurement.community_uuid.as_str(),
            measurement.facility_id.as_str(),
        ),
        timestamp: timestamp_to_string_with_padding(measurement.time_slot),
        value: measurement.energy_kwh,
    }
}

fn measurement_point_from_forecast(forecast: &ForecastSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Forecast,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Forecast,
            forecast.community_uuid.as_str(),
            forecast.facility_id.as_str(),
        ),
        property_measured: "energy_forecast".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(forecast.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: forecast.facility_id.clone(),
        datasource_name: Some(forecast.community_uuid.clone()),
    }
}

fn forecast_timeseries(forecast: &ForecastSchema) -> TimeseriesSchema {
    TimeseriesSchema {
        measurement_point: profile_measurement_id(
            MeasurementPointType::Forecast,
            forecast.community_uuid.as_str(),
            forecast.facility_id.as_str(),
        ),
        timestamp: timestamp_to_string_with_padding(forecast.time_slot),
        value: forecast.energy_kwh,
    }
}

async fn fetch_profile_values(
    db: &crate::db::DatabaseWrapper,
    point_type: MeasurementPointType,
    facility_id: Option<String>,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<Vec<(MeasurementPointSchema, TimeseriesSchema)>> {
    let points = db
        .measurement_points()
        .filter_points(facility_id, Some(point_type))
        .await?;
    let points_by_id = points
        .into_iter()
        .map(|point| (point.measurement_id.clone(), point))
        .collect::<HashMap<_, _>>();

    let values = db
        .timeseries()
        .filter_values(
            None,
            start_time.map(timestamp_to_string_with_padding),
            end_time.map(timestamp_to_string_with_padding),
        )
        .await?;

    Ok(values
        .into_iter()
        .filter_map(|value| {
            let point = points_by_id.get(&value.measurement_point)?;
            Some((point.clone(), value))
        })
        .collect())
}

pub async fn post_measurements(
    measurements: Json<Vec<MeasurementSchema>>,
    db: DbRef,
) -> impl Responder {
    let points = measurements
        .iter()
        .map(measurement_point_from_measurement)
        .collect::<Vec<_>>();
    let values = measurements
        .iter()
        .map(measurement_timeseries)
        .collect::<Vec<_>>();

    let result = async {
        db.get_ref()
            .measurement_points()
            .insert_points(points)
            .await?;
        db.get_ref().timeseries().insert_values(values).await
    }
    .await;

    match result {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_measurements(
    db: DbRef,
    query_params: Query<ProfilesParameters>,
) -> impl Responder {
    match fetch_profile_values(
        db.get_ref(),
        MeasurementPointType::Measurement,
        query_params.facility_id.clone(),
        query_params.start_time,
        query_params.end_time,
    )
    .await
    {
        Ok(values) => HttpResponse::Ok().json(
            values
                .into_iter()
                .filter_map(|(point, value)| {
                    let time_slot = parse_timeseries_timestamp(value.timestamp.as_str())?;
                    Some(MeasurementSchema {
                        facility_id: point.asset_name,
                        community_uuid: point.datasource_name.unwrap_or_default(),
                        time_slot,
                        creation_time: time_slot,
                        energy_kwh: value.value,
                    })
                })
                .collect::<Vec<_>>(),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_forecasts(forecasts: Json<Vec<ForecastSchema>>, db: DbRef) -> impl Responder {
    let points = forecasts
        .iter()
        .map(measurement_point_from_forecast)
        .collect::<Vec<_>>();
    let values = forecasts
        .iter()
        .map(forecast_timeseries)
        .collect::<Vec<_>>();

    let result = async {
        db.get_ref()
            .measurement_points()
            .insert_points(points)
            .await?;
        db.get_ref().timeseries().insert_values(values).await
    }
    .await;

    match result {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_forecasts(db: DbRef, query_params: Query<ProfilesParameters>) -> impl Responder {
    match fetch_profile_values(
        db.get_ref(),
        MeasurementPointType::Forecast,
        query_params.facility_id.clone(),
        query_params.start_time,
        query_params.end_time,
    )
    .await
    {
        Ok(values) => HttpResponse::Ok().json(
            values
                .into_iter()
                .filter_map(|(point, value)| {
                    let time_slot = parse_timeseries_timestamp(value.timestamp.as_str())?;
                    Some(ForecastSchema {
                        facility_id: point.asset_name,
                        community_uuid: point.datasource_name.unwrap_or_default(),
                        time_slot,
                        creation_time: time_slot,
                        energy_kwh: value.value,
                        confidence: 1.0,
                    })
                })
                .collect::<Vec<_>>(),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

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

pub async fn post_timeseries(timeseries: Json<Vec<TimeseriesSchema>>, db: DbRef) -> impl Responder {
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

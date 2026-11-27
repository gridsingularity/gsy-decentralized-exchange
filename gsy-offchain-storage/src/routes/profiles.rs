use crate::db::measurements_service::{flow_direction, insert_measurements};
use crate::db::DbRef;
use crate::routes::parse_time_range;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use anyhow::Result;
use chrono::{DateTime, Utc};
use primitives::db_api_schema::profiles::{
    ForecastSchema, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use primitives::ewds::dto::TimeseriesDto;
use primitives::utils::{profile_measurement_id, validate_offchain_actor_id};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct ProfilesParameters {
    #[serde(alias = "facilityId", alias = "areaUuid")]
    facility_id: Option<String>,
    #[serde(alias = "communityUuid")]
    community_uuid: Option<String>,
    #[serde(alias = "startTime")]
    start_time: Option<String>,
    #[serde(alias = "endTime")]
    end_time: Option<String>,
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
        timestamp: forecast.time_slot,
        value: forecast.energy_kwh,
    }
}

async fn fetch_profile_values(
    db: &crate::db::DatabaseWrapper,
    point_type: MeasurementPointType,
    facility_id: Option<String>,
    community_uuid: Option<String>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
) -> Result<Vec<(MeasurementPointSchema, TimeseriesSchema)>> {
    let points = db
        .measurement_points()
        .filter_points(facility_id, Some(point_type))
        .await?;
    let points_by_id = points
        .into_iter()
        .filter(|point| {
            community_uuid
                .as_ref()
                .map_or(true, |id| point.datasource_name.as_ref() == Some(id))
        })
        .map(|point| (point.measurement_id.clone(), point))
        .collect::<HashMap<_, _>>();

    let values = db
        .timeseries()
        .filter_values(None, start_time, end_time)
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
    let mut measurements = measurements.into_inner();
    for measurement in &mut measurements {
        measurement.facility_id = match validate_offchain_actor_id(&measurement.facility_id) {
            Ok(id) => id,
            Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
        };
        measurement.community_uuid = match validate_offchain_actor_id(&measurement.community_uuid) {
            Ok(id) => id,
            Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
        };
    }
    match insert_measurements(db.get_ref(), &measurements).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_measurements(
    db: DbRef,
    query_params: Query<ProfilesParameters>,
) -> impl Responder {
    let facility_id = match query_params
        .facility_id
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
    {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    let community_uuid = match query_params
        .community_uuid
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
    {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    let (start_time, end_time) = match parse_time_range(
        query_params.start_time.as_deref(),
        query_params.end_time.as_deref(),
    ) {
        Ok(range) => range,
        Err(response) => return response,
    };
    match fetch_profile_values(
        db.get_ref(),
        MeasurementPointType::Measurement,
        facility_id,
        community_uuid,
        start_time,
        end_time,
    )
    .await
    {
        Ok(values) => HttpResponse::Ok().json(
            values
                .into_iter()
                .map(|(point, value)| MeasurementSchema {
                    facility_id: point.asset_name,
                    community_uuid: point.datasource_name.unwrap_or_default(),
                    time_slot: value.timestamp,
                    creation_time: value.timestamp,
                    energy_kwh: value.value,
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
    let mut forecasts = forecasts.into_inner();
    for forecast in &mut forecasts {
        forecast.facility_id = match validate_offchain_actor_id(&forecast.facility_id) {
            Ok(id) => id,
            Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
        };
        forecast.community_uuid = match validate_offchain_actor_id(&forecast.community_uuid) {
            Ok(id) => id,
            Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
        };
    }
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
    let facility_id = match query_params
        .facility_id
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
    {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    let community_uuid = match query_params
        .community_uuid
        .as_deref()
        .map(validate_offchain_actor_id)
        .transpose()
    {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    let (start_time, end_time) = match parse_time_range(
        query_params.start_time.as_deref(),
        query_params.end_time.as_deref(),
    ) {
        Ok(range) => range,
        Err(response) => return response,
    };
    match fetch_profile_values(
        db.get_ref(),
        MeasurementPointType::Forecast,
        facility_id,
        community_uuid,
        start_time,
        end_time,
    )
    .await
    {
        Ok(values) => HttpResponse::Ok().json(
            values
                .into_iter()
                .map(|(point, value)| ForecastSchema {
                    facility_id: point.asset_name,
                    community_uuid: point.datasource_name.unwrap_or_default(),
                    time_slot: value.timestamp,
                    creation_time: value.timestamp,
                    energy_kwh: value.value,
                    confidence: 1.0,
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
        .insert_points(points.into_inner())
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

pub async fn post_timeseries(timeseries: Json<Vec<TimeseriesDto>>, db: DbRef) -> impl Responder {
    let values = match timeseries
        .into_inner()
        .into_iter()
        .map(TimeseriesSchema::try_from)
        .collect::<Result<Vec<_>>>()
    {
        Ok(values) => values,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    match db.get_ref().timeseries().insert_values(values).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_timeseries(db: DbRef, query: Query<TimeseriesQuery>) -> impl Responder {
    let (start_time, end_time) =
        match parse_time_range(query.start_time.as_deref(), query.end_time.as_deref()) {
            Ok(range) => range,
            Err(response) => return response,
        };
    match db
        .get_ref()
        .timeseries()
        .filter_values(query.measurement_point.clone(), start_time, end_time)
        .await
    {
        Ok(values) => HttpResponse::Ok().json(
            values
                .into_iter()
                .map(TimeseriesDto::from)
                .collect::<Vec<_>>(),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

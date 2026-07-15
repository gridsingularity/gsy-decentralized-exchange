use crate::db::DbRef;
use actix_web::{HttpResponse, Responder, web::Query};
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct GuaranteesOfOriginParams {
    start_time: Option<u32>,
    end_time: Option<u32>,
}

/// Asset measurement as reported by the guarantees-of-origin-measurements
/// endpoint.
#[derive(Serialize, Debug)]
pub struct AssetMeasurementSchema {
    pub asset_id: String,
    pub community_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy_kwh: f64,
}

impl From<MeasurementSchema> for AssetMeasurementSchema {
    fn from(measurement: MeasurementSchema) -> Self {
        AssetMeasurementSchema {
            asset_id: measurement.area_uuid,
            community_id: measurement.community_uuid,
            time_slot: measurement.time_slot,
            creation_time: measurement.creation_time,
            energy_kwh: measurement.energy_kwh,
        }
    }
}

#[tracing::instrument(
    name = "Retrieve asset measurements for guarantees of origin",
    skip(db)
)]
pub async fn get_guarantees_of_origin(
    db: DbRef,
    query_params: Query<GuaranteesOfOriginParams>,
) -> impl Responder {
    match db
        .get_ref()
        .asset_measurements()
        .get_asset_measurements(query_params.start_time, query_params.end_time)
        .await
    {
        Ok(measurements) => HttpResponse::Ok().json(
            measurements
                .into_iter()
                .map(AssetMeasurementSchema::from)
                .collect::<Vec<AssetMeasurementSchema>>(),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

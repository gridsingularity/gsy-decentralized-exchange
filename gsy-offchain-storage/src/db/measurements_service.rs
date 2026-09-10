use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use mongodb::bson::{Bson, doc};
use std::collections::HashMap;

pub struct MeasurementsService(pub(crate) Coll<MeasurementSchema>);

impl MeasurementsService {
    #[tracing::instrument(name = "Fetching measurements from database for one area", skip(self))]
    pub async fn filter_measurements(
        &self,
        area_uuid: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<MeasurementSchema>> {
        let mut filter_params = doc! {};
        if let Some(area_uuid) = &area_uuid {
            filter_params.insert("area_uuid", area_uuid.clone());
        }
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |measurement| {
                area_uuid
                    .as_ref()
                    .is_none_or(|area_uuid| &measurement.area_uuid == area_uuid)
                    && in_time_window(measurement.time_slot, start_time, end_time)
            })
            .await
    }

    #[tracing::instrument(
        name = "Saving measurements to database",
        skip(self, measurements),
        fields(
        measurements = ?measurements
        )
    )]
    pub async fn insert_measurements(
        &self,
        measurements: Vec<MeasurementSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        // NOTE: MeasurementSchema has no `_id`, so the fabricated in-memory ids
        // (area_uuid) diverge from Mongo's generated ObjectIds and are not unique.
        self.0
            .insert_many(measurements, |measurement| {
                Bson::String(measurement.area_uuid.clone())
            })
            .await
    }
}

impl From<&DatabaseWrapper> for MeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        MeasurementsService(db.coll("measurements", |store| store.measurements.clone()))
    }
}

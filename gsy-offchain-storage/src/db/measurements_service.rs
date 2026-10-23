use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use mongodb::bson::doc;

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

    /// Upsert every measurement keyed on `(area_hash, time_slot)`: a measurement for an area/slot
    /// already stored replaces the stored row, otherwise it is inserted. This makes the client's
    /// re-post of its whole look-back window on every tick idempotent instead of duplicating rows.
    ///
    /// A measurement equal to the stored row apart from `creation_time` is skipped, so an
    /// unchanged re-post leaves the stored row untouched and `creation_time` keeps meaning "when
    /// this value was stored" (the certificates expose it as `measurement_recorded_at` and sort
    /// on it). A real change, e.g. late data turning a `missing` row into a `complete` one,
    /// replaces the row, `creation_time` included.
    ///
    /// Rows are written in input order, so of two rows with the same key in one batch the later
    /// one wins. Returns the number of rows written (inserted or changed); skipped rows are not
    /// counted.
    #[tracing::instrument(
        name = "Saving measurements to database",
        skip(self, measurements),
        fields(
        measurements = ?measurements
        )
    )]
    pub async fn insert_measurements(&self, measurements: Vec<MeasurementSchema>) -> Result<usize> {
        let mut written = 0usize;
        for measurement in measurements {
            let mongo_filter = doc! {
                "area_hash": &measurement.area_hash,
                "time_slot": measurement.time_slot as i64,
            };
            let area_hash = measurement.area_hash.clone();
            let time_slot = measurement.time_slot;
            let same_key = |existing: &MeasurementSchema| {
                existing.area_hash == area_hash && existing.time_slot == time_slot
            };
            if let Some(mut stored) = self.0.find_one(mongo_filter.clone(), same_key).await? {
                // Compare as if the stored row carried the incoming `creation_time`, so a re-post
                // that differs only in that field counts as unchanged.
                stored.creation_time = measurement.creation_time;
                if stored == measurement {
                    continue;
                }
            }
            self.0
                .replace_one_upsert(mongo_filter, measurement, same_key)
                .await?;
            written += 1;
        }
        Ok(written)
    }
}

impl From<&DatabaseWrapper> for MeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        MeasurementsService(db.coll("measurements", |store| store.measurements.clone()))
    }
}

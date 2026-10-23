use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use mongodb::bson::doc;

/// Read-only view over the `measurements` collection that reports the
/// measurements of individual assets (smart meters, PVs, batteries, heat
/// pumps) across all communities, for guarantees-of-origin reporting.
pub struct AssetMeasurementsService(pub(crate) Coll<MeasurementSchema>);

impl AssetMeasurementsService {
    #[tracing::instrument(name = "Fetching measurements of all assets from database", skip(self))]
    pub async fn get_asset_measurements(
        &self,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<MeasurementSchema>> {
        // Community-level entries (posted for the community itself rather than
        // an individual asset) are stored with `area_uuid == community_uuid`.
        // These are excluded so only per-asset measurements are reported.
        let mut filter_params = doc! {
            "$expr": { "$ne": ["$area_uuid", "$community_uuid"] }
        };
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |measurement| {
                measurement.area_uuid != measurement.community_uuid
                    && in_time_window(measurement.time_slot, start_time, end_time)
            })
            .await
    }
}

impl From<&DatabaseWrapper> for AssetMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        AssetMeasurementsService(db.coll("measurements", |store| store.measurements.clone()))
    }
}

use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use mongodb::bson::{Bson, doc};
use std::collections::HashMap;

pub struct ForecastsService(pub(crate) Coll<ForecastSchema>);

impl ForecastsService {
    #[tracing::instrument(name = "Fetching forecasts from database for one area", skip(self))]
    pub async fn filter_forecasts(
        &self,
        area_uuid: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<ForecastSchema>> {
        let mut filter_params = doc! {};
        if let Some(area_uuid) = &area_uuid {
            filter_params.insert("area_uuid", area_uuid.clone());
        }
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |forecast| {
                area_uuid
                    .as_ref()
                    .is_none_or(|area_uuid| &forecast.area_uuid == area_uuid)
                    && in_time_window(forecast.time_slot, start_time, end_time)
            })
            .await
    }

    #[tracing::instrument(
        name = "Saving forecasts to database",
        skip(self, forecasts),
        fields(
        forecasts = ?forecasts
        )
    )]
    pub async fn insert_forecasts(
        &self,
        forecasts: Vec<ForecastSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        // NOTE: ForecastSchema has no `_id`, so the fabricated in-memory ids
        // (area_uuid) diverge from Mongo's generated ObjectIds and are not unique.
        self.0
            .insert_many(forecasts, |forecast| Bson::String(forecast.area_uuid.clone()))
            .await
    }
}

impl From<&DatabaseWrapper> for ForecastsService {
    fn from(db: &DatabaseWrapper) -> Self {
        ForecastsService(db.coll("forecasts", |store| store.forecasts.clone()))
    }
}

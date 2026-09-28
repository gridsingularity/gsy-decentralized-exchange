use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use mongodb::bson::doc;

pub struct ForecastsService(pub(crate) Coll<ForecastSchema>);

impl ForecastsService {
    #[tracing::instrument(name = "Fetching forecasts from database for one area", skip(self))]
    pub async fn filter_forecasts(
        &self,
        area_uuid: Option<String>,
        community_uuid: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<ForecastSchema>> {
        let mut filter_params = doc! {};
        if let Some(area_uuid) = &area_uuid {
            filter_params.insert("area_uuid", area_uuid.clone());
        }
        if let Some(community_uuid) = &community_uuid {
            filter_params.insert("community_uuid", community_uuid.clone());
        }
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |forecast| {
                area_uuid
                    .as_ref()
                    .is_none_or(|area_uuid| &forecast.area_uuid == area_uuid)
                    && community_uuid
                        .as_ref()
                        .is_none_or(|community_uuid| &forecast.community_uuid == community_uuid)
                    && in_time_window(forecast.time_slot, start_time, end_time)
            })
            .await
    }

    /// Upsert every forecast keyed on `(area_uuid, time_slot)`: a forecast for an area/slot
    /// already stored is overwritten in place, otherwise it is inserted. This makes the
    /// hourly re-ingest of a rolling day-ahead window idempotent instead of duplicating rows.
    #[tracing::instrument(
        name = "Saving forecasts to database",
        skip(self, forecasts),
        fields(
        forecasts = ?forecasts
        )
    )]
    pub async fn insert_forecasts(&self, forecasts: Vec<ForecastSchema>) -> Result<usize> {
        let mut upserted = 0usize;
        for forecast in forecasts {
            let mongo_filter = doc! {
                "area_uuid": &forecast.area_uuid,
                "time_slot": forecast.time_slot as i64,
            };
            let area_uuid = forecast.area_uuid.clone();
            let time_slot = forecast.time_slot;
            self.0
                .replace_one_upsert(mongo_filter, forecast, |existing| {
                    existing.area_uuid == area_uuid && existing.time_slot == time_slot
                })
                .await?;
            upserted += 1;
        }
        Ok(upserted)
    }
}

impl From<&DatabaseWrapper> for ForecastsService {
    fn from(db: &DatabaseWrapper) -> Self {
        ForecastsService(db.coll("forecasts", |store| store.forecasts.clone()))
    }
}

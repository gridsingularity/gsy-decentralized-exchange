use crate::db::DatabaseWrapper;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;


/// this function will call after connected to database
pub async fn init_forecasts(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block

    let controller = db.forecasts();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index).await?;
    Ok(())
}

#[repr(transparent)]
pub struct ForecastsService(pub Collection<ForecastSchema>);

impl ForecastsService {
    #[tracing::instrument(name = "Fetching forecasts from database for one area", skip(self))]
    pub async fn filter_forecasts(
            &self,
            area_uuid: Option<String>,
            start_time: Option<u32>,
            end_time: Option<u32>) -> Result<Vec<ForecastSchema>> {
        let mut filter_params = doc! {};
        if area_uuid.is_some() { filter_params.insert("area_uuid", area_uuid.unwrap()); }
        if start_time.is_some() { filter_params.insert("time_slot", doc! {"$gte": start_time.unwrap()} ); } 
        if end_time.is_some() {
            if start_time.is_some() {
                filter_params.insert("time_slot", 
                                     doc! {"$gte": start_time.unwrap(), "$lte": end_time.unwrap()});
            }
            else {
                filter_params.insert("time_slot", doc! {"$lte": end_time.unwrap()});
            }
        }
        let mut cursor = self.0.find(filter_params).await.unwrap();
        let mut result: Vec<ForecastSchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(document) => {
                    result.push(document);
                }
                _ => {
                    break;
                }
            }
        }
        Ok(result)
    }

    #[tracing::instrument(
        name = "Saving forecasts to database",
        skip(self, forecasts),
        fields(
        forecasts = ?forecasts
        )
    )]
    pub async fn insert_forecasts(&self, forecasts: Vec<ForecastSchema>) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(forecasts).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

impl From<&DatabaseWrapper> for ForecastsService {
    fn from(db: &DatabaseWrapper) -> Self {
        ForecastsService(db.collection("forecasts"))
    }
}

impl Deref for ForecastsService {
    type Target = Collection<ForecastSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

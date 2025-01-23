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
    pub async fn get_all_forecasts_for_area(&self, area_uuid: String) -> Result<Vec<ForecastSchema>> {
        let mut cursor = self.0.find(doc! {"area_uuid": area_uuid}).await.unwrap();
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
        ForecastsService(db.collection("orders"))
    }
}

impl Deref for ForecastsService {
    type Target = Collection<ForecastSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

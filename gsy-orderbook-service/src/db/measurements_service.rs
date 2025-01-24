use crate::db::DatabaseWrapper;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;


/// this function will call after connected to database
pub async fn init_measurements(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block

    let controller = db.measurements();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index).await?;
    Ok(())
}

#[repr(transparent)]
pub struct MeasurementsService(pub Collection<MeasurementSchema>);

impl MeasurementsService {
    #[tracing::instrument(name = "Fetching measurements from database for one area", skip(self))]
    pub async fn filter_measurements(
            &self,
            area_uuid: Option<String>,
            start_time: Option<u32>,
            end_time: Option<u32>) -> Result<Vec<MeasurementSchema>> {
        let mut filter_params = doc! {};
        if area_uuid.is_some() { filter_params.insert("area_uuid", area_uuid.unwrap()); }
        if start_time.is_some() { filter_params.insert("time_slot", doc! {"$gte": start_time.unwrap()} ); }
        if end_time.is_some() { filter_params.insert("time_slot", doc! {"$lte": end_time.unwrap()}); }
        let mut cursor = self.0.find(filter_params).await.unwrap();
        let mut result: Vec<MeasurementSchema> = Vec::new();
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
        name = "Saving measurements to database",
        skip(self, measurements),
        fields(
        measurements = ?measurements
        )
    )]
    pub async fn insert_measurements(&self, measurements: Vec<MeasurementSchema>) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

impl From<&DatabaseWrapper> for MeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        MeasurementsService(db.collection("orders"))
    }
}

impl Deref for MeasurementsService {
    type Target = Collection<MeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

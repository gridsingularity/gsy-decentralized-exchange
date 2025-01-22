use crate::db::DatabaseWrapper;
use crate::db::schema::MeasurementSchema;
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
    pub async fn get_all_measurements_for_area(&self, area_uuid: String) -> Result<Vec<MeasurementSchema>> {
        let mut cursor = self.0.find(doc! {"area_uuid": area_uuid}).await.unwrap();
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

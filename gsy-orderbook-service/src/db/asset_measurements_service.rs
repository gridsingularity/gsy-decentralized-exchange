use crate::db::{create_filter_params_with_start_end_time, DatabaseWrapper};
use anyhow::Result;
use futures::stream::TryStreamExt;
use gsy_offchain_primitives::db_api_schema::profiles::{
    BatteryMeasurementSchema, PVMeasurementSchema, SmartMeterMeasurementSchema,
    TransformerMeasurementSchema,
};
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use serde::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

/// this function will call after connected to database
pub async fn init_pv_measurements(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.pv_measurements();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index.clone()).await?;
    Ok(())
}

pub async fn init_smart_meter_measurements(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.smart_meter_measurements();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index.clone()).await?;
    Ok(())
}

pub async fn init_battery_measurements(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.battery_measurements();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index.clone()).await?;
    Ok(())
}

pub async fn init_transformer_measurements(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.transformer_measurements();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index.clone()).await?;
    Ok(())
}

#[derive(Clone)]
pub struct AssetMeasurementCollection<T: Send + Sync + Serialize>(Collection<T>);

#[async_trait::async_trait]
pub trait GetMeasurements<T: Send + Sync + Serialize + 'static> {
    fn get_database(&self) -> AssetMeasurementCollection<T>;

    async fn insert_measurements(&self, measurements: Vec<T>) -> Result<HashMap<usize, Bson>> {
        match self.get_database().insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    async fn get_measurements(
        &self,
        area_uuid: String,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned + Send + Sync,
    {
        let mut filter_params = create_filter_params_with_start_end_time(
            "metadata.time_slot".to_string(),
            start_time,
            end_time,
        );
        filter_params.insert("metadata.area_uuid".to_string(), area_uuid);

        let mut results: Vec<T> = Vec::new();
        let mut cursor = self.get_database().find(filter_params).await?;
        while let Some(doc) = cursor.try_next().await? {
            results.push(doc);
        }
        Ok(results)
    }
}

impl<T: Send + Sync + Serialize> Deref for AssetMeasurementCollection<T> {
    type Target = Collection<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Send + Sync + Serialize> DerefMut for AssetMeasurementCollection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(transparent)]
pub struct PVMeasurementsService(pub AssetMeasurementCollection<PVMeasurementSchema>);

impl From<&DatabaseWrapper> for PVMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        PVMeasurementsService(AssetMeasurementCollection(db.collection("pvmeasurements")))
    }
}

impl Deref for PVMeasurementsService {
    type Target = AssetMeasurementCollection<PVMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait::async_trait]
impl GetMeasurements<PVMeasurementSchema> for PVMeasurementsService {
    fn get_database(&self) -> AssetMeasurementCollection<PVMeasurementSchema> {
        self.0.clone()
    }
}

#[repr(transparent)]
pub struct SmartMeterMeasurementsService(
    pub AssetMeasurementCollection<SmartMeterMeasurementSchema>,
);

impl From<&DatabaseWrapper> for SmartMeterMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        SmartMeterMeasurementsService(AssetMeasurementCollection(
            db.collection("smartmetermeasurements"),
        ))
    }
}

impl Deref for SmartMeterMeasurementsService {
    type Target = AssetMeasurementCollection<SmartMeterMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait::async_trait]
impl GetMeasurements<SmartMeterMeasurementSchema> for SmartMeterMeasurementsService {
    fn get_database(&self) -> AssetMeasurementCollection<SmartMeterMeasurementSchema> {
        self.0.clone()
    }
}

#[repr(transparent)]
pub struct BatteryMeasurementsService(pub AssetMeasurementCollection<BatteryMeasurementSchema>);

impl From<&DatabaseWrapper> for BatteryMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        BatteryMeasurementsService(AssetMeasurementCollection(
            db.collection("batterymeasurements"),
        ))
    }
}

impl Deref for BatteryMeasurementsService {
    type Target = AssetMeasurementCollection<BatteryMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait::async_trait]
impl GetMeasurements<BatteryMeasurementSchema> for BatteryMeasurementsService {
    fn get_database(&self) -> AssetMeasurementCollection<BatteryMeasurementSchema> {
        self.0.clone()
    }
}

#[repr(transparent)]
pub struct TransformerMeasurementsService(
    pub AssetMeasurementCollection<TransformerMeasurementSchema>,
);

impl From<&DatabaseWrapper> for TransformerMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        TransformerMeasurementsService(AssetMeasurementCollection(
            db.collection("transformermeasurements"),
        ))
    }
}

impl Deref for TransformerMeasurementsService {
    type Target = AssetMeasurementCollection<TransformerMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait::async_trait]
impl GetMeasurements<TransformerMeasurementSchema> for TransformerMeasurementsService {
    fn get_database(&self) -> AssetMeasurementCollection<TransformerMeasurementSchema> {
        self.0.clone()
    }
}

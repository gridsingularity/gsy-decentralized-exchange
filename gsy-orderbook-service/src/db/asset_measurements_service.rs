use crate::db::DatabaseWrapper;
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::profiles::{BatteryMeasurementSchema, PVMeasurementSchema, SmartMeterMeasurementSchema, TransformerMeasurementSchema};
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

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


#[repr(transparent)]
pub struct PVMeasurementsService(pub Collection<PVMeasurementSchema>);

impl From<&DatabaseWrapper> for PVMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        PVMeasurementsService(db.collection("pvmeasurements"))
    }
}

impl Deref for PVMeasurementsService {
    type Target = Collection<PVMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PVMeasurementsService {
    pub async fn insert_measurements(
        &self,
        measurements: Vec<PVMeasurementSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

#[repr(transparent)]
pub struct SmartMeterMeasurementsService(pub Collection<SmartMeterMeasurementSchema>);

impl From<&DatabaseWrapper> for SmartMeterMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        SmartMeterMeasurementsService(db.collection("smartmetermeasurements"))
    }
}

impl Deref for SmartMeterMeasurementsService {
    type Target = Collection<SmartMeterMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SmartMeterMeasurementsService {
    pub async fn insert_measurements(
        &self,
        measurements: Vec<SmartMeterMeasurementSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

#[repr(transparent)]
pub struct BatteryMeasurementsService(pub Collection<BatteryMeasurementSchema>);

impl From<&DatabaseWrapper> for BatteryMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        BatteryMeasurementsService(db.collection("batterymeasurements"))
    }
}

impl Deref for BatteryMeasurementsService {
    type Target = Collection<BatteryMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl BatteryMeasurementsService {
    pub async fn insert_measurements(
        &self,
        measurements: Vec<BatteryMeasurementSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}


#[repr(transparent)]
pub struct TransformerMeasurementsService(pub Collection<TransformerMeasurementSchema>);

impl From<&DatabaseWrapper> for TransformerMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        TransformerMeasurementsService(db.collection("transformermeasurements"))
    }
}

impl Deref for TransformerMeasurementsService {
    type Target = Collection<TransformerMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TransformerMeasurementsService {
    pub async fn insert_measurements(
        &self,
        measurements: Vec<TransformerMeasurementSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

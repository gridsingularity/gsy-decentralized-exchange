use crate::db::DatabaseWrapper;
use gsy_offchain_primitives::db_api_schema::profiles::{
    PVMeasurementSchema, SmartMeterMeasurementSchema, BatteryMeasurementSchema};
use anyhow::Result;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;


/// this function will call after connected to database
pub async fn init_asset_measurements(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block
    let pv_controller = db.pv_measurements();
    let smart_meter_controller = db.smart_meter_measurements();
    let battery_controller = db.battery_measurements();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    pv_controller.create_index(index.clone()).await?;
    battery_controller.create_index(index.clone()).await?;
    smart_meter_controller.create_index(index.clone()).await?;
    Ok(())
}

#[repr(transparent)]
pub struct PVMeasurementsService(pub Collection<PVMeasurementSchema>);

impl From<&DatabaseWrapper> for PVMeasurementsService {
    fn from(db: &DatabaseWrapper) -> Self {
        PVMeasurementsService(db.collection("pv_measurements"))
    }
}

impl Deref for PVMeasurementsService {
    type Target = Collection<PVMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PVMeasurementsService {
    pub async fn insert_measurements(&self, measurements: Vec<PVMeasurementSchema>) -> Result<HashMap<usize, Bson>> {
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
        SmartMeterMeasurementsService(db.collection("smart_meter_measurements"))
    }
}

impl Deref for SmartMeterMeasurementsService {
    type Target = Collection<SmartMeterMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SmartMeterMeasurementsService {
    pub async fn insert_measurements(&self, measurements: Vec<SmartMeterMeasurementSchema>) -> Result<HashMap<usize, Bson>> {
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
        BatteryMeasurementsService(db.collection("battery_measurements"))
    }
}

impl Deref for BatteryMeasurementsService {
    type Target = Collection<BatteryMeasurementSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl BatteryMeasurementsService {
    pub async fn insert_measurements(&self, measurements: Vec<BatteryMeasurementSchema>) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(measurements).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

//! Measurements Storage service layer, per D3.2 §5.2.
//!
//! Two collections back this storage subcomponent:
//!   * `measurement_points` — metadata documents (one per logical
//!     measurement or forecast stream),
//!   * `timeseries` — the actual time-stamped values, referencing
//!     a parent `MeasurementPoint` via `measurement_point`.
//!
//! `asset_name` is indexed on the metadata collection;
//! `measurement_point` and `timestamp` are indexed on the timeseries
//! collection.

use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

pub async fn init_measurement_points(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.measurement_points();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"measurement_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(IndexModel::builder().keys(doc! {"asset_name": 1}).build())
        .await?;
    Ok(())
}

pub async fn init_timeseries(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.timeseries();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"measurement_point": 1, "timestamp": 1})
                .build(),
        )
        .await?;
    controller
        .create_index(IndexModel::builder().keys(doc! {"timestamp": 1}).build())
        .await?;
    Ok(())
}

#[repr(transparent)]
pub struct MeasurementPointService(pub Collection<MeasurementPointSchema>);

impl MeasurementPointService {
    #[tracing::instrument(name = "Inserting measurement points", skip(self, points))]
    pub async fn insert_points(
        &self,
        points: Vec<MeasurementPointSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        Ok(self.0.insert_many(points).await?.inserted_ids)
    }

    #[tracing::instrument(name = "Fetching measurement points", skip(self))]
    pub async fn filter_points(
        &self,
        asset_name: Option<String>,
        point_type: Option<MeasurementPointType>,
    ) -> Result<Vec<MeasurementPointSchema>> {
        let mut filter = doc! {};
        if let Some(asset_name) = asset_name {
            filter.insert("asset_name", asset_name);
        }
        if let Some(point_type) = point_type {
            filter.insert("type", mongodb::bson::to_bson(&point_type)?);
        }
        let mut cursor = self.0.find(filter).await?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
    }
}

impl From<&DatabaseWrapper> for MeasurementPointService {
    fn from(db: &DatabaseWrapper) -> Self {
        MeasurementPointService(db.collection("measurement_points"))
    }
}

impl Deref for MeasurementPointService {
    type Target = Collection<MeasurementPointSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct TimeseriesService(pub Collection<TimeseriesSchema>);

impl TimeseriesService {
    #[tracing::instrument(name = "Inserting timeseries", skip(self, points))]
    pub async fn insert_values(
        &self,
        points: Vec<TimeseriesSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        Ok(self.0.insert_many(points).await?.inserted_ids)
    }

    #[tracing::instrument(name = "Fetching timeseries values", skip(self))]
    pub async fn filter_values(
        &self,
        measurement_point: Option<String>,
        start_time: Option<String>,
        end_time: Option<String>,
    ) -> Result<Vec<TimeseriesSchema>> {
        let mut filter = doc! {};
        if let Some(point) = measurement_point {
            filter.insert("measurement_point", point);
        }
        match (start_time, end_time) {
            (Some(start), Some(end)) => {
                filter.insert("timestamp", doc! {"$gte": start, "$lte": end});
            }
            (Some(start), None) => {
                filter.insert("timestamp", doc! {"$gte": start});
            }
            (None, Some(end)) => {
                filter.insert("timestamp", doc! {"$lte": end});
            }
            (None, None) => {}
        }
        let mut cursor = self.0.find(filter).await?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
    }
}

impl From<&DatabaseWrapper> for TimeseriesService {
    fn from(db: &DatabaseWrapper) -> Self {
        TimeseriesService(db.collection("timeseries"))
    }
}

impl Deref for TimeseriesService {
    type Target = Collection<TimeseriesSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

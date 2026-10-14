use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use primitives::utils::timestamp_to_string_with_padding;
use std::collections::HashMap;
use std::ops::Deref;

pub(crate) fn profile_measurement_id(
    point_type: MeasurementPointType,
    community_uuid: &str,
    facility_id: &str,
) -> String {
    let prefix = match point_type {
        MeasurementPointType::Measurement => "measurement",
        MeasurementPointType::Forecast => "forecast",
    };
    format!("{prefix}:{community_uuid}:{facility_id}")
}

pub(crate) fn flow_direction(value: f64) -> FlowDirection {
    if value >= 0.0 {
        FlowDirection::Import
    } else {
        FlowDirection::Export
    }
}

fn measurement_point_from_measurement(measurement: &MeasurementSchema) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: profile_measurement_id(
            MeasurementPointType::Measurement,
            measurement.community_uuid.as_str(),
            measurement.facility_id.as_str(),
        ),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: flow_direction(measurement.energy_kwh),
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: measurement.facility_id.clone(),
        datasource_name: Some(measurement.community_uuid.clone()),
    }
}

fn measurement_timeseries(measurement: &MeasurementSchema) -> TimeseriesSchema {
    TimeseriesSchema {
        measurement_point: profile_measurement_id(
            MeasurementPointType::Measurement,
            measurement.community_uuid.as_str(),
            measurement.facility_id.as_str(),
        ),
        timestamp: timestamp_to_string_with_padding(measurement.time_slot),
        value: measurement.energy_kwh,
    }
}

pub async fn insert_measurements(
    db: &DatabaseWrapper,
    measurements: &[MeasurementSchema],
) -> Result<HashMap<usize, Bson>> {
    // A batch usually has many values per point, so each point is written once, in the state
    // of its last value.
    let points = measurements
        .iter()
        .map(|measurement| {
            let point = measurement_point_from_measurement(measurement);
            (point.measurement_id.clone(), point)
        })
        .collect::<HashMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let values = measurements
        .iter()
        .map(measurement_timeseries)
        .collect::<Vec<_>>();

    db.measurement_points().insert_points(points).await?;
    db.timeseries().insert_values(values).await
}

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
                .options(IndexOptions::builder().unique(true).build())
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
        let mut upserted_ids = HashMap::new();
        for (index, point) in points.into_iter().enumerate() {
            let measurement_id = point.measurement_id.clone();
            let point_doc = mongodb::bson::to_document(&point)?;
            let result = self
                .0
                .update_one(
                    doc! {"measurement_id": measurement_id.clone()},
                    doc! {"$set": point_doc},
                )
                .upsert(true)
                .await?;
            upserted_ids.insert(
                index,
                result
                    .upserted_id
                    .unwrap_or_else(|| Bson::String(measurement_id.clone())),
            );
        }
        Ok(upserted_ids)
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
        let mut upserted_ids = HashMap::new();
        for (index, point) in points.into_iter().enumerate() {
            let measurement_point = point.measurement_point.clone();
            let timestamp = point.timestamp.clone();
            let point_doc = mongodb::bson::to_document(&point)?;
            let result = self
                .0
                .update_one(
                    doc! {
                        "measurement_point": measurement_point.clone(),
                        "timestamp": timestamp.clone(),
                    },
                    doc! {"$set": point_doc},
                )
                .upsert(true)
                .await?;
            upserted_ids.insert(
                index,
                result.upserted_id.unwrap_or_else(|| {
                    Bson::String(format!("{}:{}", measurement_point, timestamp))
                }),
            );
        }
        Ok(upserted_ids)
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

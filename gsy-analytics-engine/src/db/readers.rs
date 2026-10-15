//! Read-only queries against the offchain-storage collections. Index creation is left to
//! offchain-storage; this module never writes to these collections.

use crate::kpi::DataRequirements;
use crate::mapping::{meter_readings, trade_record, usable_point_ids, MarketIndex, ReadingStats};
use crate::model::{CommunityRef, Dataset};
use crate::period::Window;
use anyhow::{Context, Result};
use futures::TryStreamExt;
use mongodb::bson::{doc, Document};
use mongodb::Database;
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::profiles::{MeasurementPointSchema, TimeseriesSchema};
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::utils::timestamp_to_string_with_padding;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

async fn find_all<T>(db: &Database, collection: &str, filter: Document) -> Result<Vec<T>>
where
    T: DeserializeOwned + Send + Sync,
{
    db.collection::<T>(collection)
        .find(filter)
        .await?
        .try_collect()
        .await
        .with_context(|| format!("Failed to read the '{}' collection", collection))
}

pub async fn load_communities(db: &Database) -> Result<Vec<CommunityRef>> {
    let communities: Vec<EnergyCommunitySchema> = find_all(db, "communities", doc! {}).await?;
    Ok(communities
        .into_iter()
        .map(|community| CommunityRef {
            community_id: community.community_id,
            community_name: community.community_name,
        })
        .collect())
}

/// Facility id → owner id.
pub async fn load_facility_owners(db: &Database) -> Result<HashMap<String, String>> {
    let facilities: Vec<FacilitySchema> = find_all(db, "facilities", doc! {}).await?;
    Ok(facilities
        .into_iter()
        .map(|facility| (facility.facility_id, facility.owner_id))
        .collect())
}

/// Trades whose delivery slot starts in the window.
pub async fn load_trades(db: &Database, window: &Window) -> Result<Vec<DbTradeSchema>> {
    find_all(
        db,
        "trades",
        doc! {"time_slot": {"$gte": window.start, "$lt": window.end}},
    )
    .await
}

pub async fn load_measurement_points(db: &Database) -> Result<Vec<MeasurementPointSchema>> {
    find_all(db, "measurement_points", doc! {"type": "Measurement"}).await
}

/// Values of the given points whose timestamp falls in the window.
/// Timestamps are zero-padded strings, so the range compares lexicographically.
pub async fn load_timeseries(
    db: &Database,
    window: &Window,
    measurement_points: &[String],
) -> Result<Vec<TimeseriesSchema>> {
    find_all(
        db,
        "timeseries",
        doc! {
            "measurement_point": {"$in": measurement_points},
            "timestamp": {
                "$gte": timestamp_to_string_with_padding(window.start as u64),
                "$lt": timestamp_to_string_with_padding(window.end as u64),
            },
        },
    )
    .await
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LoadStats {
    pub communities: usize,
    pub trades: usize,
    /// Trades on markets that match no known community in the window.
    pub unassigned_trades: usize,
    pub readings: usize,
    pub reading_stats: ReadingStats,
}

/// Loads everything the KPIs need for one tick.
pub async fn load_dataset(
    db: &Database,
    window: &Window,
    requirements: DataRequirements,
) -> Result<(Dataset, LoadStats)> {
    let mut dataset = Dataset::default();
    let mut stats = LoadStats::default();

    // Trades are attributed to communities through market ids, so they need communities too.
    if requirements.communities || requirements.trades {
        dataset.communities = load_communities(db).await?;
        stats.communities = dataset.communities.len();
    }

    if requirements.trades {
        let markets = MarketIndex::build(&dataset.communities, window);
        dataset.trades = load_trades(db, window)
            .await?
            .into_iter()
            .map(|trade| trade_record(trade, &markets))
            .collect();
        stats.trades = dataset.trades.len();
        stats.unassigned_trades = dataset
            .trades
            .iter()
            .filter(|trade| trade.community_id.is_none())
            .count();
    }

    if requirements.readings {
        let points = load_measurement_points(db).await?;
        let values = load_timeseries(db, window, &usable_point_ids(&points)).await?;
        let facility_owners = load_facility_owners(db).await?;
        let (readings, reading_stats) = meter_readings(&points, values, &facility_owners);
        dataset.readings = readings;
        stats.readings = dataset.readings.len();
        stats.reading_stats = reading_stats;
    }

    Ok((dataset, stats))
}

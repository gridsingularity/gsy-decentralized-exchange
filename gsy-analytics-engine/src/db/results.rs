//! The `kpi_results` collection: one document per `(kpi_id, community_id, granularity,
//! period_start)`, upserted on every tick so that recomputing a window is idempotent.

use crate::kpi::KpiResult;
use anyhow::Result;
use futures::TryStreamExt;
use mongodb::bson::{self, doc, Document};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use serde::de::DeserializeOwned;

pub const RESULT_KEY_INDEX: &str = "kpi_result_key";
pub const PERIOD_START_INDEX: &str = "period_start";

pub async fn ensure_indexes(collection: &Collection<Document>) -> Result<()> {
    collection
        .create_index(
            IndexModel::builder()
                .keys(doc! {"kpi_id": 1, "community_id": 1, "granularity": 1, "period_start": 1})
                .options(
                    IndexOptions::builder()
                        .name(RESULT_KEY_INDEX.to_string())
                        .unique(true)
                        .build(),
                )
                .build(),
        )
        .await?;
    collection
        .create_index(
            IndexModel::builder()
                .keys(doc! {"period_start": 1})
                .options(
                    IndexOptions::builder()
                        .name(PERIOD_START_INDEX.to_string())
                        .build(),
                )
                .build(),
        )
        .await?;
    Ok(())
}

fn to_document(result: &KpiResult) -> Result<Document> {
    Ok(match result {
        KpiResult::ProcurementCostPerKwh(result) => bson::to_document(result)?,
    })
}

/// Inserts or replaces each result by its key. Returns the number of results written.
pub async fn upsert_results(
    collection: &Collection<Document>,
    results: &[KpiResult],
) -> Result<usize> {
    for result in results {
        let (kpi_id, community_id, granularity, period_start) = result.key();
        let filter = doc! {
            "kpi_id": kpi_id,
            "community_id": community_id,
            "granularity": bson::to_bson(granularity)?,
            "period_start": period_start,
        };
        collection
            .update_one(filter, doc! {"$set": to_document(result)?})
            .upsert(true)
            .await?;
    }
    Ok(results.len())
}

/// Selects stored results of one KPI. Time bounds apply to `period_start` as `[start, end)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResultsFilter {
    pub kpi_id: String,
    pub community_id: Option<String>,
    /// Unix seconds, inclusive.
    pub start_time: Option<i64>,
    /// Unix seconds, exclusive.
    pub end_time: Option<i64>,
}

impl ResultsFilter {
    fn to_document(&self) -> Document {
        let mut filter = doc! {"kpi_id": &self.kpi_id};
        if let Some(community_id) = &self.community_id {
            filter.insert("community_id", community_id);
        }
        let mut period_start = Document::new();
        if let Some(start_time) = self.start_time {
            period_start.insert("$gte", start_time);
        }
        if let Some(end_time) = self.end_time {
            period_start.insert("$lt", end_time);
        }
        if !period_start.is_empty() {
            filter.insert("period_start", period_start);
        }
        filter
    }
}

/// Stored results matching `filter`, ordered by community and period start.
pub async fn find_results<T>(
    collection: &Collection<Document>,
    filter: &ResultsFilter,
) -> Result<Vec<T>>
where
    T: DeserializeOwned + Send + Sync,
{
    Ok(collection
        .clone_with_type::<T>()
        .find(filter.to_document())
        .sort(doc! {"community_id": 1, "period_start": 1})
        .await?
        .try_collect()
        .await?)
}

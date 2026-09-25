//! The `kpi_results` collection: one document per `(kpi_id, community_id, granularity,
//! period_start)`, upserted on every tick so that recomputing a window is idempotent.

use crate::kpi::KpiResult;
use anyhow::Result;
use mongodb::bson::{self, doc, Document};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};

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

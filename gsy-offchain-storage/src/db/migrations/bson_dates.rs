//! Converts times stored before they became BSON dates. Markets and timeseries values stored
//! them as zero-padded epoch-second strings, orders, trades, clearing results and ID mappings
//! as epoch-second integers, and communities, sites and facilities as RFC 3339 strings.
//!
//! It converts only the values that are not dates yet, so running it again changes nothing.

use crate::db::DatabaseWrapper;
use anyhow::{Context, Result};
use mongodb::Collection;
use mongodb::bson::{Document, doc};

pub async fn run(db: &DatabaseWrapper) -> Result<()> {
    convert(
        &db.markets(),
        &[
            "opening_time",
            "closing_time",
            "delivery_start_time",
            "delivery_end_time",
            "created_at",
        ],
    )
    .await?;
    convert(
        &db.orders(),
        &["time_slot", "creation_time", "status_updated_at"],
    )
    .await?;
    convert(&db.trades(), &["creation_time", "status_updated_at"]).await?;
    convert(&db.clearing_results(), &["clearing_time"]).await?;
    convert(&db.timeseries(), &["timestamp"]).await?;
    convert(&db.ids(), &["creation_time"]).await?;
    convert(&db.communities(), &["valid_from", "valid_until"]).await?;
    convert(&db.sites(), &["valid_from", "valid_until"]).await?;
    convert(&db.facilities(), &["valid_from", "valid_until"]).await?;
    Ok(())
}

/// Converts each of `fields` that holds a number or a string to a date. A value that is not
/// a time is left as it is and logged, since its document cannot be read either way.
async fn convert<T: Send + Sync>(collection: &Collection<T>, fields: &[&str]) -> Result<()> {
    for field in fields {
        let legacy = doc! {*field: {"$type": ["string", "number"]}};
        let converted = collection
            .update_many(
                legacy.clone(),
                vec![doc! {"$set": {*field: to_date(field)}}],
            )
            .await
            .with_context(|| format!("Converting {}.{} to dates", collection.name(), field))?
            .modified_count;
        if converted > 0 {
            tracing::info!(
                "Converted {} {}.{} values to dates",
                converted,
                collection.name(),
                field
            );
        }
        let unconverted = collection.count_documents(legacy).await?;
        if unconverted > 0 {
            tracing::error!(
                "{} {}.{} values are not times and were left unconverted",
                unconverted,
                collection.name(),
                field
            );
        }
    }
    Ok(())
}

/// `field` as a date: digits or a number are epoch seconds, any other string is parsed as a
/// date string (truncated to milliseconds), and a value that is neither stays unchanged.
fn to_date(field: &str) -> Document {
    let value = format!("${field}");
    doc! {"$let": {
        "vars": {
            "epoch_secs": {"$convert": {"input": &value, "to": "long", "onError": null}},
        },
        "in": {"$cond": [
            {"$eq": ["$$epoch_secs", null]},
            {"$convert": {"input": &value, "to": "date", "onError": &value}},
            {"$toDate": {"$multiply": ["$$epoch_secs", 1000]}},
        ]},
    }}
}

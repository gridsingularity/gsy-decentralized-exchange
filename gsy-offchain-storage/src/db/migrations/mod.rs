//! Changes to data stored by earlier versions, run in order on startup before the collections
//! are set up. Each runs once per database and is then recorded by name in the `migrations`
//! collection. A crash or a concurrent start can still repeat one before it is recorded, so each
//! must be safe to run again.

mod bson_dates;
mod topology_versioning;

use crate::db::DatabaseWrapper;
use anyhow::{Context, Result};
use mongodb::bson::{DateTime, Document, doc};
use std::future::Future;

const COLLECTION: &str = "migrations";

pub async fn run(db: &DatabaseWrapper) -> Result<()> {
    once(db, "topology_versioning", topology_versioning::run(db)).await?;
    once(db, "bson_dates", bson_dates::run(db)).await?;
    Ok(())
}

/// Runs `migration` unless the database records it as applied, then records it.
async fn once(
    db: &DatabaseWrapper,
    name: &str,
    migration: impl Future<Output = Result<()>>,
) -> Result<()> {
    let applied = db.collection::<Document>(COLLECTION);
    if applied.find_one(doc! {"_id": name}).await?.is_some() {
        return Ok(());
    }
    migration
        .await
        .with_context(|| format!("Migration {name} failed"))?;
    applied
        .update_one(
            doc! {"_id": name},
            doc! {"$setOnInsert": {"applied_at": DateTime::now()}},
        )
        .upsert(true)
        .await?;
    tracing::info!("Applied migration {}", name);
    Ok(())
}

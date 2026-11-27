//! Prepares communities, sites and facilities for versioned entries: drops the unique indexes on
//! ids and names, which versions repeat, and makes a document without `valid_from` the oldest
//! version of its entry. Running it again changes nothing.

use crate::db::DatabaseWrapper;
use anyhow::Result;
use mongodb::Collection;
use mongodb::bson::{self, doc};
use mongodb::error::ErrorKind;

pub async fn run(db: &DatabaseWrapper) -> Result<()> {
    prepare(&db.communities(), &["community_id_1", "community_name_1"]).await?;
    prepare(&db.sites(), &["site_name_1"]).await?;
    prepare(&db.facilities(), &["facility_name_1", "facility_id_1"]).await?;
    Ok(())
}

async fn prepare<T: Send + Sync>(
    collection: &Collection<T>,
    unique_indexes: &[&str],
) -> Result<()> {
    for name in unique_indexes {
        match collection.drop_index(*name).await {
            Ok(()) => {}
            Err(error) if is_missing(&error) => {}
            Err(error) => return Err(error.into()),
        }
    }
    collection
        .update_many(
            doc! {"valid_from": {"$exists": false}},
            doc! {"$set": {"valid_from": bson::DateTime::from_millis(0)}},
        )
        .await?;
    Ok(())
}

/// Whether `error` says that the index or the collection does not exist.
fn is_missing(error: &mongodb::error::Error) -> bool {
    const NAMESPACE_NOT_FOUND: i32 = 26;
    const INDEX_NOT_FOUND: i32 = 27;
    matches!(
        error.kind.as_ref(),
        ErrorKind::Command(command)
            if command.code == NAMESPACE_NOT_FOUND || command.code == INDEX_NOT_FOUND
    )
}

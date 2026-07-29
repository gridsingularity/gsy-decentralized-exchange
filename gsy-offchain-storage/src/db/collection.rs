use crate::db::in_memory::InMemoryCollection;
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{Bson, Document, doc};
use mongodb::options::IndexOptions;
use mongodb::{Collection, Cursor, IndexModel};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

/// A collection handle backed either by a MongoDB collection or by the
/// in-memory store (used by tests to run without MongoDB). All backend
/// dispatch lives here; the services only express their queries.
///
/// MongoDB filters (BSON documents) and in-memory filters (Rust closures)
/// are two different query languages, so the query methods take both
/// representations of the same query and run whichever the backend needs.
pub enum Coll<T: Send + Sync> {
    Mongo(Collection<T>),
    InMemory(InMemoryCollection<T>),
}

/// Backend-neutral outcome of an update operation. (mongodb's `UpdateResult`
/// is `#[non_exhaustive]`, so the in-memory backend cannot construct one.)
#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateSummary {
    pub matched_count: u64,
    pub modified_count: u64,
}

impl From<mongodb::results::UpdateResult> for UpdateSummary {
    fn from(result: mongodb::results::UpdateResult) -> Self {
        UpdateSummary {
            matched_count: result.matched_count,
            modified_count: result.modified_count,
        }
    }
}

impl<T> Coll<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync,
{
    /// Fetch all documents of the collection.
    pub async fn all(&self) -> Result<Vec<T>> {
        match self {
            Coll::Mongo(collection) => match collection.find(doc! {}).await {
                Ok(cursor) => Ok(drain(cursor).await),
                Err(e) => {
                    tracing::error!("Failed to execute query: {:?}", e);
                    Err(anyhow::Error::from(e))
                }
            },
            Coll::InMemory(store) => Ok(store.read().unwrap().clone()),
        }
    }

    /// Fetch the documents matching a filter, given as both a Mongo filter
    /// document and the equivalent in-memory predicate.
    pub async fn query(
        &self,
        mongo_filter: Document,
        mem_filter: impl Fn(&T) -> bool,
    ) -> Result<Vec<T>> {
        match self {
            Coll::Mongo(collection) => match collection.find(mongo_filter).await {
                Ok(cursor) => Ok(drain(cursor).await),
                Err(e) => {
                    tracing::error!("Failed to execute query: {:?}", e);
                    Err(anyhow::Error::from(e))
                }
            },
            Coll::InMemory(store) => Ok(store
                .read()
                .unwrap()
                .iter()
                .filter(|item| mem_filter(item))
                .cloned()
                .collect()),
        }
    }

    /// Fetch the first document matching a filter.
    pub async fn find_one(
        &self,
        mongo_filter: Document,
        mem_filter: impl Fn(&T) -> bool,
    ) -> Result<Option<T>> {
        match self {
            Coll::Mongo(collection) => match collection.find_one(mongo_filter).await {
                Ok(doc) => Ok(doc),
                Err(e) => {
                    tracing::error!("Failed to execute query: {:?}", e);
                    Err(anyhow::Error::from(e))
                }
            },
            Coll::InMemory(store) => Ok(store
                .read()
                .unwrap()
                .iter()
                .find(|item| mem_filter(item))
                .cloned()),
        }
    }

    /// Insert a batch of documents. `mem_id_of` fabricates the per-document id
    /// reported by the in-memory backend (Mongo reports its own inserted ids).
    pub async fn insert_many(
        &self,
        items: Vec<T>,
        mem_id_of: impl Fn(&T) -> Bson,
    ) -> Result<HashMap<usize, Bson>> {
        match self {
            Coll::Mongo(collection) => match collection.insert_many(items).await {
                Ok(db_result) => Ok(db_result.inserted_ids),
                Err(e) => {
                    tracing::error!("Failed to execute query: {:?}", e);
                    Err(anyhow::Error::from(e))
                }
            },
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                let inserted_ids = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| (index, mem_id_of(item)))
                    .collect();
                store.extend(items);
                Ok(inserted_ids)
            }
        }
    }

    /// Insert a single document.
    pub async fn insert_one(&self, item: T) -> Result<()> {
        match self {
            Coll::Mongo(collection) => match collection.insert_one(item).await {
                Ok(_db_result) => Ok(()),
                Err(e) => {
                    tracing::error!("Failed to execute query: {:?}", e);
                    Err(anyhow::Error::from(e))
                }
            },
            Coll::InMemory(store) => {
                store.write().unwrap().push(item);
                Ok(())
            }
        }
    }

    /// Update the first document matching a filter. `mem_apply` mutates a
    /// matched document and reports whether it actually changed.
    pub async fn update_one(
        &self,
        mongo_filter: Document,
        mongo_update: Document,
        mem_filter: impl Fn(&T) -> bool,
        mem_apply: impl Fn(&mut T) -> bool,
    ) -> Result<UpdateSummary> {
        match self {
            Coll::Mongo(collection) => {
                match collection.update_one(mongo_filter, mongo_update).await {
                    Ok(doc) => Ok(doc.into()),
                    Err(e) => {
                        tracing::error!("Failed to execute query: {:?}", e);
                        Err(anyhow::Error::from(e))
                    }
                }
            }
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                match store.iter_mut().find(|item| mem_filter(item)) {
                    Some(item) => Ok(UpdateSummary {
                        matched_count: 1,
                        modified_count: mem_apply(item) as u64,
                    }),
                    None => Ok(UpdateSummary::default()),
                }
            }
        }
    }

    /// Replace the first document matching a filter with `replacement`, inserting it if no
    /// document matches (upsert). In-memory: replace the first matching item in place, else
    /// push `replacement` as a new item.
    pub async fn replace_one_upsert(
        &self,
        mongo_filter: Document,
        replacement: T,
        mem_filter: impl Fn(&T) -> bool,
    ) -> Result<UpdateSummary> {
        match self {
            Coll::Mongo(collection) => {
                match collection
                    .replace_one(mongo_filter, &replacement)
                    .upsert(true)
                    .await
                {
                    Ok(result) => Ok(result.into()),
                    Err(e) => {
                        tracing::error!("Failed to execute query: {:?}", e);
                        Err(anyhow::Error::from(e))
                    }
                }
            }
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                match store.iter_mut().find(|item| mem_filter(item)) {
                    Some(item) => {
                        *item = replacement;
                        Ok(UpdateSummary {
                            matched_count: 1,
                            modified_count: 1,
                        })
                    }
                    None => {
                        store.push(replacement);
                        Ok(UpdateSummary {
                            matched_count: 0,
                            modified_count: 1,
                        })
                    }
                }
            }
        }
    }

    /// Update all documents matching a filter. `mem_apply` mutates a matched
    /// document and reports whether it actually changed.
    pub async fn update_many(
        &self,
        mongo_filter: Document,
        mongo_update: Document,
        mem_filter: impl Fn(&T) -> bool,
        mem_apply: impl Fn(&mut T) -> bool,
    ) -> Result<UpdateSummary> {
        match self {
            Coll::Mongo(collection) => {
                match collection.update_many(mongo_filter, mongo_update).await {
                    Ok(doc) => Ok(doc.into()),
                    Err(e) => {
                        tracing::error!("Failed to execute query: {:?}", e);
                        Err(anyhow::Error::from(e))
                    }
                }
            }
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                let mut summary = UpdateSummary::default();
                for item in store.iter_mut().filter(|item| mem_filter(item)) {
                    summary.matched_count += 1;
                    summary.modified_count += mem_apply(item) as u64;
                }
                Ok(summary)
            }
        }
    }

    /// Create the `_id` index (no-op for the in-memory backend).
    pub async fn ensure_id_index(&self) -> Result<()> {
        if let Coll::Mongo(collection) = self {
            let index: IndexModel = IndexModel::builder()
                .keys(doc! {"_id":1})
                .options(IndexOptions::builder().build())
                .build();
            collection.create_index(index).await?;
        }
        Ok(())
    }

    /// Create a unique index over `keys` (no-op for the in-memory backend, which enforces
    /// uniqueness through [`Coll::replace_one_upsert`] instead).
    pub async fn ensure_unique_index(&self, keys: Document) -> Result<()> {
        if let Coll::Mongo(collection) = self {
            let index: IndexModel = IndexModel::builder()
                .keys(keys)
                .options(IndexOptions::builder().unique(true).build())
                .build();
            collection.create_index(index).await?;
        }
        Ok(())
    }
}

/// Collect a cursor into a vector. NOTE: preserves the historical behavior of
/// returning the partial result accumulated so far on the first cursor error.
async fn drain<T: DeserializeOwned>(mut cursor: Cursor<T>) -> Vec<T> {
    let mut result: Vec<T> = Vec::new();
    while let Some(doc) = cursor.next().await {
        match doc {
            Ok(document) => {
                result.push(document);
            }
            Err(err) => {
                tracing::error!("Error while draining cursor: {}", err.to_string());
                break;
            }
        }
    }
    result
}

/// Add the optional `[start_time, end_time]` window on `time_slot` to a Mongo
/// filter document.
pub(crate) fn apply_time_window(
    filter: &mut Document,
    start_time: Option<u32>,
    end_time: Option<u32>,
) {
    if start_time.is_some() {
        filter.insert("time_slot", doc! {"$gte": start_time.unwrap()});
    }
    if end_time.is_some() {
        if start_time.is_some() {
            filter.insert(
                "time_slot",
                doc! {"$gte": start_time.unwrap(), "$lte": end_time.unwrap()},
            );
        } else {
            filter.insert("time_slot", doc! {"$lte": end_time.unwrap()});
        }
    }
}

/// Build the optional `[start_time, end_time]` range sub-document (the inner
/// `{$gte, $lte}` doc). Callers that place the range under a top-level
/// `time_slot` field use [`apply_time_window`]; callers that need the range
/// under a custom field path (e.g. nested component paths) use this directly.
/// Returns `None` when neither bound is set.
pub(crate) fn time_window_bounds(
    start_time: Option<u32>,
    end_time: Option<u32>,
) -> Option<Document> {
    let mut bounds = Document::new();
    if let Some(start) = start_time {
        bounds.insert("$gte", start);
    }
    if let Some(end) = end_time {
        bounds.insert("$lte", end);
    }
    if bounds.is_empty() {
        None
    } else {
        Some(bounds)
    }
}

/// In-memory counterpart of [`apply_time_window`].
pub(crate) fn in_time_window(
    time_slot: u64,
    start_time: Option<u32>,
    end_time: Option<u32>,
) -> bool {
    start_time.is_none_or(|start| time_slot >= start as u64)
        && end_time.is_none_or(|end| time_slot <= end as u64)
}

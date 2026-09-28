use crate::db::in_memory::InMemoryCollection;
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{Bson, Document, doc};
use mongodb::error::ErrorKind;
use mongodb::options::IndexOptions;
use mongodb::{Collection, Cursor, IndexModel};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;

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

    /// Insert a batch of documents, skipping those whose `_id` is already stored, and return the
    /// `_id`s of the documents actually inserted, keyed by their index in `items`. `id_of` gives
    /// a document's `_id`. A skipped document leaves the stored one, status included, untouched.
    ///
    /// Mongo: an unordered `insert_many`, so a duplicate does not stop the rest of the batch. If
    /// every write error is a duplicate key (`E11000`) the call succeeds; any other error, or a
    /// write concern error, is returned. There is no read before the write, so a batch costs one
    /// round trip. In-memory: documents whose `_id` is stored, or appears earlier in the same
    /// batch, are skipped, so both backends behave the same.
    pub async fn insert_many_skip_duplicates(
        &self,
        items: Vec<T>,
        id_of: impl Fn(&T) -> Bson,
    ) -> Result<HashMap<usize, Bson>> {
        match self {
            Coll::Mongo(collection) => {
                let ids: Vec<Bson> = items.iter().map(&id_of).collect();
                match collection.insert_many(items).ordered(false).await {
                    Ok(db_result) => Ok(db_result.inserted_ids),
                    Err(e) => match duplicate_key_indexes(&e) {
                        Some(duplicates) => {
                            tracing::info!(
                                "Skipped {} already stored document(s) of {}",
                                duplicates.len(),
                                ids.len()
                            );
                            Ok(ids
                                .into_iter()
                                .enumerate()
                                .filter(|(index, _)| !duplicates.contains(index))
                                .collect())
                        }
                        None => {
                            tracing::error!("Failed to execute query: {:?}", e);
                            Err(anyhow::Error::from(e))
                        }
                    },
                }
            }
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                let mut stored_ids: Vec<Bson> = store.iter().map(&id_of).collect();
                let mut inserted_ids = HashMap::new();
                for (index, item) in items.into_iter().enumerate() {
                    let id = id_of(&item);
                    if stored_ids.contains(&id) {
                        continue;
                    }
                    stored_ids.push(id.clone());
                    inserted_ids.insert(index, id);
                    store.push(item);
                }
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

    /// Insert `item` unless a document matching the filter already exists, in which case the
    /// stored document is left untouched. Returns whether `item` was inserted. Mongo: an upsert
    /// whose only operator is `$setOnInsert`, so a match writes nothing. In-memory: push `item`
    /// if no item matches.
    pub async fn insert_if_absent(
        &self,
        mongo_filter: Document,
        item: T,
        mem_filter: impl Fn(&T) -> bool,
    ) -> Result<bool> {
        match self {
            Coll::Mongo(collection) => {
                let update = doc! {"$setOnInsert": mongodb::bson::to_document(&item)?};
                match collection
                    .update_one(mongo_filter, update)
                    .upsert(true)
                    .await
                {
                    Ok(result) => Ok(result.upserted_id.is_some()),
                    Err(e) => {
                        tracing::error!("Failed to execute query: {:?}", e);
                        Err(anyhow::Error::from(e))
                    }
                }
            }
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                if store.iter().any(|stored| mem_filter(stored)) {
                    return Ok(false);
                }
                store.push(item);
                Ok(true)
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

    /// Delete duplicate documents, i.e. documents sharing the values of all `keys`, keeping of
    /// each group the one with the greatest `keep_newest_by`, and return the number of documents
    /// deleted. Run it before [`Coll::ensure_unique_index`] over the same `keys`, which cannot be
    /// built while duplicates exist. No-op returning 0 for the in-memory backend, which starts
    /// empty on every run and so holds no rows written before the unique key existed.
    pub async fn remove_duplicates(&self, keys: &[&str], keep_newest_by: &str) -> Result<u64> {
        let Coll::Mongo(collection) = self else {
            return Ok(0);
        };
        remove_mongo_duplicates(collection, keys, vec![doc! {"$sort": {keep_newest_by: -1}}]).await
    }

    /// Delete duplicate documents, keeping of each group the first in a preference order, and
    /// return the number of documents deleted. Given in both representations: for Mongo, the
    /// `keys` a group shares and the pipeline stages that sort the documents into preference
    /// order (they may `$addFields` a computed rank; nothing is written back); in-memory, the
    /// group key and a comparator that orders a preferred item first (`Ordering::Less`). Ties in
    /// the comparator keep the earlier item. Unlike [`Coll::remove_duplicates`] the in-memory
    /// backend is not a no-op, so the preference can be tested without MongoDB. Run it before
    /// [`Coll::ensure_unique_index`] over the same `keys`.
    pub async fn remove_duplicates_by<K: Eq + Hash>(
        &self,
        keys: &[&str],
        mongo_preference: Vec<Document>,
        mem_key: impl Fn(&T) -> K,
        mem_preference: impl Fn(&T, &T) -> Ordering,
    ) -> Result<u64> {
        match self {
            Coll::Mongo(collection) => {
                remove_mongo_duplicates(collection, keys, mongo_preference).await
            }
            Coll::InMemory(store) => {
                let mut store = store.write().unwrap();
                // Index of the preferred item of each group.
                let mut keep: HashMap<K, usize> = HashMap::new();
                for (index, item) in store.iter().enumerate() {
                    match keep.entry(mem_key(item)) {
                        Entry::Vacant(entry) => {
                            entry.insert(index);
                        }
                        Entry::Occupied(mut entry) => {
                            if mem_preference(item, &store[*entry.get()]) == Ordering::Less {
                                entry.insert(index);
                            }
                        }
                    }
                }
                let before = store.len();
                let mut index = 0;
                store.retain(|item| {
                    let kept = keep.get(&mem_key(item)) == Some(&index);
                    index += 1;
                    kept
                });
                Ok((before - store.len()) as u64)
            }
        }
    }
}

/// MongoDB's duplicate key error code (`E11000`).
const DUPLICATE_KEY_ERROR_CODE: i32 = 11000;

/// If `error` is an `insert_many` error made only of duplicate key write errors, the indexes of
/// the documents that were not inserted; otherwise (another write error, a write concern error,
/// or not an `insert_many` error) `None`.
fn duplicate_key_indexes(error: &mongodb::error::Error) -> Option<Vec<usize>> {
    let ErrorKind::InsertMany(insert_error) = error.kind.as_ref() else {
        return None;
    };
    if insert_error.write_concern_error.is_some() {
        return None;
    }
    let write_errors = insert_error.write_errors.as_ref()?;
    if write_errors.is_empty()
        || write_errors
            .iter()
            .any(|write_error| write_error.code != DUPLICATE_KEY_ERROR_CODE)
    {
        return None;
    }
    Some(
        write_errors
            .iter()
            .map(|write_error| write_error.index)
            .collect(),
    )
}

/// The Mongo side of [`Coll::remove_duplicates`] and [`Coll::remove_duplicates_by`]: group the
/// documents by `keys` after the `preference` stages have sorted them, and delete all but the
/// first `_id` of every group with more than one document.
async fn remove_mongo_duplicates<T: Send + Sync>(
    collection: &Collection<T>,
    keys: &[&str],
    preference: Vec<Document>,
) -> Result<u64> {
    let mut group_key = Document::new();
    for key in keys {
        group_key.insert(*key, format!("${}", key));
    }
    let mut pipeline = preference;
    pipeline.extend([
        doc! {"$group": {
            "_id": group_key,
            "keep": {"$first": "$_id"},
            "ids": {"$push": "$_id"},
            "count": {"$sum": 1},
        }},
        doc! {"$match": {"count": {"$gt": 1}}},
    ]);
    let mut groups = collection.aggregate(pipeline).allow_disk_use(true).await?;
    let mut duplicate_ids: Vec<Bson> = Vec::new();
    while let Some(group) = groups.next().await {
        let group = group?;
        let keep = group.get("keep");
        duplicate_ids.extend(
            group
                .get_array("ids")?
                .iter()
                .filter(|id| Some(*id) != keep)
                .cloned(),
        );
    }
    // Batched so that a large clean-up stays below MongoDB's 16 MB command size limit.
    let mut deleted = 0;
    for batch in duplicate_ids.chunks(REMOVE_DUPLICATES_BATCH_SIZE) {
        deleted += collection
            .delete_many(doc! {"_id": {"$in": batch.to_vec()}})
            .await?
            .deleted_count;
    }
    Ok(deleted)
}

/// Maximum number of `_id`s deleted per `delete_many` by [`Coll::remove_duplicates`].
const REMOVE_DUPLICATES_BATCH_SIZE: usize = 10_000;

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

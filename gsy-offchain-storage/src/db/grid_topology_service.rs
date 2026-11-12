use crate::db::DatabaseWrapper;
use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::error::ErrorKind;
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use primitives::ewds::invalid_event;
use primitives::utils::{epoch_to_rfc3339, now_epoch_secs, EPOCH_RFC3339};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;

pub async fn init_assets(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.assets();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"uuid": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"facility_name": 1})
                .build(),
        )
        .await?;
    Ok(())
}

pub async fn init_pilot_sites(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.pilot_sites();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"pilot_name": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

pub async fn init_communities(db: &DatabaseWrapper) -> Result<()> {
    init_versioned(
        &db.communities(),
        "community_id",
        &["community_id_1", "community_name_1"],
    )
    .await
}

pub async fn init_sites(db: &DatabaseWrapper) -> Result<()> {
    init_versioned(&db.sites(), "site_id", &["site_name_1"]).await
}

pub async fn init_facilities(db: &DatabaseWrapper) -> Result<()> {
    init_versioned(
        &db.facilities(),
        "facility_id",
        &["facility_name_1", "facility_id_1"],
    )
    .await
}

async fn init_versioned<T: Send + Sync>(
    collection: &Collection<T>,
    id_field: &str,
    unique_indexes: &[&str],
) -> Result<()> {
    for name in unique_indexes {
        match collection.drop_index(*name).await {
            Ok(()) => {}
            Err(error) if is_missing(&error) => {}
            Err(error) => return Err(error.into()),
        }
    }
    // Documents from before versioning become the oldest document of their id.
    collection
        .update_many(
            doc! {"valid_from": {"$exists": false}},
            doc! {"$set": {"valid_from": EPOCH_RFC3339}},
        )
        .await?;
    collection
        .create_index(
            IndexModel::builder()
                .keys(doc! {id_field: 1, "valid_from": -1})
                .build(),
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

async fn collect_all<T>(collection: &Collection<T>) -> Result<Vec<T>>
where
    T: Send + Sync + Unpin + serde::de::DeserializeOwned,
{
    Ok(collection.find(doc! {}).await?.try_collect().await?)
}

#[repr(transparent)]
pub struct AssetService(pub Collection<AssetSchema>);

impl AssetService {
    pub async fn insert_assets(&self, assets: Vec<AssetSchema>) -> Result<HashMap<usize, Bson>> {
        Ok(self.0.insert_many(assets).await?.inserted_ids)
    }

    pub async fn get_by_uuid(&self, uuid: &str) -> Result<Option<AssetSchema>> {
        Ok(self.0.find_one(doc! {"uuid": uuid}).await?)
    }

    pub async fn get_by_facility(&self, facility_name: &str) -> Result<Vec<AssetSchema>> {
        Ok(self.0.find(doc! {"facility_name": facility_name}).await?.try_collect().await?)
    }

    pub async fn get_all(&self) -> Result<Vec<AssetSchema>> {
        collect_all(&self.0).await
    }
}

impl From<&DatabaseWrapper> for AssetService {
    fn from(db: &DatabaseWrapper) -> Self {
        AssetService(db.collection("assets"))
    }
}

impl Deref for AssetService {
    type Target = Collection<AssetSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct PilotSiteService(pub Collection<PilotSiteSchema>);

impl PilotSiteService {
    pub async fn insert(&self, pilot: PilotSiteSchema) -> Result<PilotSiteSchema> {
        self.0.insert_one(pilot.clone()).await?;
        Ok(pilot)
    }

    pub async fn get_by_name(&self, pilot_name: &str) -> Result<Option<PilotSiteSchema>> {
        Ok(self.0.find_one(doc! {"pilot_name": pilot_name}).await?)
    }

    pub async fn get_all(&self) -> Result<Vec<PilotSiteSchema>> {
        collect_all(&self.0).await
    }
}

impl From<&DatabaseWrapper> for PilotSiteService {
    fn from(db: &DatabaseWrapper) -> Self {
        PilotSiteService(db.collection("pilot_sites"))
    }
}

impl Deref for PilotSiteService {
    type Target = Collection<PilotSiteSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A community, site or facility. Each one is stored as a series of documents with the same
/// id, one per change, valid from its `valid_from`.
pub trait TopologyEntry:
    Serialize + DeserializeOwned + Clone + PartialEq + Send + Sync + Unpin + 'static
{
    const KIND: &'static str;
    const COLLECTION: &'static str;
    const ID_FIELD: &'static str;

    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn valid_from(&self) -> &str;
    fn set_valid_from(&mut self, valid_from: String);
}

impl TopologyEntry for EnergyCommunitySchema {
    const KIND: &'static str = "community";
    const COLLECTION: &'static str = "communities";
    const ID_FIELD: &'static str = "community_id";

    fn id(&self) -> &str {
        &self.community_id
    }
    fn name(&self) -> &str {
        &self.community_name
    }
    fn valid_from(&self) -> &str {
        &self.valid_from
    }
    fn set_valid_from(&mut self, valid_from: String) {
        self.valid_from = valid_from;
    }
}

impl TopologyEntry for SiteSchema {
    const KIND: &'static str = "site";
    const COLLECTION: &'static str = "sites";
    const ID_FIELD: &'static str = "site_id";

    fn id(&self) -> &str {
        &self.site_id
    }
    fn name(&self) -> &str {
        &self.site_name
    }
    fn valid_from(&self) -> &str {
        &self.valid_from
    }
    fn set_valid_from(&mut self, valid_from: String) {
        self.valid_from = valid_from;
    }
}

impl TopologyEntry for FacilitySchema {
    const KIND: &'static str = "facility";
    const COLLECTION: &'static str = "facilities";
    const ID_FIELD: &'static str = "facility_id";

    fn id(&self) -> &str {
        &self.facility_id
    }
    fn name(&self) -> &str {
        &self.facility_name
    }
    fn valid_from(&self) -> &str {
        &self.valid_from
    }
    fn set_valid_from(&mut self, valid_from: String) {
        self.valid_from = valid_from;
    }
}

/// A name that another latest entry already uses. Over REST this is a 409.
#[derive(Debug)]
pub struct NameConflict(pub String);

impl fmt::Display for NameConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NameConflict {}

pub type EnergyCommunityService = TopologyService<EnergyCommunitySchema>;
pub type SiteService = TopologyService<SiteSchema>;
pub type FacilityService = TopologyService<FacilitySchema>;

#[repr(transparent)]
pub struct TopologyService<T: TopologyEntry>(pub Collection<T>);

impl<T: TopologyEntry> TopologyService<T> {
    /// The latest entries, ordered by id: the latest document of each id, unless its
    /// `valid_until` has passed. Only these are valid; older documents are kept for audit.
    pub async fn get(&self) -> Result<Vec<T>> {
        let now = epoch_to_rfc3339(now_epoch_secs());
        let pipeline = [
            doc! {"$group": {
                "_id": format!("${}", T::ID_FIELD),
                "latest": {"$top": {"sortBy": {"valid_from": -1, "_id": -1}, "output": "$$ROOT"}},
            }},
            doc! {"$replaceWith": "$latest"},
            doc! {"$match": {"$or": [{"valid_until": null}, {"valid_until": {"$gt": now}}]}},
            doc! {"$sort": {T::ID_FIELD: 1}},
        ];
        Ok(self
            .0
            .aggregate(pipeline)
            .with_type::<T>()
            .await?
            .try_collect()
            .await?)
    }

    /// The latest document of `id`, deleted or not.
    async fn latest_document(&self, id: &str) -> Result<Option<T>> {
        Ok(self
            .0
            .find_one(doc! {T::ID_FIELD: id})
            .sort(doc! {"valid_from": -1, "_id": -1})
            .await?)
    }

    /// Stores `entries` as new documents valid from now. An entry equal to the latest document
    /// of its id, apart from `valid_from`, is skipped. Returns the stored document of each
    /// entry, or the latest one if it was skipped.
    ///
    /// Fails before writing anything, with an error marked by [`invalid_event`], if an id or a
    /// name appears twice in `entries`, or if a name is used by a latest entry of another id
    /// ([`NameConflict`]). Entries of ids in `entries` do not count, so two entries can swap
    /// their names. Storing the same entries again changes nothing, so a failed call can be
    /// retried.
    pub async fn store(&self, entries: Vec<T>) -> Result<Vec<T>> {
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for entry in &entries {
            if !ids.insert(entry.id().to_string()) {
                return Err(invalid_event(anyhow!(
                    "{} id {} appears more than once",
                    T::KIND,
                    entry.id()
                )));
            }
            if !names.insert(entry.name().to_string()) {
                return Err(invalid_event(anyhow!(
                    "{} name {} appears more than once",
                    T::KIND,
                    entry.name()
                )));
            }
        }

        if let Some(other) = self
            .get()
            .await?
            .into_iter()
            .find(|latest| !ids.contains(latest.id()) && names.contains(latest.name()))
        {
            return Err(invalid_event(anyhow::Error::new(NameConflict(format!(
                "{} name {} is already used by {} {}",
                T::KIND,
                other.name(),
                T::KIND,
                other.id()
            )))));
        }

        let valid_from = epoch_to_rfc3339(now_epoch_secs());
        let mut stored = Vec::with_capacity(entries.len());
        for mut entry in entries {
            entry.set_valid_from(valid_from.clone());
            match self.latest_document(entry.id()).await? {
                Some(latest) if unchanged(&latest, &entry) => stored.push(latest),
                _ => {
                    self.0.insert_one(&entry).await?;
                    stored.push(entry);
                }
            }
        }
        Ok(stored)
    }
}

/// Whether `entry` equals `latest` apart from `valid_from`.
fn unchanged<T: TopologyEntry>(latest: &T, entry: &T) -> bool {
    let mut latest = latest.clone();
    latest.set_valid_from(entry.valid_from().to_string());
    latest == *entry
}

impl<T: TopologyEntry> From<&DatabaseWrapper> for TopologyService<T> {
    fn from(db: &DatabaseWrapper) -> Self {
        TopologyService(db.collection(T::COLLECTION))
    }
}

impl<T: TopologyEntry> Deref for TopologyService<T> {
    type Target = Collection<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

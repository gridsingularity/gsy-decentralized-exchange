use super::id_service::IdService;
use crate::db::DatabaseWrapper;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use mongodb::bson::{self, doc, Bson, Document};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use primitives::ewds::invalid_event;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

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
    init_versioned(&db.communities(), "community_id").await
}

pub async fn init_sites(db: &DatabaseWrapper) -> Result<()> {
    init_versioned(&db.sites(), "site_id").await
}

pub async fn init_facilities(db: &DatabaseWrapper) -> Result<()> {
    init_versioned(&db.facilities(), "facility_id").await
}

/// Indexes the versions of an entry by id, latest first. The `topology_versioning` migration
/// prepares collections from before versioning.
async fn init_versioned<T: Send + Sync>(collection: &Collection<T>, id_field: &str) -> Result<()> {
    collection
        .create_index(
            IndexModel::builder()
                .keys(doc! {id_field: 1, "valid_from": -1})
                .build(),
        )
        .await?;
    Ok(())
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
        Ok(self
            .0
            .find(doc! {"facility_name": facility_name})
            .await?
            .try_collect()
            .await?)
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
    fn valid_from(&self) -> DateTime<Utc>;
    fn set_valid_from(&mut self, valid_from: DateTime<Utc>);
    fn normalize_ids(self) -> Result<Self>;

    fn actor_ids(&self) -> Vec<&str> {
        vec![self.id()]
    }
}

impl TopologyEntry for EnergyCommunitySchema {
    const KIND: &'static str = "community";
    const COLLECTION: &'static str = "communities";
    const ID_FIELD: &'static str = "community_id";

    fn normalize_ids(self) -> Result<Self> {
        EnergyCommunitySchema::normalize_ids(self)
    }

    fn id(&self) -> &str {
        &self.community_id
    }
    fn name(&self) -> &str {
        &self.community_name
    }
    fn valid_from(&self) -> DateTime<Utc> {
        self.valid_from
    }
    fn set_valid_from(&mut self, valid_from: DateTime<Utc>) {
        self.valid_from = valid_from;
    }
}

impl TopologyEntry for SiteSchema {
    const KIND: &'static str = "site";
    const COLLECTION: &'static str = "sites";
    const ID_FIELD: &'static str = "site_id";

    fn normalize_ids(self) -> Result<Self> {
        SiteSchema::normalize_ids(self)
    }

    fn id(&self) -> &str {
        &self.site_id
    }
    fn name(&self) -> &str {
        &self.site_name
    }
    fn valid_from(&self) -> DateTime<Utc> {
        self.valid_from
    }
    fn set_valid_from(&mut self, valid_from: DateTime<Utc>) {
        self.valid_from = valid_from;
    }
}

impl TopologyEntry for FacilitySchema {
    const KIND: &'static str = "facility";
    const COLLECTION: &'static str = "facilities";
    const ID_FIELD: &'static str = "facility_id";

    fn normalize_ids(self) -> Result<Self> {
        FacilitySchema::normalize_ids(self)
    }

    fn actor_ids(&self) -> Vec<&str> {
        vec![&self.facility_id, &self.owner_id]
    }

    fn id(&self) -> &str {
        &self.facility_id
    }
    fn name(&self) -> &str {
        &self.facility_name
    }
    fn valid_from(&self) -> DateTime<Utc> {
        self.valid_from
    }
    fn set_valid_from(&mut self, valid_from: DateTime<Utc>) {
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

/// The lock of each collection, by namespace, that [`TopologyService::store`] holds from its
/// checks to its last write. REST and the EWDS event subscriber store concurrently; without it,
/// two calls could both pass the checks before either writes. It only orders the calls of this
/// process.
static STORE_LOCKS: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Default::default);

pub type EnergyCommunityService = TopologyService<EnergyCommunitySchema>;
pub type SiteService = TopologyService<SiteSchema>;
pub type FacilityService = TopologyService<FacilitySchema>;

pub struct TopologyService<T: TopologyEntry>(pub Collection<T>, IdService);

impl<T: TopologyEntry> TopologyService<T> {
    /// The latest entries, ordered by id: the latest document of each id, unless its
    /// `valid_until` has passed. Only these are valid; older documents are kept for audit.
    pub async fn get(&self) -> Result<Vec<T>> {
        self.latest(doc! {}).await
    }

    /// The latest entries of `ids`, like [`Self::get`]. Unknown and deleted ids are left out.
    pub async fn get_by_ids(&self, ids: &[&str]) -> Result<Vec<T>> {
        self.latest(doc! {T::ID_FIELD: {"$in": ids}}).await
    }

    async fn latest(&self, filter: Document) -> Result<Vec<T>> {
        let now = bson::DateTime::now();
        let pipeline = [
            doc! {"$match": filter},
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
    /// retried. Calls for one collection run one after another.
    pub async fn store(&self, entries: Vec<T>) -> Result<Vec<T>> {
        let entries = entries
            .into_iter()
            .map(|entry| entry.normalize_ids().map_err(invalid_event))
            .collect::<Result<Vec<_>>>()?;
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

        let lock = STORE_LOCKS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(self.0.namespace().to_string())
            .or_default()
            .clone();
        let _guard = lock.lock().await;

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

        // Mappings precede topology visibility and survive updates/deletion for chain replay.
        // References alone never register another entity or grant eligibility.
        for entry in &entries {
            for actor_id in entry.actor_ids() {
                self.1.get_or_create(actor_id.to_string()).await?;
            }
        }

        // At the precision of a BSON date, so that each returned entry equals its document.
        let valid_from = bson::DateTime::now().to_chrono();
        let mut stored = Vec::with_capacity(entries.len());
        for mut entry in entries {
            entry.set_valid_from(valid_from);
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
    latest.set_valid_from(entry.valid_from());
    latest == *entry
}

impl<T: TopologyEntry> From<&DatabaseWrapper> for TopologyService<T> {
    fn from(db: &DatabaseWrapper) -> Self {
        TopologyService(db.collection(T::COLLECTION), db.ids())
    }
}

impl<T: TopologyEntry> Deref for TopologyService<T> {
    type Target = Collection<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

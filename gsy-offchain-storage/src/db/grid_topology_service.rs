use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::{StreamExt, TryStreamExt};
use mongodb::bson::{self, doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
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
    let controller = db.communities();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"community_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"community_name": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

pub async fn init_sites(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.sites();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"site_name": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

pub async fn init_facilities(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.facilities();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"facility_name": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"facility_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

async fn collect_all<T>(collection: &Collection<T>) -> Result<Vec<T>>
where
    T: Send + Sync + Unpin + serde::de::DeserializeOwned,
{
    let mut cursor = collection.find(doc! {}).await?;
    let mut result = Vec::new();
    while let Some(doc) = cursor.next().await {
        if let Ok(document) = doc {
            result.push(document);
        } else {
            break;
        }
    }
    Ok(result)
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
        let mut cursor = self.0.find(doc! {"facility_name": facility_name}).await?;
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
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

/// A record with a stable id and a unique name: a facility or a community.
pub trait NamedRecord: Serialize + DeserializeOwned + Send + Sync + Unpin {
    const ID_FIELD: &'static str;
    const NAME_FIELD: &'static str;
    fn id(&self) -> &str;
    fn name(&self) -> &str;
}

impl NamedRecord for FacilitySchema {
    const ID_FIELD: &'static str = "facility_id";
    const NAME_FIELD: &'static str = "facility_name";
    fn id(&self) -> &str {
        &self.facility_id
    }
    fn name(&self) -> &str {
        &self.facility_name
    }
}

impl NamedRecord for EnergyCommunitySchema {
    const ID_FIELD: &'static str = "community_id";
    const NAME_FIELD: &'static str = "community_name";
    fn id(&self) -> &str {
        &self.community_id
    }
    fn name(&self) -> &str {
        &self.community_name
    }
}

/// A batch that would give an id a second name, or a name a second id.
#[derive(Debug)]
pub struct DuplicateRecord(pub String);

impl fmt::Display for DuplicateRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DuplicateRecord {}

/// Fails with [`DuplicateRecord`] if a record of `records` uses a stored id under another name
/// or a stored name under another id, or if the batch itself does so.
pub async fn check_for_duplicates<T: NamedRecord>(
    collection: &Collection<T>,
    records: &[T],
) -> Result<()> {
    let mut names_by_id = HashMap::new();
    let mut ids_by_name = HashMap::new();
    for record in records {
        let name = *names_by_id.entry(record.id()).or_insert(record.name());
        let id = *ids_by_name.entry(record.name()).or_insert(record.id());
        if name != record.name() || id != record.id() {
            return Err(DuplicateRecord(format!(
                "The batch uses {} {} or {} {} twice",
                T::ID_FIELD,
                record.id(),
                T::NAME_FIELD,
                record.name()
            ))
            .into());
        }
    }

    let ids: Vec<&str> = names_by_id.keys().copied().collect();
    let names: Vec<&str> = ids_by_name.keys().copied().collect();
    let filter = doc! {"$or": [{T::ID_FIELD: {"$in": ids}}, {T::NAME_FIELD: {"$in": names}}]};
    let stored: Vec<T> = collection.find(filter).await?.try_collect().await?;
    for stored in stored {
        if let Some(name) = names_by_id
            .get(stored.id())
            .filter(|name| **name != stored.name())
        {
            return Err(DuplicateRecord(format!(
                "{} {} is stored with {} {}, not {}",
                T::ID_FIELD,
                stored.id(),
                T::NAME_FIELD,
                stored.name(),
                name
            ))
            .into());
        }
        if let Some(id) = ids_by_name
            .get(stored.name())
            .filter(|id| **id != stored.id())
        {
            return Err(DuplicateRecord(format!(
                "{} {} is already used by {} {}, not {}",
                T::NAME_FIELD,
                stored.name(),
                T::ID_FIELD,
                stored.id(),
                id
            ))
            .into());
        }
    }
    Ok(())
}

/// Inserts `record`, or replaces the stored record with the same id.
pub async fn upsert_by_id<T: NamedRecord>(collection: &Collection<T>, record: T) -> Result<T> {
    let record_doc = bson::to_document(&record)?;
    collection
        .update_one(doc! {T::ID_FIELD: record.id()}, doc! {"$set": record_doc})
        .upsert(true)
        .await?;
    Ok(record)
}

#[repr(transparent)]
pub struct EnergyCommunityService(pub Collection<EnergyCommunitySchema>);

impl EnergyCommunityService {
    pub async fn insert(&self, community: EnergyCommunitySchema) -> Result<EnergyCommunitySchema> {
        self.0.insert_one(community.clone()).await?;
        Ok(community)
    }

    pub async fn get_by_name(&self, community_name: &str) -> Result<Option<EnergyCommunitySchema>> {
        Ok(self
            .0
            .find_one(doc! {"community_name": community_name})
            .await?)
    }

    pub async fn check_for_duplicates(&self, communities: &[EnergyCommunitySchema]) -> Result<()> {
        check_for_duplicates(&self.0, communities).await
    }

    pub async fn upsert(&self, community: EnergyCommunitySchema) -> Result<EnergyCommunitySchema> {
        upsert_by_id(&self.0, community).await
    }

    pub async fn get_all(&self) -> Result<Vec<EnergyCommunitySchema>> {
        collect_all(&self.0).await
    }
}

impl From<&DatabaseWrapper> for EnergyCommunityService {
    fn from(db: &DatabaseWrapper) -> Self {
        EnergyCommunityService(db.collection("communities"))
    }
}

impl Deref for EnergyCommunityService {
    type Target = Collection<EnergyCommunitySchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct SiteService(pub Collection<SiteSchema>);

impl SiteService {
    pub async fn insert(&self, site: SiteSchema) -> Result<SiteSchema> {
        self.0.insert_one(site.clone()).await?;
        Ok(site)
    }

    pub async fn get_by_name(&self, site_name: &str) -> Result<Option<SiteSchema>> {
        Ok(self.0.find_one(doc! {"site_name": site_name}).await?)
    }

    pub async fn upsert(&self, site: SiteSchema) -> Result<SiteSchema> {
        let site_doc = bson::to_document(&site)?;
        self.0
            .update_one(
                doc! {"site_name": site.site_name.clone()},
                doc! {"$set": site_doc},
            )
            .upsert(true)
            .await?;
        Ok(site)
    }

    pub async fn get_all(&self) -> Result<Vec<SiteSchema>> {
        collect_all(&self.0).await
    }
}

impl From<&DatabaseWrapper> for SiteService {
    fn from(db: &DatabaseWrapper) -> Self {
        SiteService(db.collection("sites"))
    }
}

impl Deref for SiteService {
    type Target = Collection<SiteSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct FacilityService(pub Collection<FacilitySchema>);

impl FacilityService {
    pub async fn insert(&self, facility: FacilitySchema) -> Result<FacilitySchema> {
        self.0.insert_one(facility.clone()).await?;
        Ok(facility)
    }

    pub async fn get_by_name(&self, facility_name: &str) -> Result<Option<FacilitySchema>> {
        Ok(self
            .0
            .find_one(doc! {"facility_name": facility_name})
            .await?)
    }

    pub async fn check_for_duplicates(&self, facilities: &[FacilitySchema]) -> Result<()> {
        check_for_duplicates(&self.0, facilities).await
    }

    pub async fn upsert(&self, facility: FacilitySchema) -> Result<FacilitySchema> {
        upsert_by_id(&self.0, facility).await
    }

    pub async fn get_all(&self) -> Result<Vec<FacilitySchema>> {
        collect_all(&self.0).await
    }
}

impl From<&DatabaseWrapper> for FacilityService {
    fn from(db: &DatabaseWrapper) -> Self {
        FacilityService(db.collection("facilities"))
    }
}

impl Deref for FacilityService {
    type Target = Collection<FacilitySchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

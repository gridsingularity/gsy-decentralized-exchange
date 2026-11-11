use crate::db::DatabaseWrapper;
use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use mongodb::bson::{self, doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use primitives::ewds::invalid_event;
use std::collections::HashMap;
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
    Ok(collection.find(doc! {}).await?.try_collect().await?)
}

/// Fails with an error marked by [`invalid_event`] if two of the `(id, name)` pairs share an id
/// but not the name, or a name but not the id. `kind` names the records in the error.
fn check_id_name_pairs<'a>(
    kind: &str,
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<()> {
    let mut names_by_id = HashMap::new();
    let mut ids_by_name = HashMap::new();
    for (id, name) in pairs {
        let known_name = *names_by_id.entry(id).or_insert(name);
        if known_name != name {
            return Err(invalid_event(anyhow!(
                "{kind} id {id} is already named {known_name}, not {name}"
            )));
        }
        let known_id = *ids_by_name.entry(name).or_insert(id);
        if known_id != id {
            return Err(invalid_event(anyhow!(
                "{kind} name {name} is already used by id {known_id}, not {id}"
            )));
        }
    }
    Ok(())
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

    /// Fails with an error marked by [`invalid_event`] if `communities` would give a stored or
    /// batched `community_id` a second name, or a `community_name` a second id.
    pub async fn check_for_duplicates(&self, communities: &[EnergyCommunitySchema]) -> Result<()> {
        let ids: Vec<&str> = communities
            .iter()
            .map(|community| community.community_id.as_str())
            .collect();
        let names: Vec<&str> = communities
            .iter()
            .map(|community| community.community_name.as_str())
            .collect();
        let stored: Vec<EnergyCommunitySchema> = self
            .0
            .find(
                doc! {"$or": [{"community_id": {"$in": ids}}, {"community_name": {"$in": names}}]},
            )
            .await?
            .try_collect()
            .await?;

        check_id_name_pairs(
            "community",
            stored.iter().chain(communities).map(|community| {
                (
                    community.community_id.as_str(),
                    community.community_name.as_str(),
                )
            }),
        )
    }

    pub async fn upsert(&self, community: EnergyCommunitySchema) -> Result<EnergyCommunitySchema> {
        let community_doc = bson::to_document(&community)?;
        self.0
            .update_one(
                doc! {"community_id": community.community_id.clone()},
                doc! {"$set": community_doc},
            )
            .upsert(true)
            .await?;
        Ok(community)
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

    /// Fails with an error marked by [`invalid_event`] if `facilities` would give a stored or
    /// batched `facility_id` a second name, or a `facility_name` a second id.
    pub async fn check_for_duplicates(&self, facilities: &[FacilitySchema]) -> Result<()> {
        let ids: Vec<&str> = facilities
            .iter()
            .map(|facility| facility.facility_id.as_str())
            .collect();
        let names: Vec<&str> = facilities
            .iter()
            .map(|facility| facility.facility_name.as_str())
            .collect();
        let stored: Vec<FacilitySchema> = self
            .0
            .find(doc! {"$or": [{"facility_id": {"$in": ids}}, {"facility_name": {"$in": names}}]})
            .await?
            .try_collect()
            .await?;

        check_id_name_pairs(
            "facility",
            stored.iter().chain(facilities).map(|facility| {
                (
                    facility.facility_id.as_str(),
                    facility.facility_name.as_str(),
                )
            }),
        )
    }

    pub async fn upsert(&self, facility: FacilitySchema) -> Result<FacilitySchema> {
        let facility_doc = bson::to_document(&facility)?;
        self.0
            .update_one(
                doc! {"facility_id": facility.facility_id.clone()},
                doc! {"$set": facility_doc},
            )
            .upsert(true)
            .await?;
        Ok(facility)
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

//! Grid Topology service layer, per D3.2 §5.1. Provides per-collection
//! wrappers for the five document classes that make up the Grid
//! Topology and Market Storage: assets (unified), pilot sites,
//! communities, sites and facilities.

use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use gsy_offchain_primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
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
        .create_index(IndexModel::builder().keys(doc! {"facility_name": 1}).build())
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
        let mut cursor = self
            .0
            .find(doc! {"facility_name": facility_name})
            .await?;
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

#[repr(transparent)]
pub struct EnergyCommunityService(pub Collection<EnergyCommunitySchema>);

impl EnergyCommunityService {
    pub async fn insert(&self, community: EnergyCommunitySchema) -> Result<EnergyCommunitySchema> {
        self.0.insert_one(community.clone()).await?;
        Ok(community)
    }

    pub async fn get_by_name(
        &self,
        community_name: &str,
    ) -> Result<Option<EnergyCommunitySchema>> {
        Ok(self
            .0
            .find_one(doc! {"community_name": community_name})
            .await?)
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

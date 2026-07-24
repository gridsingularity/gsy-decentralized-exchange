use crate::db::DatabaseWrapper;
use anyhow::Context;
use anyhow::{bail, Result};
use futures::StreamExt;
use mongodb::bson::{doc, to_bson};
use mongodb::options::ReturnDocument;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};
use std::ops::Deref;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn init_id_mapping(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.id_mapping();
    controller
        .create_index(IndexModel::builder().keys(doc! {"onchain_id": 1}).build())
        .await?;
    controller
        .create_index(IndexModel::builder().keys(doc! {"offchain_id": 1}).build())
        .await?;
    Ok(())
}

#[repr(transparent)]
pub struct IdService(pub Collection<IdMappingSchema>);

impl IdService {
    #[tracing::instrument(name = "Fetching ids", skip(self))]
    pub async fn filter(
        &self,
        onchain_id: Option<String>,
        offchain_id: Option<String>,
        id_type: Option<String>,
    ) -> Result<Vec<IdMappingSchema>> {
        let mut filter_params = doc! {};
        if onchain_id.is_some() && offchain_id.is_some() {
            bail!("onchain_id and offchain_id are mutually exclusive");
        }
        if let Some(onchain_id) = onchain_id {
            filter_params.insert("onchain_id", onchain_id);
        }
        if let Some(offchain_id) = offchain_id {
            filter_params.insert("offchain_id", offchain_id);
        }
        if let Some(id_type) = id_type {
            filter_params.insert("id_type", id_type);
        }
        if filter_params.is_empty() {
            bail!("at least one filter field must be provided");
        }
        let mut cursor = self.0.find(filter_params).await?;
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

    #[tracing::instrument(
        name = "get or create id",
        skip(self),
        fields(offchain_id = %offchain_id)
    )]
    pub async fn get_or_create(
        &self,
        offchain_id: String,
        id_type: String,
    ) -> Result<IdMappingSchema> {
        let result = self
            .0
            .find_one_and_update(
                doc! {"offchain_id": &offchain_id, "id_type": to_bson(&id_type)?},
                doc! {"$setOnInsert": {
                "onchain_id": bytes16_to_hex(create_encrypted_bytes16_from_string(&offchain_id)),
                "creation_time": SystemTime::now()
                    .duration_since(UNIX_EPOCH)?
                    .as_secs() as i64
                }},
            )
            .upsert(true)
            .return_document(ReturnDocument::After)
            .await?;

        Ok(result.context("upsert returned no document")?)
    }
}

impl From<&DatabaseWrapper> for IdService {
    fn from(db: &DatabaseWrapper) -> Self {
        IdService(db.collection("id_mapping"))
    }
}

impl Deref for IdService {
    type Target = Collection<IdMappingSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

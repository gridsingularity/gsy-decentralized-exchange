use crate::db::DatabaseWrapper;
use anyhow::Context;
use anyhow::{bail, ensure, Result};
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::options::IndexOptions;
use mongodb::options::ReturnDocument;
use mongodb::{Collection, IndexModel};
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::utils::{
    bytes16_to_hex, derive_onchain_id, validate_offchain_actor_id, hex_to_bytes16,
};
use std::ops::Deref;

pub async fn init_ids(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.ids();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"onchain_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"offchain_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
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
    ) -> Result<Vec<IdMappingSchema>> {
        let mut filter_params = doc! {};
        if onchain_id.is_some() && offchain_id.is_some() {
            bail!("onchain_id and offchain_id are mutually exclusive");
        }
        if let Some(onchain_id) = onchain_id {
            let onchain_id =
                hex_to_bytes16(&onchain_id).filter(|id| *id != [0; 16]).context("Invalid on-chain ID")?;
            filter_params.insert("onchain_id", bytes16_to_hex(onchain_id));
        }
        if let Some(offchain_id) = offchain_id {
            filter_params.insert("offchain_id", validate_offchain_actor_id(&offchain_id)?);
        }
        if filter_params.is_empty() {
            bail!("at least one filter field must be provided");
        }
        Ok(self.0.find(filter_params).await?.try_collect().await?)
    }

    #[tracing::instrument(
        name = "get or create id",
        skip(self),
        fields(offchain_id = %offchain_id)
    )]
    pub async fn get_or_create(&self, offchain_id: String) -> Result<IdMappingSchema> {
        let offchain_id = validate_offchain_actor_id(&offchain_id)?;
        let onchain_id = derive_onchain_id(&offchain_id);
        let result = self
            .0
            .find_one_and_update(
                doc! {"offchain_id": &offchain_id},
                doc! {"$setOnInsert": {
                "onchain_id": &onchain_id,
                "creation_time": mongodb::bson::DateTime::now(),
                }},
            )
            .upsert(true)
            .return_document(ReturnDocument::After)
            .await;

        let mapping = match result {
            Ok(mapping) => mapping.context("upsert returned no document"),
            Err(error)
                if matches!(error.kind.as_ref(), ErrorKind::Command(command) if command.code == 11000)
                    || matches!(error.kind.as_ref(), ErrorKind::Write(WriteFailure::WriteError(write)) if write.code == 11000) =>
            {
                // A concurrent upsert can win either unique index race.
                // Only recover the requested mapping, never another ID's collision.
                self.0
                    .find_one(doc! {"offchain_id": &offchain_id})
                    .await?
                    .ok_or_else(|| error.into())
            }
            Err(error) => Err(error.into()),
        }?;
        ensure!(
            mapping.onchain_id == onchain_id,
            "Stored actor mapping does not match ACTOR_ID_HASH_KEY"
        );
        Ok(mapping)
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

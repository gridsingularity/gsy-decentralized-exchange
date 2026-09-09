use crate::config::{Config, OffchainStorageTransport};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use primitives::db_api_schema::grid_topology::EnergyCommunitySchema;
use primitives::ewds::dto::EwdsCommunityDto;
use primitives::ewds::{format_response_body, EwdsClient, EwdsOperation};

#[async_trait]
pub trait CommunityProvider: Send + Sync {
    async fn fetch_communities(&self) -> Result<Vec<EnergyCommunitySchema>>;
}

pub struct OffchainStorageCommunitySource {
    transport: OffchainStorageTransport,
    offchain_storage_url: String,
    http_client: reqwest::Client,
}

impl OffchainStorageCommunitySource {
    pub fn from_config(config: &Config) -> Self {
        Self {
            transport: config.offchain_storage_transport,
            offchain_storage_url: config.offchain_storage_url.clone(),
            http_client: reqwest::Client::new(),
        }
    }

    async fn fetch_via_http(&self) -> Result<Vec<EnergyCommunitySchema>> {
        let url = format!(
            "{}/communities",
            self.offchain_storage_url.trim_end_matches('/')
        );
        let response = self.http_client.get(&url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to fetch communities from '{}': HTTP {}{}",
                url,
                status,
                format_response_body(&body)
            ));
        }

        response
            .json::<Vec<EnergyCommunitySchema>>()
            .await
            .with_context(|| format!("Failed to deserialize communities from '{}'", url))
    }

    async fn fetch_via_ewds(&self) -> Result<Vec<EnergyCommunitySchema>> {
        let client = EwdsClient::from_env(
            "EWDS_MARKET_ORCHESTRATOR_CLIENT_ID",
            "gsymarketorchestrator",
            60_000,
        );
        let communities: Vec<EwdsCommunityDto> = client
            .query(EwdsOperation::CommunitiesQuery, serde_json::json!({}))
            .await?;

        Ok(communities
            .into_iter()
            .map(EnergyCommunitySchema::from)
            .collect())
    }
}

#[async_trait]
impl CommunityProvider for OffchainStorageCommunitySource {
    async fn fetch_communities(&self) -> Result<Vec<EnergyCommunitySchema>> {
        match self.transport {
            OffchainStorageTransport::Http => self.fetch_via_http().await,
            OffchainStorageTransport::Ewds => self.fetch_via_ewds().await,
        }
    }
}

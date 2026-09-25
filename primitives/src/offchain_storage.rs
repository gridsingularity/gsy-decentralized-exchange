use crate::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use crate::db_api_schema::ids::IdMappingSchema;
use crate::db_api_schema::orders::{DbAttributes, DbRequirements};
use crate::ewds::dto::{EwdsClearingResultDto, EwdsCommunityDto};
use crate::ewds::{format_response_body, EwdsClient, EwdsOperation};
use crate::utils::{bytes16_to_hex, parse_uuid_or_hex_bytes16};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use tracing::info;

/// EWDS gateway response timeout used for every capability on this client.
const EWDS_TIMEOUT_MS: u64 = 60_000;

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OffchainStorageTransport {
    #[default]
    Http,
    Ewds,
}

impl OffchainStorageTransport {
    pub fn from_env() -> Self {
        if env::var("OFFCHAIN_STORAGE_TRANSPORT")
            .map(|value| value.eq_ignore_ascii_case("ewds"))
            .unwrap_or(false)
        {
            Self::Ewds
        } else {
            Self::Http
        }
    }
}

#[async_trait]
pub trait CommunityProvider: Send + Sync {
    async fn fetch_communities(&self) -> Result<Vec<EnergyCommunitySchema>>;
}

/// Reads communities, facility/owner mappings and offchain->onchain ID
/// resolutions from off-chain storage, over whichever transport it was
/// configured with. Construct once (per consumer client id) and reuse it
/// across calls rather than rebuilding it per request.
pub struct OffchainStorageClient {
    transport: OffchainStorageTransport,
    offchain_storage_url: String,
    http_client: Client,
    consumer_client_id_env: String,
    consumer_client_id_default: String,
}

impl OffchainStorageClient {
    pub fn new(
        transport: OffchainStorageTransport,
        offchain_storage_url: impl Into<String>,
        consumer_client_id_env: impl Into<String>,
        consumer_client_id_default: impl Into<String>,
    ) -> Self {
        Self {
            transport,
            offchain_storage_url: offchain_storage_url.into(),
            http_client: Client::new(),
            consumer_client_id_env: consumer_client_id_env.into(),
            consumer_client_id_default: consumer_client_id_default.into(),
        }
    }

    pub fn from_env(consumer_client_id_env: &str, consumer_client_id_default: &str) -> Self {
        Self::new(
            OffchainStorageTransport::from_env(),
            env::var("OFFCHAIN_STORAGE_URL")
                .unwrap_or_else(|_| "http://gsy-offchain-storage:8080".to_string()),
            consumer_client_id_env,
            consumer_client_id_default,
        )
    }

    fn ewds_client(&self) -> EwdsClient {
        EwdsClient::from_env(
            self.consumer_client_id_env.as_str(),
            self.consumer_client_id_default.as_str(),
            EWDS_TIMEOUT_MS,
        )
    }

    fn endpoint_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.offchain_storage_url.trim_end_matches('/'),
            path
        )
    }

    async fn fetch_communities_via_http(&self) -> Result<Vec<EnergyCommunitySchema>> {
        let url = self.endpoint_url("communities");
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

    async fn fetch_communities_via_ewds(&self) -> Result<Vec<EnergyCommunitySchema>> {
        let communities: Vec<EwdsCommunityDto> = self
            .ewds_client()
            .query(EwdsOperation::CommunitiesQuery, serde_json::json!({}))
            .await?;

        Ok(communities
            .into_iter()
            .map(EnergyCommunitySchema::from)
            .collect())
    }

    pub async fn fetch_facility_owner_mapping(&self) -> Result<HashMap<String, String>> {
        let facilities: Vec<FacilitySchema> = match self.transport {
            OffchainStorageTransport::Ewds => {
                info!("Fetching facilities via EWDS transport");
                self.ewds_client()
                    .query(EwdsOperation::FacilitiesQuery, serde_json::json!({}))
                    .await?
            }
            OffchainStorageTransport::Http => {
                let url = self.endpoint_url("facilities");
                info!("Fetching facilities for {}", url);

                let response = self.http_client.get(&url).send().await?;
                if !response.status().is_success() {
                    return Err(anyhow!(
                        "Failed to fetch facilities. HTTP {}",
                        response.status()
                    ));
                }
                response.json().await?
            }
        };

        let mapping: HashMap<String, String> = facilities
            .into_iter()
            .map(|f| (f.facility_id, f.owner_id))
            .collect();
        info!("returning mapping {:?}", mapping.clone());
        Ok(mapping)
    }

    pub async fn fetch_onchain_id(&self, offchain_id: &str) -> Result<String> {
        let mapping: IdMappingSchema = match self.transport {
            OffchainStorageTransport::Ewds => {
                info!("Fetching onchain_id via EWDS transport");
                let mut response: Vec<IdMappingSchema> = self
                    .ewds_client()
                    .query(
                        EwdsOperation::IdsQuery,
                        serde_json::json!({"offchain_id": offchain_id}),
                    )
                    .await?;
                response.pop().ok_or_else(|| {
                    anyhow!("No id mapping returned for offchain_id {}", offchain_id)
                })?
            }
            OffchainStorageTransport::Http => {
                let url = self.endpoint_url("ids");
                let response = self
                    .http_client
                    .post(&url)
                    .query(&[("offchain_id", offchain_id)])
                    .send()
                    .await?;
                if !response.status().is_success() {
                    return Err(anyhow!(
                        "Failed to fetch onchain_id. HTTP {}",
                        response.status()
                    ));
                }
                response.json::<IdMappingSchema>().await?
            }
        };

        if mapping.offchain_id != offchain_id {
            return Err(anyhow!(
                "ID service returned a mapping for a different facility"
            ));
        }
        let bytes = parse_uuid_or_hex_bytes16(&mapping.onchain_id)
            .filter(|bytes| *bytes != [0; 16])
            .ok_or_else(|| anyhow!("Invalid on-chain ID returned for facility {}", offchain_id))?;
        Ok(bytes16_to_hex(bytes))
    }

    pub async fn fetch_clearing_results(
        &self,
        market_id: &str,
    ) -> Result<Vec<EwdsClearingResultDto>> {
        match self.transport {
            OffchainStorageTransport::Ewds => {
                info!("Fetching clearing_results via EWDS transport");
                self.ewds_client()
                    .query(
                        EwdsOperation::ClearingResultsQuery,
                        serde_json::json!({"market_id": market_id}),
                    )
                    .await
            }
            OffchainStorageTransport::Http => {
                let url = self.endpoint_url("clearing-results");
                let response = self
                    .http_client
                    .get(&url)
                    .query(&[("market_id", market_id)])
                    .send()
                    .await?;
                if !response.status().is_success() {
                    return Err(anyhow!(
                        "Failed to fetch clearing_results. HTTP {}",
                        response.status()
                    ));
                }
                Ok(response.json().await?)
            }
        }
    }
}

#[async_trait]
impl CommunityProvider for OffchainStorageClient {
    async fn fetch_communities(&self) -> Result<Vec<EnergyCommunitySchema>> {
        match self.transport {
            OffchainStorageTransport::Http => self.fetch_communities_via_http().await,
            OffchainStorageTransport::Ewds => self.fetch_communities_via_ewds().await,
        }
    }
}

/// Resolve off-chain facility IDs before converting metadata or matching actor IDs.
/// Input identifiers are always off-chain IDs, including UUID and hex-shaped strings.
pub async fn resolve_order_partner_ids(
    requirements: &mut Option<DbRequirements>,
    attributes: &mut Option<DbAttributes>,
    id_mapping_source: &OffchainStorageClient,
) -> Result<()> {
    for partner in [
        requirements
            .as_mut()
            .and_then(|value| value.trading_partner_id.as_mut()),
        attributes
            .as_mut()
            .and_then(|value| value.trading_partner_id.as_mut()),
    ]
    .into_iter()
    .flatten()
    {
        *partner = id_mapping_source.fetch_onchain_id(partner).await?;
    }
    Ok(())
}

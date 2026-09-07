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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn source(url: String) -> OffchainStorageCommunitySource {
        OffchainStorageCommunitySource {
            transport: OffchainStorageTransport::Http,
            offchain_storage_url: url,
            http_client: reqwest::Client::new(),
        }
    }

    fn serve_once(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let body = body.to_string();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test request should connect");
            let mut request = [0_u8; 1_024];
            stream
                .read(&mut request)
                .expect("request should be readable");
            write!(
                stream,
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            )
            .expect("response should be writable");
        });

        format!("http://{}", address)
    }

    #[tokio::test]
    async fn fetches_all_communities_over_http() {
        let body = r#"[
            {
                "community_id": "11111111-1111-4111-8111-111111111111",
                "community_name": "Community One",
                "sites": ["site-one"]
            },
            {
                "community_id": "22222222-2222-4222-8222-222222222222",
                "community_name": "Community Two",
                "sites": ["site-two"]
            }
        ]"#;
        let source = source(serve_once("200 OK", body));

        let communities = source.fetch_communities().await.unwrap();

        assert_eq!(communities.len(), 2);
        assert_eq!(
            communities[0].community_id,
            "11111111-1111-4111-8111-111111111111"
        );
        assert_eq!(
            communities[1].community_id,
            "22222222-2222-4222-8222-222222222222"
        );
    }

    #[tokio::test]
    async fn propagates_http_failures() {
        let source = source(serve_once(
            "503 Service Unavailable",
            "temporarily unavailable",
        ));

        let error = source.fetch_communities().await.unwrap_err().to_string();

        assert!(error.contains("HTTP 503 Service Unavailable"));
        assert!(error.contains("temporarily unavailable"));
    }

    #[tokio::test]
    async fn propagates_deserialization_failures() {
        let source = source(serve_once("200 OK", r#"{"community_id":"invalid"}"#));

        let error = source.fetch_communities().await.unwrap_err().to_string();

        assert!(error.contains("Failed to deserialize communities"));
    }
}

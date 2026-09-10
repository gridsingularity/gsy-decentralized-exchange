use crate::config::Config;
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::utils::read_env_or;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use tracing::info;

/// Build a reqwest client that sends the `x-api-key` header the off-chain storage now
/// requires. The key comes from the `API_KEY` env var (default `fedecom_user`) and must
/// match the storage service's configured key.
fn authorized_storage_client() -> Client {
	let api_key = read_env_or("API_KEY", "fedecom_user".to_string());
	let mut headers = HeaderMap::new();
	if let Ok(value) = HeaderValue::from_str(&api_key) {
		headers.insert("x-api-key", value);
	}
	Client::builder()
		.default_headers(headers)
		.build()
		.expect("Failed to build off-chain storage HTTP client")
}

/// Read-only connector to the offchain storage. Responsible for fetching market data that the
/// market orchestrator needs from the offchain storage.
#[derive(Clone)]
pub struct OffchainStorageConnector {
	client: Client,
	markets_url: String,
}

impl OffchainStorageConnector {
	pub fn new(config: &Config) -> Self {
		let base = config.offchain_storage_url.trim_end_matches('/');
		let markets_url = format!("{}/markets", base);
		info!("Orchestrator reading markets from: {}", markets_url);
		Self { client: authorized_storage_client(), markets_url }
	}

	/// Fetch every market for all communities whose delivery time_slot falls within start_time
	/// and end_time.
	pub async fn get_markets_in_window(
		&self,
		start_time: u64,
		end_time: u64,
	) -> Result<Vec<MarketTopologySchema>> {
		let url = format!(
			"{}?start_time={}&end_time={}",
			self.markets_url, start_time, end_time
		);
		let response = self.client.get(&url).send().await?.error_for_status()?;
		Ok(response.json::<Vec<MarketTopologySchema>>().await?)
	}
}

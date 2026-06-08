use crate::config::Config;
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use reqwest::Client;
use tracing::info;

/// Read-only connector to the offchain storage. The community client is the
/// source of truth for which per-community markets exist; the orchestrator only
/// discovers them and toggles their on-chain status.
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
		Self { client: Client::new(), markets_url }
	}

	/// Fetch every market whose delivery time_slot falls within
	/// `[start_time, end_time]` (inclusive), across all communities.
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

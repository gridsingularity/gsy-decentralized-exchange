use crate::config::Config;
use anyhow::Result;
use subxt::{utils::H256, OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::Keypair;
use tracing::{error, info};

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node {}

#[derive(Clone)]
pub struct GsyNodeClient {
	api: OnlineClient<SubstrateConfig>,
	signer: Keypair,
}

impl GsyNodeClient {
	pub async fn new(config: &Config) -> Result<Self> {
		let api = OnlineClient::<SubstrateConfig>::from_insecure_url(&config.gsy_node_url).await?;
		let signer = Keypair::from_suri(&config.orchestrator_signer_suri, None)?;
		info!("Orchestrator connected to node: {}", config.gsy_node_url);
		info!("Orchestrator signer account: {}", signer.public_key());
		Ok(Self { api, signer })
	}

	pub async fn get_market_status(&self, market_id: H256) -> Result<bool> {
		let storage_address = gsy_node::storage().orderbook_registry().market_status(market_id);
		let status = self
			.api
			.storage()
			.at_latest()
			.await?
			.fetch(&storage_address)
			.await?
			.unwrap_or(false);
		Ok(status)
	}

	pub async fn update_market_status(&self, market_id: H256, is_open: bool) -> Result<()> {
		let tx = gsy_node::tx().orderbook_registry().update_market_status(market_id, is_open);

		let result = self
			.api
			.tx()
			.sign_and_submit_then_watch_default(&tx, &self.signer)
			.await?
			.wait_for_finalized_success()
			.await?;

		let event =
			result.find_first::<gsy_node::orderbook_registry::events::MarketStatusUpdated>()?;

		if let Some(event) = event {
			info!("Successfully submitted and finalized MarketStatusUpdated: {:?}", event);
		} else {
			error!("Failed to find MarketStatusUpdated event after finalization.");
		}
		Ok(())
	}
}

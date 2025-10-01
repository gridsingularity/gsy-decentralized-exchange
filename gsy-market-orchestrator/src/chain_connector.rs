use crate::config::Config;
use anyhow::Result;
use subxt::{config::substrate::AccountId32, utils::H256, OnlineClient, SubstrateConfig};
use subxt_signer::{sr25519::Keypair, SecretUri};
use tracing::{error, info};

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node {}

#[derive(Clone)]
pub struct GsyMarketOrchestratorNodeClient {
	api: OnlineClient<SubstrateConfig>,
	signer: Keypair,
}

impl GsyMarketOrchestratorNodeClient {
	pub async fn new(config: &Config) -> Result<Self> {
		let api =
			OnlineClient::<SubstrateConfig>::from_insecure_url(config.gsy_node_url.clone()).await?;
		let uri: SecretUri = config.orchestrator_signer_suri.parse()?;
		let signer = Keypair::from_uri(&uri)?;
		info!("Orchestrator connected to node: {}", config.gsy_node_url);
		let account_id = AccountId32::from(signer.public_key());
		info!("Orchestrator signer account: {}", account_id);
		Ok(Self { api, signer })
	}

	pub async fn is_operator_registered(&self) -> Result<bool> {
		let operator_account = AccountId32::from(self.signer.public_key());
		let storage_address = gsy_node::storage()
			.gsy_collateral()
			.registered_matching_engine(operator_account);
		let is_registered =
			self.api.storage().at_latest().await?.fetch(&storage_address).await?.is_some();
		Ok(is_registered)
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

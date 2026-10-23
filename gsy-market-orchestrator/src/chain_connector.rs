use crate::config::Config;
use anyhow::{Result, anyhow, bail};
use subxt::{
    OnlineClient, SubstrateConfig,
    backend::{legacy::LegacyRpcMethods, rpc::RpcClient},
    config::{DefaultExtrinsicParamsBuilder, substrate::AccountId32},
    tx::TxStatus,
    utils::H256,
};
use subxt_signer::{SecretUri, sr25519::Keypair};
use tracing::{error, info, warn};

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

#[derive(Clone)]
pub struct GsyMarketOrchestratorNodeClient {
    api: OnlineClient<SubstrateConfig>,
    /// Raw RPC on the same connection, for the two calls subxt's high-level API answers at
    /// the latest *finalized* block: the best block hash and the pool-aware account nonce.
    rpc: LegacyRpcMethods<SubstrateConfig>,
    signer: Keypair,
}

impl GsyMarketOrchestratorNodeClient {
    pub async fn new(config: &Config) -> Result<Self> {
        let rpc_client = RpcClient::from_insecure_url(config.gsy_node_url.clone()).await?;
        let api = OnlineClient::<SubstrateConfig>::from_rpc_client(rpc_client.clone()).await?;
        let rpc = LegacyRpcMethods::<SubstrateConfig>::new(rpc_client);
        let uri: SecretUri = config.orchestrator_signer_suri.parse()?;
        let signer = Keypair::from_uri(&uri)?;
        info!("Orchestrator connected to node: {}", config.gsy_node_url);
        let account_id = AccountId32::from(signer.public_key());
        info!("Orchestrator signer account: {}", account_id);
        Ok(Self { api, rpc, signer })
    }

    pub async fn is_operator_registered(&self) -> Result<bool> {
        let operator_account = AccountId32::from(self.signer.public_key());
        let storage_address = gsy_node::storage()
            .gsy_collateral()
            .registered_exchange_operator(operator_account);
        let is_registered = self
            .api
            .storage()
            .at_latest()
            .await?
            .fetch(&storage_address)
            .await?
            .is_some();
        Ok(is_registered)
    }

    /// Market status at the best block, not the finalized one: status updates are only
    /// awaited until inclusion (see [`Self::update_market_status`]), so reading at the
    /// finalized block would miss an update made a block or two ago and send it again.
    pub async fn get_market_status(&self, market_id: H256) -> Result<bool> {
        let storage_address = gsy_node::storage()
            .orderbook_registry()
            .market_status(market_id);
        let best = self
            .rpc
            .chain_get_block_hash(None)
            .await?
            .ok_or_else(|| anyhow!("node returned no best block hash"))?;
        let status = self
            .api
            .storage()
            .at(best)
            .fetch(&storage_address)
            .await?
            .unwrap_or(false);
        Ok(status)
    }

    /// Submit a market status update and wait until it is in a block — not finalized.
    ///
    /// Waiting for finality costs several blocks per update, which capped the orchestrator
    /// below the rate markets have to be opened and closed. Inclusion is enough here: every
    /// tick re-reads each market's status and re-applies whatever is missing, so an update
    /// lost to a re-org is simply sent again.
    ///
    /// The nonce comes from `system_accountNextIndex`, which counts transactions already in
    /// the pool. subxt's default reads the nonce at the latest finalized block, which still
    /// lags the previous, merely-included update and would reuse its nonce.
    pub async fn update_market_status(&self, market_id: H256, is_open: bool) -> Result<()> {
        let tx = gsy_node::tx()
            .orderbook_registry()
            .update_market_status(market_id, is_open);
        let account_id = AccountId32::from(self.signer.public_key());
        let nonce = self.rpc.system_account_next_index(&account_id).await?;
        let params = DefaultExtrinsicParamsBuilder::<SubstrateConfig>::new()
            .nonce(nonce)
            .build();

        let mut progress = self
            .api
            .tx()
            .sign_and_submit_then_watch(&tx, &self.signer, params)
            .await?;

        while let Some(status) = progress.next().await {
            match status? {
                TxStatus::InBestBlock(in_block) | TxStatus::InFinalizedBlock(in_block) => {
                    let events = in_block.wait_for_success().await?;
                    match events
                        .find_first::<gsy_node::orderbook_registry::events::MarketStatusUpdated>()?
                    {
                        Some(event) => info!(
                            "MarketStatusUpdated in block {:?} (nonce {}): {:?}",
                            in_block.block_hash(),
                            nonce,
                            event
                        ),
                        None => error!(
                            "No MarketStatusUpdated event in block {:?} for market {:?}.",
                            in_block.block_hash(),
                            market_id
                        ),
                    }
                    return Ok(());
                }
                TxStatus::NoLongerInBestBlock => {
                    warn!(
                        "Status update for market {:?} left the best chain; waiting.",
                        market_id
                    )
                }
                TxStatus::Validated | TxStatus::Broadcasted => {}
                TxStatus::Invalid { message } => {
                    bail!(
                        "status update for market {:?} invalid (nonce {}): {}",
                        market_id,
                        nonce,
                        message
                    )
                }
                TxStatus::Dropped { message } => {
                    bail!(
                        "status update for market {:?} dropped (nonce {}): {}",
                        market_id,
                        nonce,
                        message
                    )
                }
                TxStatus::Error { message } => {
                    bail!(
                        "status update for market {:?} failed (nonce {}): {}",
                        market_id,
                        nonce,
                        message
                    )
                }
            }
        }
        bail!(
            "transaction status stream for market {:?} ended before inclusion",
            market_id
        )
    }
}

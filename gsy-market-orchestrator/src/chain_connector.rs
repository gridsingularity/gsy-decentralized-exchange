use crate::config::Config;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ethers::prelude::*;
use ethers::utils::keccak256;
use std::sync::Arc;
use tracing::{info, warn};

abigen!(
    MarketControllerContract,
    r#"[
        function hasRole(bytes32 role, address account) external view returns (bool)
        struct MarketKey { bytes16 communityId; uint8 marketType; uint64 deliveryStartTime; }
        function getMarketIds(MarketKey[] keys) external view returns (bytes16[])
        struct NewMarket { bytes16 marketId; bytes16 communityId; uint64 openingTime; uint64 closingTime; uint64 deliveryStartTime; uint64 deliveryEndTime; uint8 marketType; uint8 matchingAlgorithm; }
        function createMarkets(NewMarket[] newMarkets) external
    ]"#
);

type WsSignerMiddleware = SignerMiddleware<Provider<Ws>, LocalWallet>;

#[async_trait]
pub trait MarketChainClient: Send + Sync {
    async fn is_operator_registered(&self) -> Result<bool>;
    /// One `eth_call` for all keys; `result[i]` is the ID for `keys[i]`, or zero if missing.
    async fn get_market_ids(&self, keys: Vec<MarketKey>) -> Result<Vec<[u8; 16]>>;
    /// Creates the markets in one transaction; existing ones are skipped on-chain.
    async fn create_markets(&self, new_markets: Vec<NewMarket>) -> Result<()>;
}

#[derive(Clone)]
pub struct GsyMarketOrchestratorNodeClient {
    market_controller: MarketControllerContract<WsSignerMiddleware>,
    signer_address: Address,
}

impl GsyMarketOrchestratorNodeClient {
    pub async fn new(config: &Config) -> Result<Self> {
        if config.market_controller_address.is_zero() {
            warn!("MARKET_CONTROLLER_ADDRESS is zero; contract calls will fail until configured.");
        }

        let provider = Provider::<Ws>::connect(config.evm_node_url.as_str()).await?;
        let chain_id = provider.get_chainid().await?.as_u64();

        let wallet = config
            .orchestrator_signer_private_key
            .parse::<LocalWallet>()?
            .with_chain_id(chain_id);
        let signer_address = wallet.address();

        info!(
            "Orchestrator connected to EVM node: {}",
            config.evm_node_url
        );
        info!("Orchestrator chain id: {}", chain_id);
        info!("Orchestrator signer account: {:?}", signer_address);

        let client = Arc::new(SignerMiddleware::new(provider, wallet));
        let market_controller =
            MarketControllerContract::new(config.market_controller_address, client.clone());

        Ok(Self {
            market_controller,
            signer_address,
        })
    }
}

#[async_trait]
impl MarketChainClient for GsyMarketOrchestratorNodeClient {
    async fn is_operator_registered(&self) -> Result<bool> {
        let orchestrator_role = keccak256("ORCHESTRATOR_ROLE");
        let is_registered = self
            .market_controller
            .has_role(orchestrator_role, self.signer_address)
            .call()
            .await?;
        Ok(is_registered)
    }

    async fn get_market_ids(&self, keys: Vec<MarketKey>) -> Result<Vec<[u8; 16]>> {
        let key_count = keys.len();
        let market_ids = self.market_controller.get_market_ids(keys).call().await?;
        if market_ids.len() != key_count {
            return Err(anyhow!(
                "getMarketIds returned {} results for {} keys",
                market_ids.len(),
                key_count
            ));
        }
        Ok(market_ids)
    }

    async fn create_markets(&self, new_markets: Vec<NewMarket>) -> Result<()> {
        if new_markets.is_empty() {
            return Ok(());
        }

        let market_count = new_markets.len();
        let create_markets_call = self.market_controller.create_markets(new_markets);
        let pending_tx = create_markets_call.send().await?;

        let tx_hash = pending_tx.tx_hash();
        let receipt = pending_tx.await?;

        match receipt {
            Some(receipt) => {
                let status = receipt
                    .status
                    .map(|value| value.as_u64())
                    .unwrap_or_default();
                if status != 1 {
                    return Err(anyhow!(
                        "Market creation transaction {:?} reverted with status {:?}",
                        tx_hash,
                        receipt.status
                    ));
                }
                info!(
                    "Successfully finalized market creation tx {:?} (markets={})",
                    tx_hash, market_count
                );
                Ok(())
            }
            None => Err(anyhow!(
                "Market creation transaction {:?} dropped without receipt",
                tx_hash
            )),
        }
    }
}

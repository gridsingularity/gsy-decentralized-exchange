use anyhow::Result;
use gsy_market_orchestrator::{chain_connector, config, orchestrator};
use primitives::offchain_storage::OffchainStorageClient;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting GSY Market Orchestrator...");
    let config = config::get_config()?;
    let client = chain_connector::GsyMarketOrchestratorNodeClient::new(&config).await?;
    let community_source = OffchainStorageClient::new(
        config.offchain_storage_transport,
        config.offchain_storage_url.clone(),
        "EWDS_MARKET_ORCHESTRATOR_CLIENT_ID",
        "gsymarketorchestrator",
    );

    orchestrator::run(config, client, community_source).await
}

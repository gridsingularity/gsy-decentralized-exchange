use anyhow::Result;
use gsy_market_orchestrator::{chain_connector, config, orchestrator};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting GSY Market Orchestrator...");
    let config = config::get_config()?;
    let client = chain_connector::GsyMarketOrchestratorNodeClient::new(&config).await?;

    orchestrator::run(config, client).await
}

mod chain_connector;
mod config;
mod orchestrator;

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.init();

	info!("Starting GSY Market Orchestrator...");
	let config = config::get_config()?;
	let client = chain_connector::GsyNodeClient::new(&config).await?;

	orchestrator::run(config, client).await
}

use crate::chain_connector::GsyMarketOrchestratorNodeClient;
use crate::config::Config;
use crate::storage_connector::OffchainStorageConnector;
use gsy_offchain_primitives::{
	constants::GlobalConstants, utils::string_to_h256, utils::timestamp_to_datetime_string,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{error, info, warn};

pub async fn run(
	config: Config,
	client: GsyMarketOrchestratorNodeClient,
	storage: OffchainStorageConnector,
) -> anyhow::Result<()> {
	info!("Configuration: {:?}", config);

	info!("Waiting for orchestrator account to be registered as an operator...");
	loop {
		match client.is_operator_registered().await {
			Ok(true) => {
				info!("Orchestrator account is registered. Starting main loop.");
				break;
			},
			Ok(false) => {
				warn!("Orchestrator account not yet registered. Retrying in 10 seconds...");
			},
			Err(e) => {
				error!("Error checking registration status: {:?}. Retrying in 10 seconds...", e);
			},
		}
		sleep(Duration::from_secs(10)).await;
	}

	let interval = Duration::from_secs(config.tick_interval_seconds);

	loop {
		info!("-- Orchestrator Tick --");
		if let Err(e) = orchestrate_markets(&config, &client, &storage).await {
			error!("An error occurred during orchestration tick: {:?}", e);
		}
		sleep(interval).await;
	}
}

/// Discover every per-community spot market from the offchain storage and toggle its on-chain
/// status if it is in the market open time window. The community client is creating the markets,
/// the market orchestrator only manages existing markets.
async fn orchestrate_markets(
	config: &Config,
	client: &GsyMarketOrchestratorNodeClient,
	storage: &OffchainStorageConnector,
) -> anyhow::Result<()> {
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
	let window_start = (now / GlobalConstants.TIME_SLOT_SEC) * GlobalConstants.TIME_SLOT_SEC;
	let look_ahead_horizon = now + (config.look_ahead_hours * 3600);

	info!(
		"Orchestrator Check at {}. Fetching community markets for delivery in [{}, {}].",
		now,
		timestamp_to_datetime_string(window_start),
		timestamp_to_datetime_string(look_ahead_horizon)
	);

	let markets = storage.get_markets_in_window(window_start, look_ahead_horizon).await?;
	info!("Found {} community market(s) in the look-ahead window.", markets.len());

	for market in markets {
		let delivery_secs = market.time_slot as u64;
		let market_id = string_to_h256(market.market_id.clone());
		let (open_time, close_time) = GlobalConstants.spot_market_window(delivery_secs);

		let on_chain_status = client.get_market_status(market_id).await?;
		let should_be_open = now >= open_time && now < close_time;

		if should_be_open && !on_chain_status {
			info!(
				"OPENING spot market for community '{}' ({}) delivery at {}. Opening time {}.",
				market.community_name,
				market.market_id,
				timestamp_to_datetime_string(delivery_secs),
				timestamp_to_datetime_string(open_time)
			);
			client.update_market_status(market_id, true).await?;
		} else if !should_be_open && on_chain_status {
			info!(
				"CLOSING spot market for community '{}' ({}) delivery at {}. Closing time {}.",
				market.community_name,
				market.market_id,
				timestamp_to_datetime_string(delivery_secs),
				timestamp_to_datetime_string(close_time)
			);
			client.update_market_status(market_id, false).await?;
		}
	}
	Ok(())
}

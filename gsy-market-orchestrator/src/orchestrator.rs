use crate::chain_connector::{self, gsy_node, GsyNodeClient};
use crate::config::{Config, MARKET_RULES};
use sp_runtime::traits::{BlakeTwo256, Hash as HashT};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subxt::utils::H256;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// The main loop for the market orchestrator.
pub async fn run(config: Config, client: GsyNodeClient) -> anyhow::Result<()> {
	info!("Market Orchestrator started.");
	info!("Configuration: {:?}", config);

	let interval = Duration::from_secs(config.tick_interval_seconds);

	loop {
		info!("-- Orchestrator Tick --");
		if let Err(e) = orchestrate_markets(&config, &client).await {
			error!("An error occurred during orchestration tick: {:?}", e);
		}
		sleep(interval).await;
	}
}

/// The core logic for a single orchestration cycle.
async fn orchestrate_markets(config: &Config, client: &GsyNodeClient) -> anyhow::Result<()> {
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
	let look_ahead_horizon = now + (config.look_ahead_hours * 3600);

	// Iterate through 15-minute time slots from now up to the horizon
	let fifteen_mins_as_secs = 15 * 60;
	let mut current_slot = (now / fifteen_mins_as_secs) * fifteen_mins_as_secs;

	while current_slot <= look_ahead_horizon {
		let delivery_time = current_slot;

		for rule in MARKET_RULES {
			let market_id = generate_market_id(rule.market_type, delivery_time);

			let open_time = (delivery_time as i64 + rule.open_offset_mins * 60) as u64;
			let close_time = (delivery_time as i64 + rule.close_offset_mins * 60) as u64;

			let on_chain_status = client.get_market_status(market_id).await?;

			// Determine desired state based on current time
			let should_be_open = now >= open_time && now < close_time;

			if should_be_open && !on_chain_status {
				info!(
					"Market {} for delivery at {} should be OPEN. Submitting transaction...",
					rule.market_type, delivery_time
				);
				client.update_market_status(market_id, true).await?;
			} else if !should_be_open && on_chain_status {
				info!(
					"Market {} for delivery at {} should be CLOSED. Submitting transaction...",
					rule.market_type, delivery_time
				);
				client.update_market_status(market_id, false).await?;
			}
		}
		current_slot += fifteen_mins_as_secs;
	}
	Ok(())
}

/// Generates a deterministic market ID.
pub fn generate_market_id(market_type: &str, delivery_timestamp: u64) -> H256 {
	let mut buffer = Vec::new();
	buffer.extend_from_slice(market_type.as_bytes());
	buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
	BlakeTwo256::hash_of(&buffer)
}

use crate::chain_connector::{self, GsyMarketOrchestratorNodeClient};
use crate::config::{Config, MARKET_RULES};
use blake2_rfc::blake2b::blake2b;
use gsy_offchain_primitives::{
	constants::GLOBAL_CONSTANTS, utils::timestamp_to_datetime_string, MarketType,
};
use std::thread::current;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subxt::utils::H256;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub async fn run(config: Config, client: GsyMarketOrchestratorNodeClient) -> anyhow::Result<()> {
	info!("Configuration: {:?}", config);

	info!("Waiting for orchestrator account to be registered as an operator...");
	loop {
		match client.is_operator_registered().await {
			Ok(true) => {
				info!("✅ Orchestrator account is registered. Starting main loop.");
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
		if let Err(e) = orchestrate_markets(&config, &client).await {
			error!("An error occurred during orchestration tick: {:?}", e);
		}
		sleep(interval).await;
	}
}

async fn orchestrate_markets(
	config: &Config,
	client: &GsyMarketOrchestratorNodeClient,
) -> anyhow::Result<()> {
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
	let look_ahead_horizon = now + (config.look_ahead_hours * 3600);

	let mut current_delivery_secs =
		(now / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec;

	info!("Orchestrator Check at {}. Looking ahead to {}", now, look_ahead_horizon);

	while current_delivery_secs <= look_ahead_horizon {
		for rule in MARKET_RULES.iter() {
			let market_id = generate_market_id(rule.market_type, current_delivery_secs);
			let open_time = (current_delivery_secs as i64 + rule.open_offset_mins * 60) as u64;
			let close_time = (current_delivery_secs as i64 + rule.close_offset_mins * 60) as u64;

			let on_chain_status = client.get_market_status(market_id).await?;
			let should_be_open = now >= open_time && now < close_time;

			if should_be_open && !on_chain_status {
				error!(
					"OPENING market '{:?}' for delivery at {}. Opening time {}.",
					rule.market_type,
					timestamp_to_datetime_string(current_delivery_secs),
					timestamp_to_datetime_string(open_time)
				);
				client.update_market_status(market_id, true).await?;
			} else if !should_be_open && on_chain_status {
				error!(
					"CLOSING market '{:?}' for delivery at {}. Closing time {}.",
					rule.market_type,
					timestamp_to_datetime_string(current_delivery_secs),
					timestamp_to_datetime_string(close_time)
				);
				client.update_market_status(market_id, false).await?;
			}
		}
		current_delivery_secs += GLOBAL_CONSTANTS.time_slot_sec;
	}
	Ok(())
}

pub fn generate_market_id(market_type: MarketType, delivery_timestamp: u64) -> H256 {
	let mut buffer = Vec::new();
	buffer.extend_from_slice(market_type.as_str().as_bytes());
	buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
	H256(blake2b(32, &[], &buffer).as_bytes().try_into().expect("hash is 32 bytes"))
}

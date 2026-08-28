use ::primitives::{log::setup_logging, utils::timestamp_to_datetime_string};
use clap::Parser;
use gsy_execution_engine::{
    services::execution_orchestrator::run_execution_cycle,
    timeslot_scheduler::{TimeslotScheduler, DEFAULT_ROLLOVER_RETRY_LIMIT},
    utils::cli::{Cli, Commands},
};
use std::env;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    setup_logging("gsy-execution-engine", "info");

    let cli = Cli::parse();
    match cli.command {
        Commands::Web3 {
            offchain_host,
            offchain_port,
            node_host,
            node_port,
            polling_interval,
            market_duration,
            penalty_rate,
        } => {
            info!("Starting engine...");
            let default_offchain_storage_url = format!("{}:{}", offchain_host, offchain_port);
            let offchain_url = env::var("OFFCHAIN_STORAGE_URL")
                .unwrap_or_else(|_| default_offchain_storage_url.clone());
            let evm_node_url = format!("{}:{}", node_host, node_port);
            let trade_settlement_address = env::var("TRADE_SETTLEMENT_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
            let execution_engine_private_key = env::var("EXECUTION_ENGINE_PRIVATE_KEY")
                .unwrap_or_else(|_| {
                    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
                });

            if trade_settlement_address == "0x0000000000000000000000000000000000000000" {
                info!(
                    "TRADE_SETTLEMENT_ADDRESS is zero; penalty submissions will fail until configured."
                );
            }
            info!("Using off-chain storage URL: {}", offchain_url);

            let rollover_retry_limit = env::var("EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(DEFAULT_ROLLOVER_RETRY_LIMIT);
            let mut timeslot_scheduler = TimeslotScheduler::new(rollover_retry_limit);

            loop {
                let timeslot = timeslot_scheduler.calculate_timeslot();
                info!(
                    "Execution cycle for timeslot {} ({})",
                    timestamp_to_datetime_string(timeslot),
                    timeslot
                );
                match run_execution_cycle(
                    &offchain_url,
                    &evm_node_url,
                    &trade_settlement_address,
                    &execution_engine_private_key,
                    timeslot,
                    penalty_rate,
                    market_duration,
                )
                .await
                {
                    Ok(processed_penalties) => {
                        timeslot_scheduler.record_cycle(timeslot, processed_penalties);
                    }
                    Err(e) => {
                        error!("Cycle failed for {}: {:?}", timeslot, e);
                        timeslot_scheduler.record_cycle(timeslot, 0);
                    }
                }
                info!("Sleeping for {}s...", polling_interval);
                tokio::time::sleep(std::time::Duration::from_secs(polling_interval)).await;
            }
        }
    }
}

use clap::Parser;
use gsy_execution_engine::{
    services::execution_orchestrator::run_execution_cycle,
    utils::{
        cli::{Cli, Commands},
        telemetry::{get_subscriber, init_subscriber},
    },
};
use gsy_offchain_primitives::{constants::GLOBAL_CONSTANTS, utils::timestamp_to_datetime_string};
use std::env;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let subscriber = get_subscriber("gsy-execution-engine", "info", std::io::stdout);
    init_subscriber(subscriber);

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
            let offchain_url = format!("{}:{}", offchain_host, offchain_port);
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

            loop {
                let timeslot = generate_previous_timeslot(market_duration);
                info!(
                    "Execution cycle for timeslot {} ({})",
                    timestamp_to_datetime_string(timeslot),
                    timeslot
                );
                if let Err(e) = run_execution_cycle(
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
                    error!("Cycle failed for {}: {:?}", timeslot, e);
                }
                info!("Sleeping for {}s...", polling_interval);
                tokio::time::sleep(std::time::Duration::from_secs(polling_interval)).await;
            }
        }
    }
}

fn generate_previous_timeslot(_market_duration: u64) -> u64 {
    use chrono::{Duration, Utc};

    let now = Utc::now();

    let prev = now - Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min);

    (prev.timestamp() as u64 / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

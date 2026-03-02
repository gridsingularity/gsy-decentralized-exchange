mod connectors;
mod primitives;
mod services;
mod utils;

use clap::Parser;
use gsy_offchain_primitives::{constants::GLOBAL_CONSTANTS, utils::timestamp_to_datetime_string};
use services::execution_orchestrator::run_execution_cycle;
use tracing::{error, info};
use utils::cli::{Cli, Commands};
use utils::telemetry::{get_subscriber, init_subscriber};

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
            let node_url = format!("{}:{}", node_host, node_port);

            loop {
                let timeslot = generate_previous_timeslot(market_duration);
                info!(
                    "Execution cycle for timeslot {} ({})",
                    timestamp_to_datetime_string(timeslot),
                    timeslot
                );
                if let Err(e) = run_execution_cycle(
                    &offchain_url,
                    &node_url,
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

fn generate_previous_timeslot(market_duration: u64) -> u64 {
    use chrono::{Duration, Utc};

    let now = Utc::now();

    let prev = now - Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min);

    (prev.timestamp() as u64 / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

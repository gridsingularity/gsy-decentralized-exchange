mod primitives;
mod services;
mod connectors;
mod utils;

use clap::Parser;
use tracing::{error, info};
use utils::cli::{Cli, Commands};
use utils::telemetry::{get_subscriber, init_subscriber};
use services::execution_orchestrator::run_execution_cycle;

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
            polling_interval 
        } => {
            info!("Starting engine...");
            let offchain_url = format!("{}:{}", offchain_host, offchain_port);
            let node_url = format!("{}:{}", node_host, node_port);

            loop {
                let timeslot = generate_previous_timeslot();
                if let Err(e) = run_execution_cycle(&offchain_url, &node_url, &timeslot).await {
                    error!("Cycle failed for {}: {:?}", timeslot, e);
                }
                info!("Sleeping for {}s...", polling_interval);
                tokio::time::sleep(std::time::Duration::from_secs(polling_interval)).await;
            }
        }
    }
}

fn generate_previous_timeslot() -> String {
    use chrono::{Utc, Duration};
    
    let now = Utc::now();
    let prev = now - Duration::seconds(30);

    prev.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

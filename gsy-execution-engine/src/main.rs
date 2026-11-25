use ::primitives::{log::setup_logging, utils::timestamp_to_datetime_string};
use clap::Parser;
use gsy_execution_engine::{
    services::execution_orchestrator::{finalize_timeslot, run_execution_cycle},
    timeslot_scheduler::TimeslotScheduler,
    utils::cli::{Cli, Commands},
};
use std::env;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging("gsy-execution-engine", "info");

    let cli = Cli::parse();
    ::primitives::actor_ids::initialize_actor_id_hashing()?;
    match cli.command {
        Commands::Web3 {
            offchain_host,
            offchain_port,
            node_host,
            node_port,
            polling_interval,
            market_duration,
            penalty_rate,
            penalty_tolerance_percent,
        } => {
            info!("Starting engine...");
            let default_offchain_storage_url = format!("{}:{}", offchain_host, offchain_port);
            let offchain_url = env::var("OFFCHAIN_STORAGE_URL")
                .unwrap_or_else(|_| default_offchain_storage_url.clone());
            let evm_node_url = format!("{}:{}", node_host, node_port);
            let trade_matching_address = env::var("TRADE_MATCHING_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
            let execution_engine_private_key = env::var("EXECUTION_ENGINE_PRIVATE_KEY")
                .unwrap_or_else(|_| {
                    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
                });

            if trade_matching_address == "0x0000000000000000000000000000000000000000" {
                info!(
                    "TRADE_MATCHING_ADDRESS is zero; delivery verification will fail until configured."
                );
            }
            info!("Using off-chain storage URL: {}", offchain_url);

            let mut timeslot_scheduler = TimeslotScheduler::from_env()?;
            let mut finalized_timeslot = None;

            loop {
                let timeslot = timeslot_scheduler.calculate_timeslot();
                info!(
                    "Execution cycle for timeslot {} ({})",
                    timestamp_to_datetime_string(timeslot),
                    timeslot
                );
                if let Err(e) = run_execution_cycle(
                    &offchain_url,
                    &evm_node_url,
                    &trade_matching_address,
                    &execution_engine_private_key,
                    timeslot,
                    penalty_rate,
                    market_duration,
                    penalty_tolerance_percent,
                )
                .await
                {
                    error!("Cycle failed for {}: {:?}", timeslot, e);
                }
                // Once a slot's grace period is over, close its remaining trades: a party
                // still without a usable reading is penalized on its whole net position.
                // Retried until complete, for as long as this is the latest expired slot.
                if let Some(expired) = timeslot_scheduler
                    .expired_timeslot()
                    .filter(|expired| finalized_timeslot != Some(*expired))
                {
                    info!(
                        "Finalizing timeslot {} ({}) after its grace period",
                        timestamp_to_datetime_string(expired),
                        expired
                    );
                    match finalize_timeslot(
                        &offchain_url,
                        &evm_node_url,
                        &trade_settlement_address,
                        &execution_engine_private_key,
                        expired,
                        penalty_rate,
                        penalty_tolerance_percent,
                    )
                    .await
                    {
                        Ok(true) => finalized_timeslot = Some(expired),
                        Ok(false) => info!(
                            "Timeslot {} still has trades unknown to the contract; retrying",
                            expired
                        ),
                        Err(e) => error!("Finalization failed for {}: {:?}", expired, e),
                    }
                }
                info!("Sleeping for {}s...", polling_interval);
                tokio::time::sleep(std::time::Duration::from_secs(polling_interval)).await;
            }
        }
    }
}

use ::primitives::{
    constants::GLOBAL_CONSTANTS, log::setup_logging, utils::timestamp_to_datetime_string,
};
use clap::Parser;
use gsy_execution_engine::{
    services::execution_orchestrator::run_execution_cycle,
    utils::cli::{Cli, Commands},
};
use std::env;
use tracing::{error, info};

const DEFAULT_ROLLOVER_RETRY_LIMIT: u32 = 2;

#[derive(Debug)]
struct PendingTimeslot {
    timeslot: u64,
    retries_remaining: u32,
}

#[derive(Debug)]
struct ExecutionTimeslotScheduler {
    latest_target_timeslot: u64,
    pending_timeslot: Option<PendingTimeslot>,
    rollover_retry_limit: u32,
}

impl ExecutionTimeslotScheduler {
    fn new(initial_timeslot: u64, rollover_retry_limit: u32) -> Self {
        Self {
            latest_target_timeslot: initial_timeslot,
            pending_timeslot: None,
            rollover_retry_limit,
        }
    }

    fn next_timeslot(&mut self, current_target_timeslot: u64) -> u64 {
        if current_target_timeslot != self.latest_target_timeslot {
            if self.rollover_retry_limit > 0 {
                info!(
                    "Target timeslot advanced from {} to {}; retaining {} for up to {} retries",
                    self.latest_target_timeslot,
                    current_target_timeslot,
                    self.latest_target_timeslot,
                    self.rollover_retry_limit
                );
                self.pending_timeslot = Some(PendingTimeslot {
                    timeslot: self.latest_target_timeslot,
                    retries_remaining: self.rollover_retry_limit,
                });
            }
            self.latest_target_timeslot = current_target_timeslot;
        }

        self.pending_timeslot
            .as_ref()
            .map(|pending| pending.timeslot)
            .unwrap_or(current_target_timeslot)
    }

    fn record_cycle(&mut self, timeslot: u64, processed_penalties: usize) {
        let Some(pending) = self.pending_timeslot.as_mut() else {
            return;
        };
        if pending.timeslot != timeslot {
            return;
        }

        if processed_penalties > 0 {
            info!(
                "Finished retained timeslot {} after processing {} penalties",
                timeslot, processed_penalties
            );
            self.pending_timeslot = None;
        } else if pending.retries_remaining <= 1 {
            info!(
                "Finished retained timeslot {} after exhausting rollover retries",
                timeslot
            );
            self.pending_timeslot = None;
        } else {
            pending.retries_remaining -= 1;
            info!(
                "Retained timeslot {} has {} retries remaining",
                timeslot, pending.retries_remaining
            );
        }
    }
}

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
            let initial_timeslot = generate_target_timeslot();
            let mut timeslot_scheduler =
                ExecutionTimeslotScheduler::new(initial_timeslot, rollover_retry_limit);

            loop {
                let current_target_timeslot = generate_target_timeslot();
                let timeslot = timeslot_scheduler.next_timeslot(current_target_timeslot);
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

fn generate_target_timeslot() -> u64 {
    use chrono::{Duration, Utc};

    let now = Utc::now();

    let prev = now - Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min);

    (prev.timestamp() as u64 / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

#[cfg(test)]
mod tests {
    use super::ExecutionTimeslotScheduler;

    #[test]
    fn uses_current_target_without_a_rollover() {
        let mut scheduler = ExecutionTimeslotScheduler::new(900, 2);

        assert_eq!(scheduler.next_timeslot(900), 900);
    }

    #[test]
    fn retries_outgoing_timeslot_after_rollover() {
        let mut scheduler = ExecutionTimeslotScheduler::new(900, 2);

        assert_eq!(scheduler.next_timeslot(1_800), 900);
        scheduler.record_cycle(900, 0);
        assert_eq!(scheduler.next_timeslot(1_800), 900);
        scheduler.record_cycle(900, 0);
        assert_eq!(scheduler.next_timeslot(1_800), 1_800);
    }

    #[test]
    fn releases_outgoing_timeslot_after_processing_penalties() {
        let mut scheduler = ExecutionTimeslotScheduler::new(900, 2);

        assert_eq!(scheduler.next_timeslot(1_800), 900);
        scheduler.record_cycle(900, 2);
        assert_eq!(scheduler.next_timeslot(1_800), 1_800);
    }

    #[test]
    fn can_disable_rollover_retries() {
        let mut scheduler = ExecutionTimeslotScheduler::new(900, 0);

        assert_eq!(scheduler.next_timeslot(1_800), 1_800);
    }
}

use clap::Parser;
use gsy_matching_engine::algorithms::PayAsClearPricing;
use gsy_matching_engine::connectors::evm_subscribe;
use gsy_matching_engine::markets::MarketCache;
use gsy_matching_engine::utils::{Cli, Commands};
use primitives::log::setup_logging;
use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
use std::env;
use std::{thread, time};
use tracing::{error, info};

/// Pricing of every pay-as-clear market. Any market can use pay-as-clear, so
/// an invalid value always stops the engine at startup.
fn configured_pay_as_clear_pricing() -> PayAsClearPricing {
    match env::var("PAY_AS_CLEAR_PRICING") {
        Ok(value) => value
            .parse::<PayAsClearPricing>()
            .unwrap_or_else(|error| panic!("Invalid PAY_AS_CLEAR_PRICING: {}", error)),
        Err(env::VarError::NotPresent) => PayAsClearPricing::default(),
        Err(error) => panic!("Invalid PAY_AS_CLEAR_PRICING: {}", error),
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    setup_logging("matching_engine", "info");

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Commands::Web3 {
            orderbook_host,
            orderbook_port,
            node_host,
            node_port,
        } => {
            async {
                let default_offchain_storage_url = format!("{}:{}", orderbook_host, orderbook_port);
                let offchain_storage_url = env::var("OFFCHAIN_STORAGE_URL")
                    .unwrap_or_else(|_| default_offchain_storage_url.clone());
                let orderbook_url = format!(
                    "{}/orders",
                    offchain_storage_url.trim_end_matches('/')
                );
                let node_url = format!("{}:{}", node_host, node_port);
                let trade_settlement_address = env::var("TRADE_SETTLEMENT_ADDRESS")
                    .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
                let matching_engine_private_key = env::var("MATCHING_ENGINE_PRIVATE_KEY")
                    .unwrap_or_else(|_| {
                        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                            .to_string()
                    });
                let pay_as_clear_pricing = configured_pay_as_clear_pricing();
                // Each order book is matched with the algorithm of its market record.
                let mut markets = MarketCache::new(OffchainStorageClient::new(
                    OffchainStorageTransport::from_env(),
                    offchain_storage_url.clone(),
                    "EWDS_MATCHING_ENGINE_CLIENT_ID",
                    "gsymatchingengine",
                ));

                if trade_settlement_address == "0x0000000000000000000000000000000000000000" {
                    info!(
                        "TRADE_SETTLEMENT_ADDRESS is zero; settlement submissions will fail until configured."
                    );
                }
                info!("Using off-chain storage URL: {}", offchain_storage_url);
                info!("Using pay-as-clear pricing: {}", pay_as_clear_pricing);

                if let Err(error) =
                    evm_subscribe(
                        orderbook_url.clone(),
                        node_url.clone(),
                        trade_settlement_address.clone(),
                        matching_engine_private_key.clone(),
                        &mut markets,
                        pay_as_clear_pricing,
                    )
                    .await
                {
                    info!("Error - {:?}", error);
                    let mut attempt: u8 = 1;
                    while attempt <= cli.max_attempts {
                        info!("Retrying...\nAttempt: {:}", attempt);
                        let two_seconds = time::Duration::from_millis(2000);
                        thread::sleep(two_seconds);
                        if let Err(error) =
                            evm_subscribe(
                                orderbook_url.clone(),
                                node_url.clone(),
                                trade_settlement_address.clone(),
                                matching_engine_private_key.clone(),
                                &mut markets,
                                pay_as_clear_pricing,
                            )
                            .await
                        {
                            error!("Error - {:?}", error);
                            attempt += 1;
                        }
                    }
                }
            }
            .await
        }
    }
}

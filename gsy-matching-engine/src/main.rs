use clap::Parser;
use gsy_matching_engine::connectors::{evm_subscribe, redis_subscribe};
use gsy_matching_engine::utils::telemetry::{get_subscriber, init_subscriber};
use gsy_matching_engine::utils::{Cli, Commands};
use std::env;
use std::{thread, time};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let subscriber = get_subscriber("matching_engine".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Commands::Web2 {
            orderbook_host,
            orderbook_port,
        } => {
            async {
                let orders_response_channel =
                    String::from("external-matching-engine/*/offers-bids/response/");
                let recommendations_channel =
                    String::from("external-matching-engine/*/recommendations");
                let tick_channel = String::from("external-matching-engine/*/events/");

                let channels = vec![
                    tick_channel.clone(),
                    orders_response_channel.clone(),
                    recommendations_channel.clone(),
                ];

                info!("Connecting to: {}:{}", orderbook_host, orderbook_port);

                let url = format!("{}:{}", orderbook_host, orderbook_port);

                if let Err(error) = redis_subscribe(channels.clone(), url).await {
                    error!("Error - {:?}", error);
                    panic!("{:?}", error);
                }
            }
            .await
        }
        Commands::Web3 {
            orderbook_host,
            orderbook_port,
            node_host,
            node_port,
        } => {
            async {
                let orderbook_url = format!("{}:{}/{}", orderbook_host, orderbook_port, "orders");
                let node_url = format!("{}:{}", node_host, node_port);
                let trade_settlement_address = env::var("TRADE_SETTLEMENT_ADDRESS")
                    .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
                let matching_engine_private_key = env::var("MATCHING_ENGINE_PRIVATE_KEY")
                    .unwrap_or_else(|_| {
                        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                            .to_string()
                    });

                if trade_settlement_address == "0x0000000000000000000000000000000000000000" {
                    info!(
                        "TRADE_SETTLEMENT_ADDRESS is zero; settlement submissions will fail until configured."
                    );
                }

                if let Err(error) =
                    evm_subscribe(
                        orderbook_url.clone(),
                        node_url.clone(),
                        trade_settlement_address.clone(),
                        matching_engine_private_key.clone(),
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

use clap::Parser;
use gsy_matching_engine::connectors::{redis_subscribe, substrate_subscribe};
use gsy_matching_engine::utils::{Cli, Commands, MatchingAlgorithmType};
use std::{thread, time};
use tracing::{error, info};
use gsy_matching_engine::utils::telemetry::{get_subscriber, init_subscriber};

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
            orderbook_port
        } => async {
            let orders_response_channel = String::from("external-matching-engine/*/offers-bids/response/");
            let recommendations_channel = String::from("external-matching-engine/*/recommendations");
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
        }.await,
        Commands::Web3 {
            orderbook_host,
            orderbook_port,
            node_host,
            node_port,
            algorithm
        } => async {
            let orderbook_url = format!("{}:{}/{}", orderbook_host, orderbook_port, "orders");
            let node_url = format!("{}:{}", node_host, node_port);
            if let Err(error) = substrate_subscribe(orderbook_url.clone(), node_url.clone(), algorithm.clone()).await {
                info!("Error - {:?}", error);
                let mut attempt: u8 = 1;
                while attempt <= cli.max_attempts {
                    info!("Retrying...\nAttempt: {:}", attempt);
                    let two_seconds = time::Duration::from_millis(2000);
                    thread::sleep(two_seconds);
                    if let Err(error) = substrate_subscribe(orderbook_url.clone(), node_url.clone(), algorithm.clone()).await {
                        error!("Error - {:?}", error);
                        attempt += 1;
                    }
                }
            }
        }.await
    }
}
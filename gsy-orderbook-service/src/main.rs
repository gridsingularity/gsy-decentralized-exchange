use actix_web::web;
use anyhow::{Error, Result};
use gsy_ethers_listener::{GsyEthersListener, ListenerConfig};
use gsy_orderbook_service::configuration::get_configuration;
use gsy_orderbook_service::db::{init_database, DbRef};
use gsy_orderbook_service::evm_handler::OrderbookEvmHandler;
use gsy_orderbook_service::ewds_handler::{start_ewds_request_handler, EwdsHandlerConfig};
use gsy_orderbook_service::scheduler::start_scheduler;
use gsy_orderbook_service::startup::run;
use gsy_orderbook_service::telemetry::{get_subscriber, init_subscriber};
use std::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv::dotenv().ok();

    let subscriber = get_subscriber(
        "gsy-orderbook-service".into(),
        "info".into(),
        std::io::stdout,
    );
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to load configuration");
    let db_connection_string = configuration.get_connection_string();
    let scheduler_interval = configuration.get_scheduler_interval();

    let db_connection_wrapper =
        init_database(db_connection_string, configuration.database_name).await?;
    let db: DbRef = web::Data::new(db_connection_wrapper.clone());

    info!("🚀 Starting Orderbook Service with EVM Listener");
    let db_for_listener = db_connection_wrapper.clone();
    let listener_config = ListenerConfig {
        node_url: configuration.evm_node_url.clone(),
        order_registry_address: configuration.contract_order_registry,
        trade_settlement_address: configuration.contract_trade_settlement,
        market_controller_address: configuration.contract_market_controller,
    };

    tokio::task::spawn(async move {
        let handler = OrderbookEvmHandler {
            db: db_for_listener,
        };
        let listener = GsyEthersListener::new(listener_config, handler);
        if let Err(e) = listener.run().await {
            tracing::error!("EVM Listener crashed: {:?}", e);
        }
    });

    tokio::task::spawn(async move {
        start_scheduler(db, scheduler_interval).await;
    });

    let ewds_config = EwdsHandlerConfig::from_env();
    if ewds_config.enabled {
        let db_for_ewds = db_connection_wrapper.clone();
        tokio::task::spawn(async move {
            start_ewds_request_handler(db_for_ewds, ewds_config).await;
        });
    }

    let address = format!(
        "{}:{}",
        configuration.application_host, configuration.application_port
    );
    let listener = TcpListener::bind(address).expect("Failed to bind");

    info!("Server listening on {}", listener.local_addr().unwrap());

    match run(listener, db_connection_wrapper)?.await {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::from(e)),
    }
}

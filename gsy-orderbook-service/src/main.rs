use actix_web::web;
use anyhow::{Error, Result};
use gsy_orderbook_service::configuration::get_configuration;
use gsy_orderbook_service::db::{init_database, DbRef};
use gsy_orderbook_service::event_listener::init_event_listener;
use gsy_orderbook_service::scheduler::start_scheduler;
use gsy_orderbook_service::startup::run;
use gsy_orderbook_service::telemetry::{get_subscriber, init_subscriber};
use std::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
	dotenv::dotenv().ok();

	let subscriber = get_subscriber("gsy-orderbook-service".into(), "info".into(), std::io::stdout);
	init_subscriber(subscriber);
	let configuration = get_configuration().expect("Failed to load configuration");
	let db_connection_string = configuration.get_connection_string();
	let node_url = configuration.get_node_url();
	let scheduler_interval = configuration.get_scheduler_interval();
	let db_connection_wrapper =
		init_database(db_connection_string, configuration.database_name).await?;
	let db: DbRef = web::Data::new(db_connection_wrapper.clone());
	let db_event_listener_instance = web::Data::clone(&db);
	if !node_url.is_empty() {
		tokio::task::spawn(async move {
			let _ = init_event_listener(db_event_listener_instance, node_url).await;
		});
		tokio::task::spawn(async move {
			start_scheduler(db, scheduler_interval).await;
		});
	}
	let address = format!("{}:{}", configuration.application_host, configuration.application_port);
	let listener = TcpListener::bind(address).expect("Failed to bind");
	match run(listener, db_connection_wrapper)?.await {
		Ok(_) => Ok(()),
		Err(e) => Err(Error::from(e)),
	}
}

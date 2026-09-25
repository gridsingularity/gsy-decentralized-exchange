pub mod readers;

use crate::config::Config;
use anyhow::Result;
use mongodb::bson::doc;
use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use std::time::Duration;
use tracing::{info, warn};

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const SERVER_SELECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// The offchain-storage database the engine reads from, and the database it writes KPIs to.
#[derive(Clone)]
pub struct Databases {
    pub source: Database,
    pub results: Database,
}

/// Delay before the next connection attempt: doubles after each failure, capped at 30 s.
pub fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(MAX_BACKOFF)
}

async fn try_connect(config: &Config) -> Result<Databases> {
    let mut options = ClientOptions::parse(config.database.connection_string()).await?;
    options.app_name = Some("gsy-analytics-engine".to_string());
    options.server_selection_timeout = Some(SERVER_SELECTION_TIMEOUT);
    let client = Client::with_options(options)?;
    let source = client.database(&config.database.name);
    source.run_command(doc! { "ping": 1 }).await?;
    Ok(Databases {
        source,
        results: client.database(&config.results_database_name),
    })
}

/// Connects to MongoDB, retrying with exponential backoff until the server answers a ping.
pub async fn connect(config: &Config) -> Databases {
    let mut delay = INITIAL_BACKOFF;
    loop {
        match try_connect(config).await {
            Ok(databases) => {
                info!(
                    host = %config.database.host,
                    source_database = %config.database.name,
                    results_database = %config.results_database_name,
                    "Connected to MongoDB"
                );
                return databases;
            }
            Err(error) => {
                warn!(
                    "Could not connect to MongoDB at {}: {}. Retrying in {:?}",
                    config.database.host, error, delay
                );
                tokio::time::sleep(delay).await;
                delay = next_backoff(delay);
            }
        }
    }
}

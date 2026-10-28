mod feature_selection;
mod steps;
mod utils;
mod world;

use anyhow::Result;
use cucumber::World as _;
use mongodb::options::ClientOptions;
use primitives::MatchingAlgorithm;
use std::env;
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub async fn delete_database() -> Result<()> {
    let db_url = std::env::var("MONGO_URL").unwrap_or_else(|_| {
        "mongodb://gsy:gsy@mongodb:27017/?retryWrites=true&w=majority".to_string()
    });
    let db_name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "offchain_storage".to_string());
    let options = ClientOptions::parse(&db_url).await?;
    let client = mongodb::Client::with_options(options)?;
    let db = client.database(db_name.as_str());
    for name in db.list_collection_names().await? {
        if !name.starts_with("system.") {
            db.collection::<mongodb::bson::Document>(&name)
                .delete_many(mongodb::bson::doc! {})
                .await?;
        }
    }
    info!("Cleared test collections, preserving indexes");
    Ok(())
}

/// Polls the off-chain storage health check until it answers with success, and panics if it
/// does not within `OFFCHAIN_STORAGE_HEALTH_TIMEOUT`. Every feature relies on the service.
async fn wait_for_offchain_storage() {
    const OFFCHAIN_STORAGE_HEALTH_TIMEOUT: Duration = Duration::from_secs(60);
    const POLL_INTERVAL: Duration = Duration::from_secs(2);

    let base_url =
        env::var("OFFCHAIN_STORAGE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let url = format!("{}/health_check", base_url);
    let client = reqwest::Client::new();
    let started = Instant::now();
    loop {
        let last_error = match client.get(&url).timeout(POLL_INTERVAL).send().await {
            Ok(response) if response.status().is_success() => {
                info!("Off-chain storage is healthy at {}", base_url);
                return;
            }
            Ok(response) => format!("status {}", response.status()),
            Err(error) => error.to_string(),
        };
        if started.elapsed() >= OFFCHAIN_STORAGE_HEALTH_TIMEOUT {
            panic!(
                "Off-chain storage at {} is not healthy after {:?}: {}",
                base_url, OFFCHAIN_STORAGE_HEALTH_TIMEOUT, last_error
            );
        }
        sleep(POLL_INTERVAL).await;
    }
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    println!("Waiting for services to start...");
    sleep(Duration::from_secs(30)).await;
    wait_for_offchain_storage().await;

    let matching_algorithm =
        env::var("MATCHING_ALGORITHM").unwrap_or_else(|_| MatchingAlgorithm::default().to_string());
    let matching_algorithm = MatchingAlgorithm::from_str(&matching_algorithm)
        .unwrap_or_else(|error| panic!("Invalid MATCHING_ALGORITHM: {}", error));
    let feature_override = env::var("E2E_FEATURE_PATH").ok();
    let feature_selection = feature_override.clone().unwrap_or_else(|| {
        format!(
            "features/{} and features/matching_algorithms",
            matching_algorithm.as_str()
        )
    });
    let mut cli = cucumber::cli::Opts::<cucumber::parser::basic::Cli, _, _>::parsed();
    feature_selection::configure_features(
        &mut cli.parser,
        &matching_algorithm,
        feature_override.as_deref(),
    );
    let feature_path = feature_override.unwrap_or_else(|| "features".to_string());

    println!(
        "Running {} E2E feature(s) from {}",
        matching_algorithm, feature_selection
    );

    // @ewds features publish EWDS events, so they only run with the EWDS transport.
    let ewds_transport = env::var("OFFCHAIN_STORAGE_TRANSPORT")
        .map(|transport| transport.eq_ignore_ascii_case("ewds"))
        .unwrap_or(false);

    world::MyWorld::cucumber()
        .with_cli(cli)
        .max_concurrent_scenarios(1)
        .after(|_feature, _rule, _scenario, _ev, _world| {
            Box::pin(async move {
                delete_database()
                    .await
                    .expect("Failed to clear E2E records between scenarios");
            })
        })
        .filter_run_and_exit(feature_path, move |feature, _rule, scenario| {
            ewds_transport
                || !feature
                    .tags
                    .iter()
                    .chain(&scenario.tags)
                    .any(|tag| tag == "ewds")
        })
        .await;
}

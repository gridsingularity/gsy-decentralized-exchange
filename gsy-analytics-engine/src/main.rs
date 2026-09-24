use anyhow::Result;
use gsy_analytics_engine::{config::Config, db};
use primitives::log::setup_logging;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging("gsy-analytics-engine", "info");

    info!("Starting GSY Analytics Engine...");
    let config = Config::from_env()?;
    info!("Loaded config: {:?}", config);
    if config.tariffs.is_empty() {
        warn!(
            "No grid tariff configured (ANALYTICS_GRID_TARIFF_EUR_PER_KWH / \
             ANALYTICS_GRID_TARIFF_OVERRIDES). Procurement cost per kWh values and baselines will be null"
        );
    }

    tokio::select! {
        _databases = db::connect(&config) => {}
        _ = shutdown_signal() => {
            info!("Shutdown requested before MongoDB connection was established");
            return Ok(());
        }
    }

    shutdown_signal().await;
    info!("Shutting down GSY Analytics Engine");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

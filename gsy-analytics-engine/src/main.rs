use anyhow::Result;
use chrono::Utc;
use gsy_analytics_engine::engine::Engine;
use gsy_analytics_engine::{api, config::Config, db, kpi, scheduler};
use primitives::log::setup_logging;
use std::net::TcpListener;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging("gsy-analytics-engine", "info");

    info!("Starting GSY Analytics Engine...");
    let config = Config::from_env()?;
    info!("Loaded config: {:?}", config);
    let kpis = kpi::build_registry(&config)?;
    info!(
        "Enabled KPIs: {}",
        kpis.iter()
            .map(|kpi| kpi.id())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if config.tariffs.is_empty() {
        warn!(
            "No grid tariff configured (ANALYTICS_GRID_TARIFF_EUR_PER_KWH / \
             ANALYTICS_GRID_TARIFF_OVERRIDES). Procurement cost per kWh values and baselines will be null"
        );
    }

    let databases = tokio::select! {
        databases = db::connect(&config) => databases,
        _ = shutdown_signal() => {
            info!("Shutdown requested before MongoDB connection was established");
            return Ok(());
        }
    };

    let engine = Engine::new(config, kpis, databases);
    engine.ensure_indexes().await?;

    let listener = TcpListener::bind(engine.config().api_address())?;
    info!("HTTP API listening on {}", listener.local_addr()?);
    let server = api::run_http_server(listener, engine.results_collection())?;
    let server_handle = server.handle();
    let server_task = tokio::spawn(server);

    if let Some(from) = engine.config().backfill_from {
        info!("Backfilling KPIs from {}", from);
        tokio::select! {
            result = engine.run_backfill(from, Utc::now().timestamp()) => {
                if let Err(error) = result {
                    error!("KPI backfill failed: {:#}", error);
                }
            }
            _ = shutdown_signal() => {
                info!("Shutting down GSY Analytics Engine during backfill");
                server_handle.stop(true).await;
                return Ok(());
            }
        }
    }

    scheduler::run(&engine, shutdown_signal()).await;
    info!("Shutting down GSY Analytics Engine");
    server_handle.stop(true).await;
    if let Err(error) = server_task.await? {
        error!("HTTP API stopped with an error: {}", error);
    }
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

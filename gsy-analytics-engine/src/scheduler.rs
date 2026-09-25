use crate::engine::{Engine, TickStats};
use chrono::Utc;
use std::future::Future;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

fn log_tick(stats: &TickStats) {
    let reading_stats = &stats.load.reading_stats;
    info!(
        window_start = stats.window.start,
        window_end = stats.window.end,
        communities = stats.load.communities,
        trades = stats.load.trades,
        unassigned_trades = stats.load.unassigned_trades,
        readings = stats.load.readings,
        skipped_points = reading_stats.skipped_points,
        values_without_owner = reading_stats.values_without_owner,
        invalid_timestamps = reading_stats.invalid_timestamps,
        results_upserted = stats.results_upserted,
        duration_ms = stats.duration.as_millis() as u64,
        "KPI tick finished"
    );
    if stats.load.unassigned_trades > 0 {
        warn!(
            "{} trades matched no known community and were left out",
            stats.load.unassigned_trades
        );
    }
}

/// Runs a tick immediately and then every `ANALYTICS_INTERVAL_SECONDS` until `shutdown`
/// completes. A failed tick is logged and retried on the next interval; a tick that overruns
/// the interval makes the next one skip rather than pile up.
pub async fn run(engine: &Engine, shutdown: impl Future<Output = ()>) {
    let mut interval = tokio::time::interval(Duration::from_secs(engine.config().interval_seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => return,
            _ = interval.tick() => {}
        }
        tokio::select! {
            _ = &mut shutdown => return,
            result = engine.run_tick(Utc::now().timestamp()) => match result {
                Ok(stats) => log_tick(&stats),
                Err(error) => error!("KPI tick failed: {:#}", error),
            },
        }
    }
}

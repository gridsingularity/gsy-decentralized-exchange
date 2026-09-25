use crate::config::Config;
use crate::db::readers::{load_dataset, LoadStats};
use crate::db::results::{ensure_indexes, upsert_results};
use crate::db::Databases;
use crate::kpi::{combined_requirements, Kpi, KpiContext};
use crate::period::{align_to_slot, Window};
use anyhow::Result;
use mongodb::bson::Document;
use mongodb::Collection;
use primitives::constants::GLOBAL_CONSTANTS;
use std::time::{Duration, Instant};
use tracing::info;

const BACKFILL_CHUNK_SECONDS: i64 = 24 * 3600;

#[derive(Debug, Clone, PartialEq)]
pub struct TickStats {
    pub window: Window,
    pub load: LoadStats,
    pub results_upserted: usize,
    pub duration: Duration,
}

pub struct Engine {
    config: Config,
    kpis: Vec<Box<dyn Kpi>>,
    databases: Databases,
    results: Collection<Document>,
    slot_length: i64,
}

impl Engine {
    pub fn new(config: Config, kpis: Vec<Box<dyn Kpi>>, databases: Databases) -> Self {
        let results = databases
            .results
            .collection::<Document>(&config.results_collection);
        Engine {
            config,
            kpis,
            databases,
            results,
            slot_length: GLOBAL_CONSTANTS.time_slot_sec as i64,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn ensure_indexes(&self) -> Result<()> {
        ensure_indexes(&self.results).await
    }

    /// The window a tick at `now` recomputes.
    pub fn tick_window(&self, now: i64) -> Window {
        Window::for_tick(
            now,
            self.config.lookback_hours,
            self.config.settlement_delay_minutes,
            self.slot_length,
        )
    }

    /// Recomputes and upserts every enabled KPI over the tick window.
    pub async fn run_tick(&self, now: i64) -> Result<TickStats> {
        self.run_window(self.tick_window(now), now).await
    }

    /// Computes everything from `from` up to the start of the first tick window, one day at a
    /// time, so a long history does not have to be loaded at once.
    pub async fn run_backfill(&self, from: i64, now: i64) -> Result<Vec<TickStats>> {
        let until = self.tick_window(now).start;
        let mut start = align_to_slot(from, self.slot_length);
        let mut stats = Vec::new();
        while start < until {
            let end = (start + BACKFILL_CHUNK_SECONDS).min(until);
            let window = Window {
                start,
                end,
                slot_length: self.slot_length,
            };
            let chunk = self.run_window(window, now).await?;
            info!(
                window_start = window.start,
                window_end = window.end,
                results_upserted = chunk.results_upserted,
                "Backfilled KPI window"
            );
            stats.push(chunk);
            start = end;
        }
        Ok(stats)
    }

    async fn run_window(&self, window: Window, now: i64) -> Result<TickStats> {
        let started = Instant::now();
        let (dataset, load) = load_dataset(
            &self.databases.source,
            &window,
            combined_requirements(&self.kpis),
        )
        .await?;

        let context = KpiContext {
            window: &window,
            dataset: &dataset,
            computed_at: now,
        };
        let mut results_upserted = 0;
        for kpi in &self.kpis {
            let results = kpi.compute(&context);
            results_upserted += upsert_results(&self.results, &results).await?;
        }

        Ok(TickStats {
            window,
            load,
            results_upserted,
            duration: started.elapsed(),
        })
    }
}

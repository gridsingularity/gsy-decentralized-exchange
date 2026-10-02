//! Runs the engine against a real MongoDB, using the `DATABASE_*` env vars like the other
//! services' integration tests. Each test uses its own randomly named database and drops it.

use gsy_analytics_engine::config::Config;
use gsy_analytics_engine::db::results::{PERIOD_START_INDEX, RESULT_KEY_INDEX};
use gsy_analytics_engine::db::{connect, Databases};
use gsy_analytics_engine::engine::Engine;
use gsy_analytics_engine::kpi::build_registry;
use gsy_analytics_engine::mapping::onchain_id;
use mongodb::bson::{doc, Document};
use mongodb::Database;
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::kpi::ProcurementCostResultSchema;
use primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeParameters, TradeStatus};
use primitives::utils::{bytes16_to_hex, generate_market_id, timestamp_to_string_with_padding};
use primitives::MarketType;
use std::time::Duration;
use uuid::Uuid;

const SLOT: i64 = 1_758_621_600; // 2025-09-23 10:00 UTC
const SLOT_LENGTH: i64 = 900;
const COMMUNITY: &str = "Pilot1";
const EPSILON: f64 = 1e-9;
/// A tick at 10:30 with the default 15 min delay computes up to and including the 10:00 slot.
const NOW: i64 = SLOT + 2 * SLOT_LENGTH;

struct TestDb {
    config: Config,
    databases: Databases,
}

impl TestDb {
    async fn new() -> Self {
        let overridden = [
            "DATABASE_NAME",
            "ANALYTICS_RESULTS_DATABASE_NAME",
            "ANALYTICS_GRID_TARIFF_EUR_PER_KWH",
            "ANALYTICS_GRID_TARIFF_OVERRIDES",
        ];
        let mut vars: Vec<(String, String)> = std::env::vars()
            .filter(|(key, _)| !overridden.contains(&key.as_str()))
            .collect();
        vars.push(("DATABASE_NAME".to_string(), Uuid::new_v4().to_string()));
        vars.push((
            "ANALYTICS_GRID_TARIFF_EUR_PER_KWH".to_string(),
            "0.30".to_string(),
        ));
        let config = Config::from_vars(vars).unwrap();
        let databases = tokio::time::timeout(Duration::from_secs(60), connect(&config))
            .await
            .expect("Could not connect to MongoDB; check the DATABASE_* env vars");
        TestDb { config, databases }
    }

    fn source(&self) -> &Database {
        &self.databases.source
    }

    fn engine(&self) -> Engine {
        let kpis = build_registry(&self.config).unwrap();
        Engine::new(self.config.clone(), kpis, self.databases.clone())
    }

    async fn results(&self) -> Vec<ProcurementCostResultSchema> {
        let mut cursor = self
            .databases
            .results
            .collection::<ProcurementCostResultSchema>(&self.config.results_collection)
            .find(doc! {})
            .sort(doc! {"community_id": 1, "period_start": 1})
            .await
            .unwrap();
        let mut results = Vec::new();
        while cursor.advance().await.unwrap() {
            results.push(cursor.deserialize_current().unwrap());
        }
        results
    }

    async fn drop(self) {
        self.databases.source.drop().await.unwrap();
    }
}

fn market_id(community_id: &str, slot: i64) -> String {
    bytes16_to_hex(generate_market_id(
        community_id,
        MarketType::Spot,
        slot as u64,
    ))
}

fn trade(
    uuid: &str,
    buyer_owner: &str,
    market_id: String,
    energy_kwh: f64,
    rate: f64,
) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: uuid.to_string(),
        status: TradeStatus::Settled,
        seller: onchain_id("owner-p"),
        buyer: onchain_id(buyer_owner),
        market_id,
        time_slot: SLOT as u64,
        creation_time: (SLOT - 3600) as u64,
        offer_hash: format!("{}-offer", uuid),
        bid_hash: format!("{}-bid", uuid),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: energy_kwh,
            energy_rate: rate,
        },
    }
}

fn measurement_point(facility_id: &str) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: format!("measurement:{}:{}", COMMUNITY, facility_id),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: FlowDirection::Import,
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: facility_id.to_string(),
        datasource_name: Some(COMMUNITY.to_string()),
    }
}

/// Seeds the worked example from the plan (§3.2) plus one trade on an unknown market.
async fn seed_worked_example(db: &Database) {
    db.collection::<EnergyCommunitySchema>("communities")
        .insert_one(EnergyCommunitySchema {
            community_id: COMMUNITY.to_string(),
            community_name: "Pilot 1".to_string(),
            sites: vec![],
        })
        .await
        .unwrap();

    let facilities = ["a", "b", "p"].map(|name| FacilitySchema {
        facility_id: format!("facility-{}", name),
        facility_name: format!("Facility {}", name),
        site_id: "site-1".to_string(),
        owner_id: format!("owner-{}", name),
    });
    db.collection::<FacilitySchema>("facilities")
        .insert_many(facilities)
        .await
        .unwrap();

    let readings = [
        ("facility-a", 4.0),
        ("facility-b", 2.0),
        ("facility-p", -5.0),
    ];
    db.collection::<MeasurementPointSchema>("measurement_points")
        .insert_many(readings.map(|(facility, _)| measurement_point(facility)))
        .await
        .unwrap();
    db.collection::<TimeseriesSchema>("timeseries")
        .insert_many(readings.map(|(facility, value)| TimeseriesSchema {
            measurement_point: format!("measurement:{}:{}", COMMUNITY, facility),
            timestamp: timestamp_to_string_with_padding(SLOT as u64),
            value,
        }))
        .await
        .unwrap();

    db.collection::<DbTradeSchema>("trades")
        .insert_many([
            trade("t1", "owner-a", market_id(COMMUNITY, SLOT), 3.0, 0.12),
            trade("t2", "owner-b", market_id(COMMUNITY, SLOT), 1.5, 0.15),
            trade("t3", "owner-a", market_id("Unknown", SLOT), 1.0, 0.10),
        ])
        .await
        .unwrap();
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < EPSILON,
        "expected {}, got {}",
        expected,
        actual
    );
}

#[tokio::test]
async fn tick_computes_the_worked_example_from_mongodb() {
    let test_db = TestDb::new().await;
    seed_worked_example(test_db.source()).await;
    let engine = test_db.engine();
    engine.ensure_indexes().await.unwrap();

    let stats = engine.run_tick(NOW).await.unwrap();
    assert_eq!(stats.window.end, SLOT + SLOT_LENGTH);
    assert_eq!(stats.load.communities, 1);
    assert_eq!(stats.load.trades, 3);
    assert_eq!(stats.load.unassigned_trades, 1);
    assert_eq!(stats.load.readings, 3);
    assert_eq!(stats.results_upserted, 1);

    let results = test_db.results().await;
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert_eq!(result.kpi_id, "procurement_cost_per_kwh");
    assert_eq!(result.community_id, COMMUNITY);
    assert_eq!(result.period_start, SLOT);
    assert_eq!(result.period_end, SLOT + SLOT_LENGTH);
    assert_eq!(result.computed_at, NOW);
    assert_close(result.value.unwrap(), 0.1725);
    assert_close(result.components.p2p_cost, 0.585);
    assert_close(result.components.net_demand_kwh, 6.0);
    assert_close(result.components.grid_residual_energy_kwh, 1.5);
    assert_eq!(result.components.trade_count, 2);
    assert_eq!(result.components.buyers_without_reading, 0);
    let baseline = result.baseline.as_ref().unwrap();
    assert_close(baseline.baseline_value.unwrap(), 0.30);
    assert_close(baseline.improvement_pct.unwrap(), 42.5);

    test_db.drop().await;
}

#[tokio::test]
async fn rerunning_a_tick_is_idempotent() {
    let test_db = TestDb::new().await;
    seed_worked_example(test_db.source()).await;
    let engine = test_db.engine();
    engine.ensure_indexes().await.unwrap();

    engine.run_tick(NOW).await.unwrap();
    engine.run_tick(NOW + 60).await.unwrap();

    let results = test_db.results().await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].computed_at, NOW + 60);
    assert_close(results[0].value.unwrap(), 0.1725);

    test_db.drop().await;
}

#[tokio::test]
async fn tick_without_data_writes_nothing() {
    let test_db = TestDb::new().await;
    let engine = test_db.engine();
    engine.ensure_indexes().await.unwrap();

    let stats = engine.run_tick(NOW).await.unwrap();
    assert_eq!(stats.results_upserted, 0);
    assert!(test_db.results().await.is_empty());

    test_db.drop().await;
}

#[tokio::test]
async fn backfill_covers_history_before_the_tick_window() {
    let test_db = TestDb::new().await;
    seed_worked_example(test_db.source()).await;
    let engine = test_db.engine();
    engine.ensure_indexes().await.unwrap();

    // Two days later the example slot is outside the 48 h lookback, so only backfill reaches it.
    let later = NOW + 3 * 24 * 3600;
    assert!(engine.tick_window(later).start > SLOT);
    let chunks = engine.run_backfill(SLOT - 3600, later).await.unwrap();
    assert!(chunks.len() >= 2);
    assert_eq!(
        chunks
            .iter()
            .map(|chunk| chunk.results_upserted)
            .sum::<usize>(),
        1
    );
    assert_eq!(test_db.results().await.len(), 1);

    test_db.drop().await;
}

#[tokio::test]
async fn indexes_are_created_idempotently() {
    let test_db = TestDb::new().await;
    let engine = test_db.engine();
    engine.ensure_indexes().await.unwrap();
    engine.ensure_indexes().await.unwrap();

    let names = test_db
        .databases
        .results
        .collection::<Document>(&test_db.config.results_collection)
        .list_index_names()
        .await
        .unwrap();
    assert!(names.contains(&RESULT_KEY_INDEX.to_string()));
    assert!(names.contains(&PERIOD_START_INDEX.to_string()));

    test_db.drop().await;
}

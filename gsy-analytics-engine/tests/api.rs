//! Runs the HTTP API against a real MongoDB, using the `DATABASE_*` env vars like the other
//! services' integration tests. Each test uses its own randomly named database and drops it.

use gsy_analytics_engine::api::run_http_server;
use gsy_analytics_engine::config::Config;
use gsy_analytics_engine::db::{connect, Databases};
use mongodb::bson::Document;
use primitives::db_api_schema::kpi::{
    KpiBaseline, MarketTimeSeriesGranularity, ProcurementCostComponents,
    ProcurementCostResultSchema, PROCUREMENT_COST_PER_KWH_KPI_ID,
};
use reqwest::StatusCode;
use std::net::TcpListener;
use std::time::Duration;
use uuid::Uuid;

const SLOT: i64 = 1_758_621_600; // 2025-09-23 10:00 UTC
const SLOT_LENGTH: i64 = 900;
const ENDPOINT: &str = "kpis/procurement-cost-per-kwh";

struct TestApi {
    address: String,
    databases: Databases,
    config: Config,
    client: reqwest::Client,
}

impl TestApi {
    async fn new() -> Self {
        let overridden = ["DATABASE_NAME", "ANALYTICS_RESULTS_DATABASE_NAME"];
        let mut vars: Vec<(String, String)> = std::env::vars()
            .filter(|(key, _)| !overridden.contains(&key.as_str()))
            .collect();
        vars.push(("DATABASE_NAME".to_string(), Uuid::new_v4().to_string()));
        let config = Config::from_vars(vars).unwrap();
        let databases = tokio::time::timeout(Duration::from_secs(60), connect(&config))
            .await
            .expect("Could not connect to MongoDB; check the DATABASE_* env vars");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let server = run_http_server(listener, Self::collection(&databases, &config)).unwrap();
        tokio::spawn(server);

        TestApi {
            address,
            databases,
            config,
            client: reqwest::Client::new(),
        }
    }

    fn collection(databases: &Databases, config: &Config) -> mongodb::Collection<Document> {
        databases
            .results
            .collection::<Document>(&config.results_collection)
    }

    async fn seed(&self, results: Vec<ProcurementCostResultSchema>) {
        Self::collection(&self.databases, &self.config)
            .clone_with_type::<ProcurementCostResultSchema>()
            .insert_many(results)
            .await
            .unwrap();
    }

    async fn get(&self, query: &str) -> reqwest::Response {
        self.client
            .get(format!("{}/{}{}", self.address, ENDPOINT, query))
            .send()
            .await
            .unwrap()
    }

    /// `(community_id, period_start)` of each returned result, in response order.
    async fn get_keys(&self, query: &str) -> Vec<(String, i64)> {
        let response = self.get(query).await;
        assert_eq!(response.status(), StatusCode::OK);
        response
            .json::<Vec<ProcurementCostResultSchema>>()
            .await
            .unwrap()
            .into_iter()
            .map(|result| (result.community_id, result.period_start))
            .collect()
    }

    async fn drop(self) {
        self.databases.source.drop().await.unwrap();
    }
}

fn result(community_id: &str, period_start: i64) -> ProcurementCostResultSchema {
    ProcurementCostResultSchema {
        kpi_id: PROCUREMENT_COST_PER_KWH_KPI_ID.to_string(),
        community_id: community_id.to_string(),
        granularity: MarketTimeSeriesGranularity::FifteenMinutes,
        period_start,
        period_end: period_start + SLOT_LENGTH,
        value: Some(0.1725),
        null_reason: None,
        unit: "EUR/kWh".to_string(),
        components: ProcurementCostComponents {
            p2p_cost: 0.585,
            net_demand_kwh: 6.0,
            ..Default::default()
        },
        baseline: Some(KpiBaseline {
            baseline_value: Some(0.30),
            improvement_pct: Some(42.5),
        }),
        computed_at: SLOT + 2 * SLOT_LENGTH,
        engine_version: "0.1.0".to_string(),
    }
}

/// Four slots for Pilot1 and one for Pilot2, inserted out of order.
fn seed_results() -> Vec<ProcurementCostResultSchema> {
    vec![
        result("Pilot2", SLOT),
        result("Pilot1", SLOT + 3 * SLOT_LENGTH),
        result("Pilot1", SLOT),
        result("Pilot1", SLOT + 2 * SLOT_LENGTH),
        result("Pilot1", SLOT + SLOT_LENGTH),
    ]
}

fn keys(items: &[(&str, i64)]) -> Vec<(String, i64)> {
    items
        .iter()
        .map(|(community_id, period_start)| (community_id.to_string(), *period_start))
        .collect()
}

#[tokio::test]
async fn health_check_returns_ok() {
    let api = TestApi::new().await;

    let response = api
        .client
        .get(format!("{}/health_check", api.address))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    api.drop().await;
}

#[tokio::test]
async fn returns_the_stored_result_document() {
    let api = TestApi::new().await;
    api.seed(vec![result("Pilot1", SLOT)]).await;

    let response = api.get("").await;
    assert_eq!(response.status(), StatusCode::OK);
    let results: Vec<ProcurementCostResultSchema> = response.json().await.unwrap();
    assert_eq!(results, vec![result("Pilot1", SLOT)]);

    api.drop().await;
}

#[tokio::test]
async fn without_parameters_returns_everything_ordered() {
    let api = TestApi::new().await;
    api.seed(seed_results()).await;

    assert_eq!(
        api.get_keys("").await,
        keys(&[
            ("Pilot1", SLOT),
            ("Pilot1", SLOT + SLOT_LENGTH),
            ("Pilot1", SLOT + 2 * SLOT_LENGTH),
            ("Pilot1", SLOT + 3 * SLOT_LENGTH),
            ("Pilot2", SLOT),
        ])
    );

    api.drop().await;
}

#[tokio::test]
async fn time_range_is_half_open_on_period_start() {
    let api = TestApi::new().await;
    api.seed(seed_results()).await;

    let query = format!(
        "?start_time={}&end_time={}",
        SLOT + SLOT_LENGTH,
        SLOT + 3 * SLOT_LENGTH
    );
    assert_eq!(
        api.get_keys(&query).await,
        keys(&[
            ("Pilot1", SLOT + SLOT_LENGTH),
            ("Pilot1", SLOT + 2 * SLOT_LENGTH),
        ])
    );

    api.drop().await;
}

#[tokio::test]
async fn a_single_bound_is_open_ended() {
    let api = TestApi::new().await;
    api.seed(seed_results()).await;

    assert_eq!(
        api.get_keys(&format!("?start_time={}", SLOT + 2 * SLOT_LENGTH))
            .await,
        keys(&[
            ("Pilot1", SLOT + 2 * SLOT_LENGTH),
            ("Pilot1", SLOT + 3 * SLOT_LENGTH),
        ])
    );
    assert_eq!(
        api.get_keys(&format!("?end_time={}", SLOT + SLOT_LENGTH))
            .await,
        keys(&[("Pilot1", SLOT), ("Pilot2", SLOT)])
    );

    api.drop().await;
}

#[tokio::test]
async fn community_filter_combines_with_the_time_range() {
    let api = TestApi::new().await;
    api.seed(seed_results()).await;

    assert_eq!(
        api.get_keys("?community_id=Pilot2").await,
        keys(&[("Pilot2", SLOT)])
    );
    assert_eq!(
        api.get_keys(&format!(
            "?community_id=Pilot1&start_time={}&end_time={}",
            SLOT,
            SLOT + SLOT_LENGTH
        ))
        .await,
        keys(&[("Pilot1", SLOT)])
    );
    assert!(api.get_keys("?community_id=Unknown").await.is_empty());

    api.drop().await;
}

#[tokio::test]
async fn invalid_parameters_are_rejected() {
    let api = TestApi::new().await;

    for query in [
        format!("?start_time={}&end_time={}", SLOT, SLOT),
        format!("?start_time={}&end_time={}", SLOT, SLOT - SLOT_LENGTH),
    ] {
        let response = api.get(&query).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{}", query);
        assert_eq!(
            response.text().await.unwrap(),
            "end_time must be after start_time"
        );
    }
    for query in [
        "?start_time=yesterday",
        "?end_time=-5",
        "?start_time=18446744073709551615",
    ] {
        assert_eq!(
            api.get(query).await.status(),
            StatusCode::BAD_REQUEST,
            "{}",
            query
        );
    }

    api.drop().await;
}

#[tokio::test]
async fn empty_collection_returns_an_empty_list() {
    let api = TestApi::new().await;

    assert!(api.get_keys("").await.is_empty());

    api.drop().await;
}

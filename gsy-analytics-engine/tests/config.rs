use gsy_analytics_engine::config::Config;
use primitives::MarketTimeSeriesGranularity;

fn config_from(vars: &[(&str, &str)]) -> anyhow::Result<Config> {
    Config::from_vars(
        vars.iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
}

#[test]
fn defaults_are_applied_when_no_env_is_set() {
    let config = config_from(&[]).unwrap();

    assert_eq!(config.database.url_scheme, "mongodb");
    assert_eq!(config.database.host, "mongodb");
    assert_eq!(config.database.username, "gsy");
    assert_eq!(config.database.password.0, "gsy");
    assert_eq!(config.database.name, "offchain_storage");
    assert_eq!(config.results_database_name, "offchain_storage");
    assert_eq!(config.results_collection, "kpi_results");
    assert_eq!(config.interval_seconds, 900);
    assert_eq!(config.lookback_hours, 48);
    assert_eq!(config.settlement_delay_minutes, 15);
    assert_eq!(config.backfill_from, None);
    assert_eq!(
        config.enabled_kpis,
        vec!["procurement_cost_per_kwh".to_string()]
    );
    assert_eq!(
        config.granularities,
        vec![MarketTimeSeriesGranularity::FifteenMinutes]
    );
    assert_eq!(config.tariffs.default_eur_per_kwh, None);
    assert!(config.tariffs.overrides.is_empty());
    assert!(config.tariffs.is_empty());
}

#[test]
fn env_values_override_defaults() {
    let config = config_from(&[
        ("DATABASE_HOST", "localhost:27017"),
        ("DATABASE_NAME", "custom_db"),
        ("ANALYTICS_INTERVAL_SECONDS", "60"),
        ("ANALYTICS_LOOKBACK_HOURS", "6"),
        (
            "ANALYTICS_ENABLED_KPIS",
            " procurement_cost_per_kwh , other_kpi ,",
        ),
    ])
    .unwrap();

    assert_eq!(config.database.host, "localhost:27017");
    assert_eq!(config.database.name, "custom_db");
    assert_eq!(config.interval_seconds, 60);
    assert_eq!(config.lookback_hours, 6);
    assert_eq!(
        config.enabled_kpis,
        vec!["procurement_cost_per_kwh", "other_kpi"]
    );
}

#[test]
fn results_database_falls_back_to_source_database() {
    let config = config_from(&[("DATABASE_NAME", "source_db")]).unwrap();
    assert_eq!(config.results_database_name, "source_db");

    let config = config_from(&[
        ("DATABASE_NAME", "source_db"),
        ("ANALYTICS_RESULTS_DATABASE_NAME", ""),
    ])
    .unwrap();
    assert_eq!(config.results_database_name, "source_db");

    let config = config_from(&[
        ("DATABASE_NAME", "source_db"),
        ("ANALYTICS_RESULTS_DATABASE_NAME", "analytics"),
    ])
    .unwrap();
    assert_eq!(config.results_database_name, "analytics");
}

#[test]
fn connection_string_matches_offchain_storage_format() {
    let config = config_from(&[
        ("DATABASE_URL_SCHEME", "mongodb+srv"),
        ("DATABASE_HOST", "cluster.example.com"),
        ("DATABASE_USERNAME", "user"),
        ("DATABASE_PASSWORD", "secret"),
    ])
    .unwrap();

    assert_eq!(
        config.database.connection_string(),
        "mongodb+srv://user:secret@cluster.example.com/?retryWrites=true&w=majority"
    );
}

#[test]
fn password_is_redacted_in_debug_output() {
    let config = config_from(&[("DATABASE_PASSWORD", "super-secret")]).unwrap();
    let debug = format!("{:?}", config);

    assert!(!debug.contains("super-secret"));
    assert!(debug.contains("***"));
}

#[test]
fn only_fifteen_minute_granularity_is_supported() {
    for value in ["1h", "15min,1d"] {
        let error = config_from(&[("ANALYTICS_GRANULARITIES", value)]).unwrap_err();
        assert!(error.to_string().contains("not supported yet"), "{}", value);
    }

    let error = config_from(&[("ANALYTICS_GRANULARITIES", "5min")]).unwrap_err();
    assert!(error.to_string().contains("Unknown granularity"));

    assert!(config_from(&[("ANALYTICS_GRANULARITIES", " , ")]).is_err());
}

#[test]
fn tariffs_are_parsed() {
    let config = config_from(&[
        ("ANALYTICS_GRID_TARIFF_EUR_PER_KWH", "0.30"),
        (
            "ANALYTICS_GRID_TARIFF_OVERRIDES",
            "Pilot1=0.28, Pilot2 = 0.25",
        ),
    ])
    .unwrap();

    assert_eq!(config.tariffs.default_eur_per_kwh, Some(0.30));
    assert_eq!(config.tariffs.overrides.len(), 2);
    assert_eq!(config.tariffs.overrides["Pilot1"], 0.28);
    assert_eq!(config.tariffs.overrides["Pilot2"], 0.25);
    assert!(!config.tariffs.is_empty());
}

#[test]
fn empty_tariff_values_mean_unset() {
    let config = config_from(&[
        ("ANALYTICS_GRID_TARIFF_EUR_PER_KWH", ""),
        ("ANALYTICS_GRID_TARIFF_OVERRIDES", "  "),
    ])
    .unwrap();

    assert!(config.tariffs.is_empty());
}

#[test]
fn malformed_tariffs_are_rejected() {
    for value in ["abc", "-0.1", "NaN"] {
        assert!(
            config_from(&[("ANALYTICS_GRID_TARIFF_EUR_PER_KWH", value)]).is_err(),
            "{}",
            value
        );
    }
    for value in ["Pilot1", "Pilot1=abc", "=0.2", "Pilot1=-1"] {
        assert!(
            config_from(&[("ANALYTICS_GRID_TARIFF_OVERRIDES", value)]).is_err(),
            "{}",
            value
        );
    }
}

#[test]
fn backfill_accepts_unix_seconds_and_rfc3339() {
    let config = config_from(&[("ANALYTICS_BACKFILL_FROM", "1758621600")]).unwrap();
    assert_eq!(config.backfill_from, Some(1758621600));

    let config = config_from(&[("ANALYTICS_BACKFILL_FROM", "2025-09-23T10:00:00Z")]).unwrap();
    assert_eq!(config.backfill_from, Some(1758621600));

    assert!(config_from(&[("ANALYTICS_BACKFILL_FROM", "yesterday")]).is_err());
}

#[test]
fn zero_interval_is_rejected() {
    assert!(config_from(&[("ANALYTICS_INTERVAL_SECONDS", "0")]).is_err());
}

#[test]
fn api_address_defaults_and_overrides() {
    let config = config_from(&[]).unwrap();
    assert_eq!(config.api_host, "0.0.0.0");
    assert_eq!(config.api_port, 8081);
    assert_eq!(config.api_address(), "0.0.0.0:8081");

    let config = config_from(&[
        ("ANALYTICS_API_HOST", "127.0.0.1"),
        ("ANALYTICS_API_PORT", "9000"),
    ])
    .unwrap();
    assert_eq!(config.api_address(), "127.0.0.1:9000");

    assert!(config_from(&[("ANALYTICS_API_PORT", "70000")]).is_err());
    assert!(config_from(&[("ANALYTICS_API_PORT", "http")]).is_err());
}

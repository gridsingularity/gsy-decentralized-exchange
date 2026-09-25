use gsy_analytics_engine::config::{Config, TariffConfig};
use gsy_analytics_engine::kpi::{build_registry, combined_requirements, DataRequirements};
use gsy_analytics_engine::tariff::TariffProvider;
use std::collections::HashMap;

fn config_with_kpis(kpis: &str) -> Config {
    Config::from_vars([("ANALYTICS_ENABLED_KPIS".to_string(), kpis.to_string())]).unwrap()
}

#[test]
fn default_config_enables_procurement_cost_per_kwh() {
    let config = Config::from_vars(Vec::new()).unwrap();
    let kpis = build_registry(&config).unwrap();

    assert_eq!(kpis.len(), 1);
    assert_eq!(kpis[0].id(), "procurement_cost_per_kwh");
    assert_eq!(kpis[0].unit(), "EUR/kWh");
}

#[test]
fn unknown_kpi_is_rejected() {
    let error = build_registry(&config_with_kpis("procurement_cost_per_kwh,other_kpi"))
        .err()
        .unwrap();
    assert!(error.to_string().contains("Unknown KPI 'other_kpi'"));
}

#[test]
fn duplicate_kpi_is_rejected() {
    let error = build_registry(&config_with_kpis(
        "procurement_cost_per_kwh,procurement_cost_per_kwh",
    ))
    .err()
    .unwrap();
    assert!(error.to_string().contains("more than once"));
}

#[test]
fn empty_kpi_list_is_rejected() {
    assert!(build_registry(&config_with_kpis(" , ")).is_err());
}

#[test]
fn requirements_are_combined_across_kpis() {
    let kpis = build_registry(&Config::from_vars(Vec::new()).unwrap()).unwrap();
    assert_eq!(
        combined_requirements(&kpis),
        DataRequirements {
            communities: true,
            trades: true,
            readings: true,
        }
    );

    let trades_only = DataRequirements {
        trades: true,
        ..Default::default()
    };
    let readings_only = DataRequirements {
        readings: true,
        ..Default::default()
    };
    assert_eq!(
        trades_only | readings_only,
        DataRequirements {
            communities: false,
            trades: true,
            readings: true,
        }
    );
}

#[test]
fn tariff_override_wins_over_the_default() {
    let tariffs = TariffConfig {
        default_eur_per_kwh: Some(0.30),
        overrides: HashMap::from([("Pilot1".to_string(), 0.25)]),
    };

    assert_eq!(tariffs.tariff_for("Pilot1", 0), Some(0.25));
    assert_eq!(tariffs.tariff_for("Pilot2", 0), Some(0.30));

    let overrides_only = TariffConfig {
        default_eur_per_kwh: None,
        ..tariffs
    };
    assert_eq!(overrides_only.tariff_for("Pilot2", 0), None);
}

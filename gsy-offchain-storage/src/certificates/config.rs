//! Pilot-instance parameters of Annex A.1 (`openapi.yaml` component descriptions), one
//! `GOO_*` env var per field. Defaults are the pilot values; see the plan for provenance.

use std::sync::LazyLock;

use gsy_offchain_primitives::utils::read_env_or;

pub struct PilotConfig {
    pub site_id: String,
    pub interval_duration_s: u64,
    pub rounding_rule: String,
    pub municipality_code: String,
    pub grid_operator_id: String,
    pub grid_level: u32,
    pub data_provider_id: String,
    pub property_measured: String,
}

impl PilotConfig {
    fn from_env() -> Self {
        Self {
            site_id: read_env_or("GOO_SITE_ID", "ch-aem-lic-goo-poc".to_string()),
            interval_duration_s: read_env_or("GOO_INTERVAL_DURATION_S", 900u64),
            rounding_rule: read_env_or("GOO_ROUNDING_RULE", "half_up_2dp".to_string()),
            municipality_code: read_env_or("GOO_MUNICIPALITY_CODE", "5226".to_string()),
            grid_operator_id: read_env_or("GOO_GRID_OPERATOR_ID", "AEM".to_string()),
            grid_level: read_env_or("GOO_GRID_LEVEL", 7u32),
            data_provider_id: read_env_or(
                "GOO_DATA_PROVIDER_ID",
                "did:example:aem-metering".to_string(),
            ),
            property_measured: read_env_or(
                "GOO_PROPERTY_MEASURED",
                "measurement#active_energy".to_string(),
            ),
        }
    }
}

pub static PILOT: LazyLock<PilotConfig> = LazyLock::new(PilotConfig::from_env);

#![allow(non_snake_case,non_upper_case_globals)]

use std::env;
use gsy_offchain_primitives::utils::read_env_or;
use once_cell::sync::Lazy;

pub struct Constants {
    pub FEDECOM_ONTOLOGY_URL: String,
    pub FEDECOM_ONTOLOGY_ASSETS_URL: String,
    pub FEDECOM_INFLUX_DB_URL: String,
    pub FEDECOM_INFLUX_DB_ORG: String,
    pub FEDECOM_INFLUX_DB_TOKEN: String,
}

impl Constants {
    fn new() -> Self {
        Self {
            FEDECOM_ONTOLOGY_URL: read_env_or(
                "FEDECOM_ONTOLOGY_URL",
                "https://fedecom.tekniker.es/services/queries/get_lecs_buildings".to_string()),
            FEDECOM_ONTOLOGY_ASSETS_URL: read_env_or(
                "FEDECOM_ONTOLOGY_ASSETS_URL",
                "https://fedecom.tekniker.es/services/queries/get_assets".to_string()),
            FEDECOM_INFLUX_DB_URL: read_env_or(
                "FEDECOM_INFLUX_DB_URL",
                "https://fedecom.imp.bg.ac.rs/influxdb/api/v2/query".to_string()),
            FEDECOM_INFLUX_DB_ORG: read_env_or(
                "FEDECOM_INFLUX_DB_ORG",
                "fedecom".to_string()),
            // Token is mandatory
            FEDECOM_INFLUX_DB_TOKEN: env::var("FEDECOM_INFLUX_DB_TOKEN".to_string()).unwrap()
        }
    }
}

pub static CommunityClientConstants: Lazy<Constants> = Lazy::new(Constants::new);

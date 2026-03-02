use gsy_offchain_primitives::{constants::GLOBAL_CONSTANTS, MarketType};
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(default = "default_node_url")]
    pub gsy_node_url: String,
    #[serde(default = "default_signer_suri")]
    pub orchestrator_signer_suri: String,
    #[serde(default = "default_tick_interval")]
    pub tick_interval_seconds: u64,
    #[serde(default = "default_look_ahead")]
    pub look_ahead_hours: u64,
}

fn default_node_url() -> String {
    "ws://gsy-node:9944".to_string()
}
fn default_signer_suri() -> String {
    "//Alice".to_string()
}
fn default_tick_interval() -> u64 {
    60
} // 1 minute
fn default_look_ahead() -> u64 {
    24
} // 24 hours

pub fn get_config() -> anyhow::Result<Config> {
    Ok(envy::from_env::<Config>()?)
}

pub struct MarketRule {
    pub market_type: MarketType,
    pub open_offset_mins: i64,
    pub close_offset_mins: i64,
}

pub static MARKET_RULES: Lazy<Vec<MarketRule>> = Lazy::new(|| {
    vec![
        MarketRule {
            market_type: MarketType::Spot,
            open_offset_mins: GLOBAL_CONSTANTS.spot_market_open_offset_min,
            close_offset_mins: GLOBAL_CONSTANTS.spot_market_close_offset_min,
        },
        MarketRule {
            market_type: MarketType::Flexibility,
            open_offset_mins: GLOBAL_CONSTANTS.flex_market_open_offset_min,
            close_offset_mins: GLOBAL_CONSTANTS.flex_market_close_offset_min,
        },
        MarketRule {
            market_type: MarketType::Settlement,
            open_offset_mins: GLOBAL_CONSTANTS.settlement_market_open_offset_min,
            close_offset_mins: GLOBAL_CONSTANTS.settlement_market_close_offset_min,
        },
    ]
});

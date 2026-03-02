use config::{Config, ConfigError, File};
use ethers::types::Address;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Settings {
    pub database_host: String,
    pub database_username: String,
    pub database_password: String,
    pub database_name: String,
    pub database_url_scheme: String,
    pub application_host: String,
    pub application_port: u16,
    pub scheduler_interval: u32,

    #[serde(default = "default_evm_url")]
    pub evm_node_url: String,
    #[serde(default = "default_address")]
    pub contract_order_registry: Address,
    #[serde(default = "default_address")]
    pub contract_trade_settlement: Address,
    #[serde(default = "default_address")]
    pub contract_market_controller: Address,
}

fn default_evm_url() -> String {
    "ws://localhost:8545".to_string()
}

fn default_address() -> Address {
    Address::zero()
}

impl Settings {
    pub fn get_connection_string(&self) -> String {
        format!(
            "{}://{}:{}@{}/?retryWrites=true&w=majority",
            self.database_url_scheme,
            self.database_username,
            self.database_password,
            self.database_host
        )
    }
    pub fn get_scheduler_interval(&self) -> u32 {
        self.scheduler_interval
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    match envy::from_env::<Settings>() {
        Ok(settings) => Ok(settings),
        Err(_) => Config::builder()
            .add_source(File::with_name("configuration.yaml"))
            .build()?
            .try_deserialize(),
    }
}

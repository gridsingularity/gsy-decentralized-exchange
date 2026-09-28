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
	#[serde(default = "default_offchain_storage_url")]
	pub offchain_storage_url: String,
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
fn default_offchain_storage_url() -> String {
	"http://gsy-offchain-storage:8080".to_string()
}

pub fn get_config() -> anyhow::Result<Config> {
	Ok(envy::from_env::<Config>()?)
}

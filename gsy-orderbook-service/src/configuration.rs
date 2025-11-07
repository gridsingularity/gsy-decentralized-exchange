use config::{Config, ConfigError, File};

#[derive(serde::Deserialize)]
pub struct Settings {
	pub database_host: String,
	pub database_username: String,
	pub database_password: String,
	pub database_name: String,
	pub database_url_scheme: String,
	pub application_host: String,
	pub application_port: u16,
	pub node_url: String,
	pub scheduler_interval: u32,
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
	pub fn get_node_url(&self) -> String {
		self.node_url.clone()
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

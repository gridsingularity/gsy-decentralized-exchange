use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub database_host: String,
    pub database_username: String,
    pub database_password: String,
    pub database_name: String,
    pub database_url_scheme: String,
    pub application_host: String,
    pub application_port: u16,
    pub jwt_secret: String,
}

impl Configuration {
    pub fn get_connection_string(&self) -> String {
        format!(
            "{}://{}:{}@{}/?retryWrites=true&w=majority",
            self.database_url_scheme,
            self.database_username,
            self.database_password,
            self.database_host
        )
    }
}

pub fn get_configuration() -> Result<Configuration, ConfigError> {
    Config::builder()
        .add_source(File::with_name("configuration.yaml").required(false))
        .add_source(config::Environment::default())
        .build()?
        .try_deserialize()
}

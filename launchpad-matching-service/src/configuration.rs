use config::{Config, ConfigError, File};
use serde::Deserialize;

/// Default maximum size of a JSON request body: 16 MiB. It must exceed the
/// largest plan's request limit (10 MB, Advanced) because the launchpad
/// backend adds a `user_id` to every order before forwarding a batch.
pub const DEFAULT_JSON_BODY_LIMIT: usize = 16 * 1024 * 1024;

fn default_json_body_limit() -> usize {
    DEFAULT_JSON_BODY_LIMIT
}

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
    /// Maximum size in bytes of a JSON request body (`POST /match`,
    /// `POST /token`); larger bodies get `413 payload_too_large`.
    #[serde(default = "default_json_body_limit")]
    pub json_body_limit: usize,
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

    /// Checks the values serde cannot check by itself.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.json_body_limit == 0 {
            return Err(ConfigError::Message(
                "json_body_limit must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Builds and validates the configuration from `builder`'s sources.
pub fn configuration_from(
    builder: config::ConfigBuilder<config::builder::DefaultState>,
) -> Result<Configuration, ConfigError> {
    let configuration: Configuration = builder.build()?.try_deserialize()?;
    configuration.validate()?;
    Ok(configuration)
}

/// Loads `configuration.yaml` (optional), overridden by environment
/// variables (`DATABASE_HOST`, `JSON_BODY_LIMIT`, ...).
pub fn get_configuration() -> Result<Configuration, ConfigError> {
    configuration_from(
        Config::builder()
            .add_source(File::with_name("configuration.yaml").required(false))
            .add_source(config::Environment::default()),
    )
}

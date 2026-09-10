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
    #[serde(default)]
    pub database_replica_set: Option<String>,
    #[serde(default)]
    pub database_options: Option<String>,
    #[serde(default)]
    pub database_auth_source: Option<String>,
    #[serde(default)]
    pub database_tls: Option<bool>,
    #[serde(default = "default_api_key")]
    pub api_key: String,
}

fn default_api_key() -> String {
    "fedecom_user".to_string()
}

impl Settings {
    pub fn get_connection_string(&self) -> String {
        let mut uri = format!(
            "{}://{}:{}@{}/?retryWrites=true&w=majority",
            self.database_url_scheme,
            self.database_username,
            self.database_password,
            self.database_host
        );

        if let Some(replica_set) = self.database_replica_set.as_deref() {
            if !replica_set.is_empty() {
                uri.push_str("&replicaSet=");
                uri.push_str(replica_set);
            }
        }

        if let Some(auth_source) = self.database_auth_source.as_deref() {
            if !auth_source.is_empty() {
                uri.push_str("&authSource=");
                uri.push_str(auth_source);
            }
        }

        if let Some(tls) = self.database_tls {
            uri.push_str(if tls { "&tls=true" } else { "&tls=false" });
        }

        if let Some(options) = self.database_options.as_deref() {
            let trimmed = options.trim_start_matches(|c| c == '&' || c == '?');
            if !trimmed.is_empty() {
                uri.push('&');
                uri.push_str(trimmed);
            }
        }

        uri
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

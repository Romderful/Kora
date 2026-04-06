//! Application configuration loaded via figment.

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

/// Top-level configuration for the Kora server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KoraConfig {
    /// `PostgreSQL` connection string.
    pub database_url: String,
    /// Host address to bind the server to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Minimum log level.
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Maximum request body size in bytes.
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
}

fn default_host() -> String {
    "0.0.0.0".to_owned()
}

fn default_port() -> u16 {
    8080
}

fn default_log_level() -> String {
    "info".to_owned()
}

fn default_max_body_size() -> usize {
    16 * 1_024 * 1_024
}

impl Default for KoraConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
            max_body_size: default_max_body_size(),
        }
    }
}

impl KoraConfig {
    /// Load configuration from defaults, optional `kora.toml`, and environment variables.
    ///
    /// Layer order (last wins): defaults → `kora.toml` → env vars.
    /// Environment variables use the `KORA_` prefix (e.g. `KORA_PORT=9090`).
    /// `DATABASE_URL` is also accepted without prefix for convenience.
    ///
    /// # Errors
    ///
    /// Returns an error if required values are missing or cannot be parsed.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file("kora.toml"))
            .merge(Env::prefixed("KORA_"))
            .merge(Env::raw().only(&["DATABASE_URL"]))
            .extract()
            .map_err(Box::new)
    }
}

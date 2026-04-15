//! Application configuration loaded via figment.

use figment::{
    Figment,
    providers::{Env, Serialized},
};
use serde::{Deserialize, Serialize};

// -- Types --

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
    /// Maximum request body size in bytes.
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
}

// -- Impls --

impl Default for KoraConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            host: default_host(),
            port: default_port(),
            max_body_size: default_max_body_size(),
        }
    }
}

impl KoraConfig {
    /// Load configuration from defaults and environment variables.
    ///
    /// Layer order (last wins): defaults → env vars.
    /// Uses direct variable names: `DATABASE_URL`, `HOST`, `PORT`,
    /// `MAX_BODY_SIZE`.
    ///
    /// # Errors
    ///
    /// Returns an error if required values are missing or cannot be parsed.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::raw().only(&["DATABASE_URL", "HOST", "PORT", "MAX_BODY_SIZE"]))
            .extract()
            .map_err(Box::new)
    }
}

// -- Helpers --

fn default_host() -> String {
    "0.0.0.0".to_owned()
}

fn default_port() -> u16 {
    8080
}

fn default_max_body_size() -> usize {
    16 * 1_024 * 1_024
}

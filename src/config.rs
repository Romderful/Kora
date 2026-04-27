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
    /// `PostgreSQL` connection string. If empty, composed at startup from
    /// `DB_HOST` / `DB_PORT` / `DB_USER` / `DB_PASSWORD` / `DB_NAME`.
    #[serde(default)]
    pub database_url: String,
    /// `PostgreSQL` host (used when `database_url` is empty).
    #[serde(default)]
    pub db_host: String,
    /// `PostgreSQL` port.
    #[serde(default = "default_db_port")]
    pub db_port: u16,
    /// `PostgreSQL` user.
    #[serde(default)]
    pub db_user: String,
    /// `PostgreSQL` password.
    #[serde(default)]
    pub db_password: String,
    /// `PostgreSQL` database name.
    #[serde(default)]
    pub db_name: String,
    /// Host address to bind the server to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Maximum request body size in bytes.
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
    /// Maximum number of database connections in the pool.
    #[serde(default = "default_db_pool_max")]
    pub db_pool_max: u32,
}

// -- Impls --

impl Default for KoraConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            db_host: String::new(),
            db_port: default_db_port(),
            db_user: String::new(),
            db_password: String::new(),
            db_name: String::new(),
            host: default_host(),
            port: default_port(),
            max_body_size: default_max_body_size(),
            db_pool_max: default_db_pool_max(),
        }
    }
}

impl KoraConfig {
    /// Load configuration from defaults and environment variables.
    ///
    /// Recognized env vars: `DATABASE_URL`, `DB_HOST`, `DB_PORT`, `DB_USER`,
    /// `DB_PASSWORD`, `DB_NAME`, `HOST`, `PORT`, `MAX_BODY_SIZE`, `DB_POOL_MAX`.
    ///
    /// When `DATABASE_URL` is empty, it is composed from the `DB_*` components
    /// (with the user/password percent-encoded).
    ///
    /// # Errors
    ///
    /// Returns an error if values cannot be parsed, or if neither `DATABASE_URL`
    /// nor a complete `DB_*` set (`DB_HOST`, `DB_USER`, `DB_NAME`) is provided.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let mut cfg: Self = Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::raw().only(&[
                "DATABASE_URL",
                "DB_HOST",
                "DB_PORT",
                "DB_USER",
                "DB_PASSWORD",
                "DB_NAME",
                "HOST",
                "PORT",
                "MAX_BODY_SIZE",
                "DB_POOL_MAX",
            ]))
            .extract()
            .map_err(Box::new)?;

        if cfg.database_url.is_empty() {
            let mut missing = Vec::new();
            if cfg.db_host.is_empty() {
                missing.push("DB_HOST");
            }
            if cfg.db_user.is_empty() {
                missing.push("DB_USER");
            }
            if cfg.db_name.is_empty() {
                missing.push("DB_NAME");
            }
            if !missing.is_empty() {
                return Err(Box::new(figment::Error::from(format!(
                    "DATABASE_URL is unset; cannot compose from components — missing: {}",
                    missing.join(", ")
                ))));
            }
            cfg.database_url = format!(
                "postgres://{}:{}@{}:{}/{}",
                urlencoding::encode(&cfg.db_user),
                urlencoding::encode(&cfg.db_password),
                cfg.db_host,
                cfg.db_port,
                cfg.db_name,
            );
        }

        Ok(cfg)
    }
}

// -- Helpers --

fn default_host() -> String {
    "0.0.0.0".to_owned()
}

fn default_port() -> u16 {
    8080
}

fn default_db_port() -> u16 {
    5432
}

fn default_max_body_size() -> usize {
    16 * 1_024 * 1_024
}

fn default_db_pool_max() -> u32 {
    20
}

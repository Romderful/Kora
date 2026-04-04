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

impl Default for KoraConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_applied() {
        // Use figment with only defaults — no file, no env.
        let cfg: KoraConfig = Figment::from(Serialized::defaults(KoraConfig::default()))
            .extract()
            .expect("defaults should parse");

        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.database_url.is_empty());
    }

    #[test]
    fn env_overrides_defaults() {
        // Simulate env vars via figment data.
        let cfg: KoraConfig = Figment::from(Serialized::defaults(KoraConfig::default()))
            .merge(("port", 9090_u16))
            .merge(("host", "127.0.0.1"))
            .merge(("database_url", "postgres://test:test@localhost/test"))
            .extract()
            .expect("overrides should parse");

        assert_eq!(cfg.port, 9090);
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.database_url, "postgres://test:test@localhost/test");
    }

    #[test]
    fn database_url_required_for_startup() {
        // An empty database_url is technically valid at config-parse time,
        // but the caller is responsible for rejecting it before connecting.
        let cfg: KoraConfig = Figment::from(Serialized::defaults(KoraConfig::default()))
            .extract()
            .expect("defaults should parse");

        assert!(cfg.database_url.is_empty(), "default database_url should be empty");
    }
}

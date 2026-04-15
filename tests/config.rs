//! Tests for application configuration.

use figment::{Figment, providers::Serialized};
use kora::config::KoraConfig;

#[test]
fn config_defaults_applied() {
    let cfg: KoraConfig = Figment::from(Serialized::defaults(KoraConfig::default()))
        .extract()
        .expect("defaults should parse");

    assert_eq!(cfg.host, "0.0.0.0");
    assert_eq!(cfg.port, 8080);
    assert!(cfg.database_url.is_empty());
}

#[test]
fn config_env_overrides_defaults() {
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
fn config_database_url_required() {
    let cfg: KoraConfig = Figment::from(Serialized::defaults(KoraConfig::default()))
        .extract()
        .expect("defaults should parse");

    assert!(
        cfg.database_url.is_empty(),
        "default database_url should be empty"
    );
}

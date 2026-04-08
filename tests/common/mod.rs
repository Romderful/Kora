//! Shared test helpers for integration tests.
// Each test file compiles this module independently, so unused helpers
// produce false-positive warnings.
#![allow(dead_code)]

pub mod api;

use tokio::net::TcpListener;

// -- Constants --

/// Pass to list helpers to include soft-deleted items.
pub const INCLUDE_DELETED: bool = true;

/// Pass to list helpers to show only active items.
pub const ACTIVE_ONLY: bool = false;

// -- Fixtures --

/// A simple valid Avro schema with one field.
pub const AVRO_SCHEMA_V1: &str =
    r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"}]}"#;

/// A valid Avro schema with two fields (different from V1 to create a new version).
pub const AVRO_SCHEMA_V2: &str =
    r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"},{"name":"name","type":"string"}]}"#;

/// A valid Avro schema with three fields.
pub const AVRO_SCHEMA_V3: &str =
    r#"{"type":"record","name":"Test","fields":[{"name":"id","type":"int"},{"name":"name","type":"string"},{"name":"active","type":"boolean"}]}"#;

/// A valid Avro schema with a different record name (for check-schema tests).
pub const AVRO_SCHEMA_OTHER: &str =
    r#"{"type":"record","name":"Other","fields":[{"name":"x","type":"string"}]}"#;

// -- Setup --

/// Get `DATABASE_URL` from env. Panics if not set — use `just test` to run.
pub fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set — run via `just test`")
}

/// Create a PG pool with migrations applied.
pub async fn pool() -> sqlx::PgPool {
    kora::storage::create_pool(&database_url())
        .await
        .expect("database should be reachable")
}

/// Spawn the Kora server on a random port and return the base URL.
pub async fn spawn_server() -> String {
    let pool = pool().await;
    let config = kora::config::KoraConfig::default();
    let app = kora::api::router(pool, config.max_body_size);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("should bind to random port");
    let addr = listener.local_addr().expect("should have local addr");
    let base = format!("http://127.0.0.1:{}", addr.port());

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("server should run");
    });

    base
}

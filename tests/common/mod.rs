//! Shared test helpers for integration tests.

#![allow(dead_code)]

use tokio::net::TcpListener;

/// Get `DATABASE_URL` from env. Panics if not set — use `make test` to run.
pub fn database_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set — run via `make test`")
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

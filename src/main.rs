//! Kora — A lightweight, high-performance Schema Registry.

use kora::{api, config::KoraConfig, storage};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Structured JSON logging.
    tracing_subscriber::fmt().json().init();

    let cfg = KoraConfig::load().expect("failed to load configuration");

    assert!(!cfg.database_url.is_empty(), "DATABASE_URL is required");

    tracing::info!(host = %cfg.host, port = %cfg.port, "starting Kora");

    let pool = storage::create_pool(&cfg.database_url)
        .await
        .expect("failed to connect to database");

    let app = api::router(pool);
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");

    tracing::info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}

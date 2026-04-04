//! API route construction.

pub mod health;
mod middleware;

use axum::{Router, routing::get};
use sqlx::PgPool;

/// Build the application router with all routes.
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .layer(middleware::content_type_layer())
        .with_state(pool)
}

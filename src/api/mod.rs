//! API route construction.

pub mod health;
mod middleware;
pub mod subjects;

use axum::{Router, extract::DefaultBodyLimit, routing::{get, post}};
use sqlx::PgPool;

/// Build the application router with all routes.
pub fn router(pool: PgPool, max_body_size: usize) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route(
            "/subjects/{subject}/versions",
            post(subjects::register_schema),
        )
        .layer(DefaultBodyLimit::max(max_body_size))
        .layer(middleware::content_type_layer())
        .with_state(pool)
}

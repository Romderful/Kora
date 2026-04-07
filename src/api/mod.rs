//! API route construction.

pub mod health;
mod middleware;
pub mod schemas;
pub mod subjects;

use axum::{Router, extract::DefaultBodyLimit, routing::get};
use sqlx::PgPool;

/// Build the application router with all routes.
pub fn router(pool: PgPool, max_body_size: usize) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/schemas/ids/{id}", get(schemas::get_schema_by_id))
        .route("/subjects", get(subjects::list_subjects))
        .route(
            "/subjects/{subject}/versions",
            get(subjects::list_versions).post(subjects::register_schema),
        )
        .route(
            "/subjects/{subject}/versions/{version}",
            get(subjects::get_schema_by_version),
        )
        .layer(DefaultBodyLimit::max(max_body_size))
        .layer(middleware::content_type_layer())
        .with_state(pool)
}

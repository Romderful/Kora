//! API route construction.

pub mod compatibility;
pub mod health;
mod middleware;
pub mod schemas;
pub mod subjects;

use axum::{Router, extract::DefaultBodyLimit, routing::{get, post}};
use sqlx::PgPool;

// -- Router --

/// Build the application router with all routes.
pub fn router(pool: PgPool, max_body_size: usize) -> Router {
    Router::new()
        .route("/health", get(health::check_health))
        .route("/schemas/ids/{id}", get(schemas::get_schema_by_id))
        .route("/schemas/ids/{id}/subjects", get(schemas::get_subjects_by_schema_id))
        .route("/schemas/ids/{id}/versions", get(schemas::get_versions_by_schema_id))
        .route("/schemas/types", get(schemas::list_schema_types))
        .route("/subjects", get(subjects::list_subjects))
        .route("/subjects/{subject}", post(subjects::check_schema).delete(subjects::delete_subject))
        .route(
            "/subjects/{subject}/versions",
            get(subjects::list_versions).post(subjects::register_schema),
        )
        .route(
            "/subjects/{subject}/versions/{version}",
            get(subjects::get_schema_by_version).delete(subjects::delete_version),
        )
        .route(
            "/config",
            get(compatibility::get_global_compatibility).put(compatibility::set_global_compatibility),
        )
        .route(
            "/config/{subject}",
            get(compatibility::get_subject_compatibility)
                .put(compatibility::set_subject_compatibility)
                .delete(compatibility::delete_subject_compatibility),
        )
        .layer(DefaultBodyLimit::max(max_body_size))
        .layer(middleware::content_type_layer())
        .with_state(pool)
}

//! Health check endpoint.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use sqlx::PgPool;

/// Health check response body.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

/// `GET /health` — returns 200 when PG is reachable, 503 otherwise.
pub async fn health(State(pool): State<PgPool>) -> Response {
    let ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&pool)
        .await
        .is_ok();

    let (status_code, body) = if ok {
        (StatusCode::OK, HealthResponse { status: "UP" })
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            HealthResponse { status: "DOWN" },
        )
    };

    (status_code, Json(body)).into_response()
}

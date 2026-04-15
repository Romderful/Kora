//! Prometheus metrics endpoint.

use std::time::Duration;

use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;

// -- Handlers --

/// `GET /metrics` — Prometheus text exposition format.
///
/// Snapshots business and pool gauges at scrape time so values are
/// always fresh when the Prometheus scraper calls.
pub async fn get_metrics(
    State(pool): State<PgPool>,
    State(handle): State<PrometheusHandle>,
) -> Response {
    // Snapshot business metrics.  On DB failure or slow query the gauge
    // keeps its previous value rather than blocking the entire scrape.
    let query =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM schema_contents").fetch_one(&pool);
    if let Ok(Ok(n)) = tokio::time::timeout(Duration::from_secs(2), query).await {
        #[allow(clippy::cast_precision_loss)]
        metrics::gauge!("kora_schema_count").set(n as f64);
    }

    // Snapshot connection pool metrics.
    // num_idle() and size() are non-atomic — clamp to avoid a negative gauge.
    #[allow(clippy::cast_precision_loss)]
    let idle = pool.num_idle() as f64;
    let size = f64::from(pool.size());
    metrics::gauge!("kora_db_connections_in_use").set((size - idle).max(0.0));
    metrics::gauge!("kora_db_connections_idle").set(idle);

    let body = handle.render();
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}

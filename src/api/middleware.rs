//! Shared middleware layers for the API.

use std::time::Instant;

use axum::{
    extract::MatchedPath,
    http::{Request, Uri, header},
    middleware::Next,
    response::IntoResponse,
};

use crate::error::CONTENT_TYPE_SCHEMA_REGISTRY;

/// Middleware that normalizes case-insensitive boolean values in query strings.
///
/// The Confluent Python client sends `True`/`False` (Python's `str(bool)`)
/// but serde only accepts lowercase. This rewrites the URI once so every
/// endpoint handles it transparently.
pub async fn normalize_query_bools(
    mut req: Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    if let Some(query) = req.uri().query() {
        let normalized: String = query
            .split('&')
            .map(|pair| match pair.split_once('=') {
                Some((k, v)) if v.eq_ignore_ascii_case("true") => format!("{k}=true"),
                Some((k, v)) if v.eq_ignore_ascii_case("false") => format!("{k}=false"),
                _ => pair.to_string(),
            })
            .collect::<Vec<_>>()
            .join("&");

        if normalized != query {
            let pq = format!("{}?{normalized}", req.uri().path());
            if let Ok(uri) = pq.parse::<Uri>() {
                *req.uri_mut() = uri;
            }
        }
    }
    next.run(req).await
}

// -- Content-type negotiation --

const CONTENT_TYPE_JSON: &str = "application/json";

/// Middleware that sets the response `Content-Type` based on the request `Accept`
/// header, matching real Confluent Schema Registry behaviour:
/// - If the client sends `Accept: application/vnd.schemaregistry.v1+json`,
///   respond with that content type.
/// - Otherwise, default to `application/json`.
pub async fn content_type_negotiation(
    req: Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    let use_vendor = req
        .headers()
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|accept| accept.contains(CONTENT_TYPE_SCHEMA_REGISTRY));

    let mut response = next.run(req).await;

    let ct = if use_vendor {
        CONTENT_TYPE_SCHEMA_REGISTRY
    } else {
        CONTENT_TYPE_JSON
    };
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, ct.parse().unwrap());

    response
}

// -- Request instrumentation --

/// Middleware that records Prometheus metrics for every request:
/// - `http_requests_total` counter with `method`, `path`, `status` labels
/// - `http_request_duration_seconds` histogram with `method`, `path` labels
///
/// The `/metrics` endpoint itself is excluded to avoid a self-referential
/// feedback loop that pollutes counters with scraper traffic.
pub async fn track_metrics(req: Request<axum::body::Body>, next: Next) -> impl IntoResponse {
    let method = req.method().to_string();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |mp| mp.as_str().to_owned());

    let start = Instant::now();
    let response = next.run(req).await;

    // Skip self-instrumentation for the metrics endpoint.
    if path == "/metrics" {
        return response;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status,
    )
    .increment(1);

    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => path,
    )
    .record(elapsed);

    response
}

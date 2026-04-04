//! Shared middleware layers for the API.

use axum::http::{HeaderName, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::error::CONTENT_TYPE_SCHEMA_REGISTRY;

/// Returns a layer that sets `Content-Type: application/vnd.schemaregistry.v1+json`
/// on every response.
pub fn content_type_layer() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static(CONTENT_TYPE_SCHEMA_REGISTRY),
    )
}

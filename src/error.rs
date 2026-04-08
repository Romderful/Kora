//! Confluent-compatible error types and response formatting.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

// -- Constants --

/// Content type for all Schema Registry responses.
pub const CONTENT_TYPE_SCHEMA_REGISTRY: &str = "application/vnd.schemaregistry.v1+json";

// -- Types --

/// Application-level errors mapped to Confluent Schema Registry error codes.
#[derive(Debug, thiserror::Error)]
pub enum KoraError {
    /// Invalid schema (42201).
    #[error("Invalid schema: {0}")]
    InvalidSchema(String),
    /// Subject not found (40401).
    #[error("Subject not found")]
    SubjectNotFound,
    /// Version not found (40402).
    #[error("Version not found")]
    VersionNotFound,
    /// Schema not found (40403).
    #[error("Schema not found")]
    SchemaNotFound,
    /// Backend data store error (50001).
    #[error("Error in the backend data store: {0}")]
    BackendDataStore(String),
}

/// Confluent-compatible JSON error body.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error_code: u32,
    message: String,
}

// -- Impls --

impl From<sqlx::Error> for KoraError {
    fn from(err: sqlx::Error) -> Self {
        Self::BackendDataStore(err.to_string())
    }
}

impl KoraError {
    /// Confluent numeric error code.
    const fn error_code(&self) -> u32 {
        match self {
            Self::InvalidSchema(_) => 42201,
            Self::SubjectNotFound => 40401,
            Self::VersionNotFound => 40402,
            Self::SchemaNotFound => 40403,
            Self::BackendDataStore(_) => 50001,
        }
    }

    /// HTTP status code derived from the Confluent error code.
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidSchema(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::SubjectNotFound | Self::VersionNotFound | Self::SchemaNotFound => {
                StatusCode::NOT_FOUND
            }
            Self::BackendDataStore(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for KoraError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error_code: self.error_code(),
            message: self.to_string(),
        };
        (self.status_code(), Json(body)).into_response()
    }
}

//! Confluent-compatible error types and response formatting.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

/// Content type for all Schema Registry responses.
pub const CONTENT_TYPE_SCHEMA_REGISTRY: &str = "application/vnd.schemaregistry.v1+json";

/// Application-level errors mapped to Confluent Schema Registry error codes.
#[derive(Debug, thiserror::Error)]
pub enum KoraError {
    /// Invalid schema (42201).
    #[error("Invalid schema: {0}")]
    InvalidSchema(String),
    /// Backend data store error (50001).
    #[error("Error in the backend data store: {0}")]
    BackendDataStore(String),
}

impl From<sqlx::Error> for KoraError {
    fn from(err: sqlx::Error) -> Self {
        Self::BackendDataStore(err.to_string())
    }
}

/// Confluent-compatible JSON error body.
#[derive(Debug, Serialize, Deserialize)]
struct ErrorBody {
    error_code: u32,
    message: String,
}

impl KoraError {
    /// Confluent numeric error code.
    const fn error_code(&self) -> u32 {
        match self {
            Self::InvalidSchema(_) => 42201,
            Self::BackendDataStore(_) => 50001,
        }
    }

    /// HTTP status code derived from the Confluent error code.
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidSchema(_) => StatusCode::UNPROCESSABLE_ENTITY,
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
        let json = serde_json::to_string(&body).unwrap_or_else(|_| {
            format!(
                r#"{{"error_code":50001,"message":"{}"}}"#,
                "Error in the backend data store"
            )
        });

        (self.status_code(), json).into_response()
    }
}

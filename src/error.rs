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
    /// Subject not found (40401).
    #[error("Subject not found")]
    SubjectNotFound,
    /// Version not found (40402).
    #[error("Version not found")]
    VersionNotFound,
    /// Schema not found (40403).
    #[error("Schema not found")]
    SchemaNotFound,
    /// Invalid schema (42201).
    #[error("Invalid schema: {0}")]
    InvalidSchema(String),
    /// Invalid version (42202).
    #[error("Invalid version: {0}")]
    InvalidVersion(String),
    /// Incompatible schema (40901).
    #[error("Schema being registered is incompatible with an earlier schema")]
    IncompatibleSchema,
    /// Backend data store error (50001).
    #[error("Error in the backend data store")]
    BackendDataStore,
    /// Operation timed out (50002).
    #[error("Operation timed out")]
    OperationTimedOut,
    /// Error forwarding request (50003).
    #[error("Error while forwarding the request to the leader")]
    ForwardingError,
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
            Self::SubjectNotFound => 40401,
            Self::VersionNotFound => 40402,
            Self::SchemaNotFound => 40403,
            Self::InvalidSchema(_) => 42201,
            Self::InvalidVersion(_) => 42202,
            Self::IncompatibleSchema => 40901,
            Self::BackendDataStore => 50001,
            Self::OperationTimedOut => 50002,
            Self::ForwardingError => 50003,
        }
    }

    /// HTTP status code derived from the Confluent error code.
    const fn status_code(&self) -> StatusCode {
        match self {
            Self::SubjectNotFound | Self::VersionNotFound | Self::SchemaNotFound => {
                StatusCode::NOT_FOUND
            }
            Self::InvalidSchema(_) | Self::InvalidVersion(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::IncompatibleSchema => StatusCode::CONFLICT,
            Self::BackendDataStore | Self::OperationTimedOut | Self::ForwardingError => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn error_body(error: KoraError) -> (StatusCode, ErrorBody) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        let body: ErrorBody = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }

    #[tokio::test]
    async fn subject_not_found() {
        let (status, body) = error_body(KoraError::SubjectNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error_code, 40401);
        assert_eq!(body.message, "Subject not found");
    }

    #[tokio::test]
    async fn version_not_found() {
        let (status, body) = error_body(KoraError::VersionNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error_code, 40402);
    }

    #[tokio::test]
    async fn schema_not_found() {
        let (status, body) = error_body(KoraError::SchemaNotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error_code, 40403);
    }

    #[tokio::test]
    async fn invalid_schema() {
        let (status, body) = error_body(KoraError::InvalidSchema("bad field".into())).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.error_code, 42201);
        assert!(body.message.contains("bad field"));
    }

    #[tokio::test]
    async fn invalid_version() {
        let (status, body) = error_body(KoraError::InvalidVersion("abc".into())).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.error_code, 42202);
    }

    #[tokio::test]
    async fn incompatible_schema() {
        let (status, body) = error_body(KoraError::IncompatibleSchema).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body.error_code, 40901);
    }

    #[tokio::test]
    async fn backend_data_store() {
        let (status, body) = error_body(KoraError::BackendDataStore).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error_code, 50001);
    }

    #[tokio::test]
    async fn operation_timed_out() {
        let (status, body) = error_body(KoraError::OperationTimedOut).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error_code, 50002);
    }

    #[tokio::test]
    async fn forwarding_error() {
        let (status, body) = error_body(KoraError::ForwardingError).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error_code, 50003);
    }

}

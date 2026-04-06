//! Tests for Confluent-compatible error responses.

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use kora::error::KoraError;

async fn error_response(error: KoraError) -> (StatusCode, serde_json::Value) {
    let response = error.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, body)
}

#[tokio::test]
async fn invalid_schema() {
    let (status, body) = error_response(KoraError::InvalidSchema("bad field".into())).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error_code"], 42201);
    assert!(body["message"].as_str().unwrap().contains("bad field"));
}

#[tokio::test]
async fn backend_data_store() {
    let (status, body) = error_response(KoraError::BackendDataStore("test".into())).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error_code"], 50001);
}

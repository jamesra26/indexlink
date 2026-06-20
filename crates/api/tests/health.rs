use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use indexlink_api::{build_router, ApiState, ReadinessCheck, ReadinessError};
use serde_json::{json, Value};
use tower::ServiceExt;

struct FakeReadiness {
    available: bool,
}

#[async_trait]
impl ReadinessCheck for FakeReadiness {
    async fn check(&self) -> Result<(), ReadinessError> {
        if self.available {
            Ok(())
        } else {
            Err(ReadinessError::new(
                "postgres://secret:password@internal/database: connection refused",
            ))
        }
    }
}

fn app(available: bool) -> axum::Router {
    build_router(ApiState::with_readiness(
        Arc::new(FakeReadiness { available }),
        "0.1.0",
    ))
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body should be readable")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response should be valid JSON")
}

#[tokio::test]
async fn health_returns_ok_with_expected_json() {
    let response = app(true)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        json!({
            "status": "ok",
            "service": "indexlink-server",
            "version": "0.1.0"
        })
    );
}

#[tokio::test]
async fn ready_returns_ok_when_database_is_available() {
    let response = app(true)
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        json!({"status": "ready", "database": "ok"})
    );
}

#[tokio::test]
async fn ready_hides_internal_error_when_database_is_unavailable() {
    let response = app(false)
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response_json(response).await;
    assert_eq!(
        body,
        json!({
            "error": {
                "code": "service_unavailable",
                "message": "database is unavailable"
            }
        })
    );
    let serialized = body.to_string();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("internal"));
    assert!(!serialized.contains("password"));
}

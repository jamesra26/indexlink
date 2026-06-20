use axum::Json;
use serde::Serialize;

use crate::ApiState;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: String,
}

pub(crate) async fn health(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "indexlink-server",
        version: state.version().to_owned(),
    })
}

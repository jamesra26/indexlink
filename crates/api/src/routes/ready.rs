use axum::Json;
use serde::Serialize;

use crate::{ApiError, ApiState};

#[derive(Debug, Serialize)]
pub(crate) struct ReadyResponse {
    status: &'static str,
    database: &'static str,
}

pub(crate) async fn ready(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Result<Json<ReadyResponse>, ApiError> {
    state.check_readiness().await.map_err(|error| {
        tracing::warn!(error = %error, "database readiness check failed");
        ApiError::ServiceUnavailable
    })?;

    Ok(Json(ReadyResponse {
        status: "ready",
        database: "ok",
    }))
}

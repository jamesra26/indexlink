//! Decision record HTTP routes.

use axum::{
    extract::{rejection::PathRejection, Path, State},
    routing::get,
    Json, Router,
};
use decision_records::DecisionRecord;
use uuid::Uuid;

use crate::{ApiError, ApiState};

/// Build decision record routes.
pub(crate) fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/investment-plans/:id/decisions",
            get(list_records_for_plan),
        )
        .route("/decisions/:id", get(get_record))
}

/// List decision records for one investment plan.
async fn list_records_for_plan(
    State(state): State<ApiState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<Vec<DecisionRecord>>, ApiError> {
    let Path(plan_id) = id.map_err(|_| ApiError::BadRequest)?;
    Ok(Json(state.records().list_by_plan(plan_id).await?))
}

/// Fetch one decision record.
async fn get_record(
    State(state): State<ApiState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<DecisionRecord>, ApiError> {
    let Path(id) = id.map_err(|_| ApiError::BadRequest)?;
    Ok(Json(state.records().get(id).await?))
}

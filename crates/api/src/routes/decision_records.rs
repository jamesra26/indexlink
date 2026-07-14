//! Decision record history HTTP routes.

use ::decision_records::{DecisionRecord, DecisionRecordListQuery};
use axum::{
    extract::{
        rejection::{PathRejection, QueryRejection},
        Path, Query, State,
    },
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{ApiError, ApiState};

/// Query parameters accepted by the decision-record history route.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionRecordListRequest {
    /// Optional bounded number of records to return.
    limit: Option<u16>,
}

impl DecisionRecordListRequest {
    /// Convert the HTTP query into the validated domain query.
    fn into_domain(self) -> Result<DecisionRecordListQuery, ApiError> {
        self.limit
            .map(DecisionRecordListQuery::new)
            .transpose()
            .map_err(|_| ApiError::BadRequest)?
            .map_or_else(|| Ok(DecisionRecordListQuery::default()), Ok)
    }
}

/// Build decision-record history routes.
pub(crate) fn router() -> Router<ApiState> {
    Router::new()
        .route(
            "/investment-plans/:id/decisions",
            get(list_decision_records),
        )
        .route("/decisions/:id", get(get_decision_record))
}

/// List the newest persisted decision records for one existing investment plan.
async fn list_decision_records(
    State(state): State<ApiState>,
    id: Result<Path<Uuid>, PathRejection>,
    query: Result<Query<DecisionRecordListRequest>, QueryRejection>,
) -> Result<Json<Vec<DecisionRecord>>, ApiError> {
    let Path(plan_id) = id.map_err(|_| ApiError::BadRequest)?;
    let Query(query) = query.map_err(|_| ApiError::BadRequest)?;
    let query = query.into_domain()?;

    state.plans().get(plan_id).await?;
    Ok(Json(
        state
            .decision_records()
            .list_by_plan_with_query(plan_id, query)
            .await?,
    ))
}

/// Fetch one persisted decision record by its ID.
async fn get_decision_record(
    State(state): State<ApiState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<DecisionRecord>, ApiError> {
    let Path(id) = id.map_err(|_| ApiError::BadRequest)?;
    Ok(Json(state.decision_records().get(id).await?))
}

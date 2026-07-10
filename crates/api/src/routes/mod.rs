mod decision_preview;
mod health;
mod investment_plans;
mod ready;

use axum::{routing::get, Router};

use crate::ApiState;

pub(crate) fn router() -> Router<ApiState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(ready::ready))
        .merge(decision_preview::router())
        .merge(investment_plans::router())
}

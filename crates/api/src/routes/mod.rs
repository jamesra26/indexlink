mod health;
mod ready;

use axum::{routing::get, Router};

use crate::ApiState;

pub(crate) fn router() -> Router<ApiState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(ready::ready))
}

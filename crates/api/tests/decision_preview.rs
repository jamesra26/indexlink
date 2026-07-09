use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use broker::MockBroker;
use http_body_util::BodyExt;
use indexlink_api::{build_router, ApiState, ReadinessCheck, ReadinessError};
use investment_plans::{
    CreateInvestmentPlan, InvestmentPlan, InvestmentPlanRepository, InvestmentPlanService,
    PlanRepositoryError, ScheduleKind, UpdateInvestmentPlan,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

/// Readiness stub used by decision preview route tests.
struct Ready;

#[async_trait]
impl ReadinessCheck for Ready {
    /// Always report dependencies as available.
    async fn check(&self) -> Result<(), ReadinessError> {
        Ok(())
    }
}

/// In-memory repository fake for previewing decisions through the API router.
#[derive(Default)]
struct FakeRepository {
    plans: Mutex<Vec<InvestmentPlan>>,
}

#[async_trait]
impl InvestmentPlanRepository for FakeRepository {
    /// Store the normalized create input as a persisted plan.
    async fn create(
        &self,
        input: CreateInvestmentPlan,
    ) -> Result<InvestmentPlan, PlanRepositoryError> {
        let mut plans = self.plans.lock().unwrap();
        let plan = plan_from(Uuid::from_u128((plans.len() + 1) as u128), input);
        plans.push(plan.clone());
        Ok(plan)
    }

    /// Return a snapshot of stored plans.
    async fn list(&self) -> Result<Vec<InvestmentPlan>, PlanRepositoryError> {
        Ok(self.plans.lock().unwrap().clone())
    }

    /// Return one stored plan by ID.
    async fn get(&self, id: Uuid) -> Result<InvestmentPlan, PlanRepositoryError> {
        self.plans
            .lock()
            .unwrap()
            .iter()
            .find(|plan| plan.id == id)
            .cloned()
            .ok_or(PlanRepositoryError::NotFound)
    }

    /// Merge and store updates through the repository port.
    async fn update(
        &self,
        id: Uuid,
        input: UpdateInvestmentPlan,
    ) -> Result<InvestmentPlan, PlanRepositoryError> {
        let mut plans = self.plans.lock().unwrap();
        let plan = plans
            .iter_mut()
            .find(|plan| plan.id == id)
            .ok_or(PlanRepositoryError::NotFound)?;

        if let Some(name) = input.name {
            plan.name = name;
        }
        if let Some(base_contribution) = input.base_contribution {
            plan.base_contribution = base_contribution;
        }
        if let Some(schedule_day) = input.schedule_day {
            plan.schedule_day = schedule_day;
        }
        if let Some(max_single_execution) = input.max_single_execution {
            plan.max_single_execution = max_single_execution;
        }
        if let Some(is_active) = input.is_active {
            plan.is_active = is_active;
        }

        Ok(plan.clone())
    }

    /// Active-state toggles are outside this route's scope.
    async fn set_active(
        &self,
        _id: Uuid,
        _is_active: bool,
    ) -> Result<InvestmentPlan, PlanRepositoryError> {
        Err(PlanRepositoryError::Unavailable)
    }
}

/// Convert service input into a stored test plan.
fn plan_from(id: Uuid, input: CreateInvestmentPlan) -> InvestmentPlan {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    InvestmentPlan {
        id,
        name: input.name,
        symbol: input.symbol,
        base_contribution: input.base_contribution,
        currency: input.currency,
        schedule_kind: input.schedule_kind,
        schedule_day: input.schedule_day,
        max_single_execution: input.max_single_execution,
        is_active: true,
        created_at: now,
        updated_at: now,
    }
}

/// Build an API app wired to fake investment plans and a mock broker.
fn app(repository: Arc<FakeRepository>, broker: Arc<MockBroker>) -> axum::Router {
    build_router(ApiState::with_readiness_plans_and_broker(
        Arc::new(Ready),
        InvestmentPlanService::new(repository),
        broker,
        "0.1.0",
    ))
}

/// Parse an HTTP response body as JSON.
async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Build a normalized domain input for seeding the fake repository.
fn create_input() -> CreateInvestmentPlan {
    CreateInvestmentPlan {
        name: "Core ETF".to_owned(),
        symbol: "VOO".to_owned(),
        base_contribution: Decimal::new(1000, 0),
        currency: "USD".to_owned(),
        schedule_kind: ScheduleKind::Monthly,
        schedule_day: 15,
        max_single_execution: Decimal::new(1500, 0),
    }
}

/// Build a valid decision preview payload.
fn preview_payload(day_of_month: i16, regime: &str) -> Value {
    json!({
        "day_of_month": day_of_month,
        "bucket_allocation": {
            "core_ratio": "0.80",
            "opportunity_ratio": "0.20"
        },
        "fundamental": {
            "score": 0.10,
            "cape_percentile": 0.10,
            "erp_percentile": 0.90
        },
        "trend": {
            "score": 0.50,
            "ma_distance_percentile": 0.50,
            "rsi_percentile": 0.50,
            "vix_percentile": 0.50,
            "regime": regime
        },
        "sentiment": {"score": 0.80},
        "paper_order": {
            "idempotency_key": "decision-preview-demo-1",
            "side": "buy",
            "order_type": "market",
            "quantity": "1.00"
        }
    })
}

/// Verify a due executable decision submits one MockBroker paper order.
#[tokio::test]
async fn decision_preview_submits_mock_paper_order_when_due() {
    let repository = Arc::new(FakeRepository::default());
    let broker = Arc::new(MockBroker::paper_only());
    let created = repository.create(create_input()).await.unwrap();
    let app = app(repository, Arc::clone(&broker));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/investment-plans/{}/decision-preview", created.id))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(preview_payload(15, "neutral").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["execution"]["status"], json!("due"));
    assert_eq!(
        body["execution"]["bucket_split"]["core_contribution"],
        json!("800.00")
    );
    assert_eq!(body["decision"]["action"], json!("overweight"));
    assert_eq!(body["paper_order_ack"]["status"], json!("accepted"));
    assert_eq!(broker.accepted_orders().len(), 1);
}

/// Verify non-due previews never submit paper orders.
#[tokio::test]
async fn decision_preview_waiting_does_not_submit_order() {
    let repository = Arc::new(FakeRepository::default());
    let broker = Arc::new(MockBroker::paper_only());
    let created = repository.create(create_input()).await.unwrap();
    let app = app(repository, Arc::clone(&broker));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/investment-plans/{}/decision-preview", created.id))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(preview_payload(16, "neutral").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["execution"]["status"], json!("waiting"));
    assert!(body.get("paper_order_ack").is_none());
    assert!(broker.accepted_orders().is_empty());
}

/// Verify tactical-delay decisions do not submit paper orders even when due.
#[tokio::test]
async fn decision_preview_tactical_delay_does_not_submit_order() {
    let repository = Arc::new(FakeRepository::default());
    let broker = Arc::new(MockBroker::paper_only());
    let created = repository.create(create_input()).await.unwrap();
    let app = app(repository, Arc::clone(&broker));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/investment-plans/{}/decision-preview", created.id))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(preview_payload(15, "overheated").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["execution"]["status"], json!("due"));
    assert_eq!(body["decision"]["action"], json!("tactical_delay"));
    assert!(body.get("paper_order_ack").is_none());
    assert!(broker.accepted_orders().is_empty());
}

/// Verify malformed previews return the shared bad-request envelope.
#[tokio::test]
async fn decision_preview_maps_bad_input_to_safe_bad_request() {
    let repository = Arc::new(FakeRepository::default());
    let broker = Arc::new(MockBroker::paper_only());
    let created = repository.create(create_input()).await.unwrap();
    let app = app(repository, broker);
    let mut waiting_invalid_order = preview_payload(16, "neutral");
    waiting_invalid_order["paper_order"]["limit_price"] = json!("10.00");
    let mut tactical_delay_invalid_order = preview_payload(15, "overheated");
    tactical_delay_invalid_order["paper_order"]["limit_price"] = json!("10.00");

    for (uri, body) in [
        (
            "/investment-plans/not-a-uuid/decision-preview".to_owned(),
            preview_payload(15, "neutral").to_string(),
        ),
        (
            format!("/investment-plans/{}/decision-preview", created.id),
            json!({"day_of_month": 32}).to_string(),
        ),
        (
            format!("/investment-plans/{}/decision-preview", created.id),
            json!({
                "day_of_month": 15,
                "fundamental": {
                    "score": 1.20,
                    "cape_percentile": 0.10,
                    "erp_percentile": 0.90
                },
                "trend": {
                    "score": 0.50,
                    "ma_distance_percentile": 0.50,
                    "rsi_percentile": 0.50,
                    "vix_percentile": 0.50,
                    "regime": "neutral"
                }
            })
            .to_string(),
        ),
        (
            format!("/investment-plans/{}/decision-preview", created.id),
            json!({
                "day_of_month": 15,
                "fundamental": {
                    "score": 0.10,
                    "cape_percentile": 0.10,
                    "erp_percentile": 0.90
                },
                "trend": {
                    "score": 0.50,
                    "ma_distance_percentile": 0.50,
                    "rsi_percentile": 0.50,
                    "vix_percentile": 0.50,
                    "regime": "neutral"
                },
                "paper_order": {
                    "idempotency_key": "bad-market-limit",
                    "side": "buy",
                    "order_type": "market",
                    "quantity": "1.00",
                    "limit_price": "10.00"
                }
            })
            .to_string(),
        ),
        (
            format!("/investment-plans/{}/decision-preview", created.id),
            waiting_invalid_order.to_string(),
        ),
        (
            format!("/investment-plans/{}/decision-preview", created.id),
            tactical_delay_invalid_order.to_string(),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({"error": {"code": "bad_request", "message": "invalid request"}})
        );
    }
}

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use broker::MockBroker;
use decision_records::{
    CreateDecisionRecord, DecisionRecord, DecisionRecordRepository, DecisionRecordRepositoryError,
    DecisionRecordService,
};
use http_body_util::BodyExt;
use indexlink_api::{build_router, ApiState, ReadinessCheck, ReadinessError};
use investment_plans::{
    CreateInvestmentPlan, InvestmentPlan, InvestmentPlanRepository, InvestmentPlanService,
    PlanRepositoryError, UpdateInvestmentPlan,
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

/// Readiness stub used by decision record route tests.
struct Ready;

#[async_trait]
impl ReadinessCheck for Ready {
    /// Always report dependencies as available.
    async fn check(&self) -> Result<(), ReadinessError> {
        Ok(())
    }
}

/// Investment plan repository stub unused by decision record routes.
struct UnusedPlans;

#[async_trait]
impl InvestmentPlanRepository for UnusedPlans {
    /// Reject creates because these routes do not mutate plans.
    async fn create(
        &self,
        _input: CreateInvestmentPlan,
    ) -> Result<InvestmentPlan, PlanRepositoryError> {
        Err(PlanRepositoryError::Unavailable)
    }

    /// Reject lists because these routes do not read plans.
    async fn list(&self) -> Result<Vec<InvestmentPlan>, PlanRepositoryError> {
        Err(PlanRepositoryError::Unavailable)
    }

    /// Reject gets because these routes do not read plans.
    async fn get(&self, _id: Uuid) -> Result<InvestmentPlan, PlanRepositoryError> {
        Err(PlanRepositoryError::Unavailable)
    }

    /// Reject updates because these routes do not mutate plans.
    async fn update(
        &self,
        _id: Uuid,
        _input: UpdateInvestmentPlan,
    ) -> Result<InvestmentPlan, PlanRepositoryError> {
        Err(PlanRepositoryError::Unavailable)
    }

    /// Reject active-state changes because these routes do not mutate plans.
    async fn set_active(
        &self,
        _id: Uuid,
        _is_active: bool,
    ) -> Result<InvestmentPlan, PlanRepositoryError> {
        Err(PlanRepositoryError::Unavailable)
    }
}

/// In-memory decision record repository fake.
struct FakeRecords {
    records: Mutex<Vec<DecisionRecord>>,
    fail: bool,
}

impl FakeRecords {
    /// Build a repository fake with stored records.
    fn with_records(records: Vec<DecisionRecord>) -> Self {
        Self {
            records: Mutex::new(records),
            fail: false,
        }
    }

    /// Build a repository fake that reports unavailable.
    fn unavailable() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            fail: true,
        }
    }
}

#[async_trait]
impl DecisionRecordRepository for FakeRecords {
    /// Store one record through the fake repository.
    async fn create(
        &self,
        input: CreateDecisionRecord,
    ) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
        if self.fail {
            return Err(DecisionRecordRepositoryError::Unavailable);
        }

        let mut records = self.records.lock().unwrap();
        let record = record_from(Uuid::from_u128((records.len() + 1) as u128), input.plan_id);
        records.push(record.clone());
        Ok(record)
    }

    /// List fake records for one plan.
    async fn list_by_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<DecisionRecord>, DecisionRecordRepositoryError> {
        if self.fail {
            return Err(DecisionRecordRepositoryError::Unavailable);
        }

        Ok(self
            .records
            .lock()
            .unwrap()
            .iter()
            .filter(|record| record.plan_id == plan_id)
            .cloned()
            .collect())
    }

    /// Fetch one fake record.
    async fn get(&self, id: Uuid) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
        if self.fail {
            return Err(DecisionRecordRepositoryError::Unavailable);
        }

        self.records
            .lock()
            .unwrap()
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .ok_or(DecisionRecordRepositoryError::NotFound)
    }
}

/// Build an API app wired to a fake decision record service.
fn app(records: Arc<dyn DecisionRecordRepository>) -> axum::Router {
    build_router(ApiState::with_readiness_plans_records_and_broker(
        Arc::new(Ready),
        InvestmentPlanService::new(Arc::new(UnusedPlans)),
        DecisionRecordService::new(records),
        Arc::new(MockBroker::paper_only()),
        "0.1.0",
    ))
}

/// Parse an HTTP response body as JSON.
async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Build a stored test decision record.
fn record_from(id: Uuid, plan_id: Uuid) -> DecisionRecord {
    DecisionRecord {
        id,
        plan_id,
        symbol: "VOO".to_owned(),
        currency: "USD".to_owned(),
        execution_status: "due".to_owned(),
        planned_contribution: Some("1000.00".to_owned()),
        execution_snapshot: json!({"status": "due", "bucket_split": {"core": "800.00"}}),
        fundamental_snapshot: json!({"score": 0.10}),
        trend_snapshot: json!({"score": 0.50, "regime": "neutral"}),
        sentiment_snapshot: Some(json!({"score": 0.80, "provider": "manual"})),
        decision_snapshot: json!({"action": "overweight", "multiplier": 1.32}),
        broker_order_request: Some(json!({"side": "buy", "quantity": "1.00"})),
        broker_order_ack: Some(json!({"status": "accepted", "order_id": "MOCK-1"})),
        summary: "Decision preview for VOO is due.".to_owned(),
        created_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
    }
}

/// Verify decision records can be listed by plan.
#[tokio::test]
async fn list_decision_records_by_plan() {
    let plan_id = Uuid::from_u128(7);
    let other_plan_id = Uuid::from_u128(8);
    let first = record_from(Uuid::from_u128(1), plan_id);
    let other = record_from(Uuid::from_u128(2), other_plan_id);
    let app = app(Arc::new(FakeRecords::with_records(vec![
        first.clone(),
        other,
    ])));

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/investment-plans/{plan_id}/decisions"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body, json!([first]));
}

/// Verify one decision record can be fetched by ID.
#[tokio::test]
async fn get_decision_record_by_id() {
    let record = record_from(Uuid::from_u128(1), Uuid::from_u128(7));
    let app = app(Arc::new(FakeRecords::with_records(vec![record.clone()])));

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/decisions/{}", record.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["id"], json!(record.id));
    assert_eq!(body["execution_snapshot"]["status"], json!("due"));
    assert_eq!(body["decision_snapshot"]["action"], json!("overweight"));
}

/// Verify decision record routes map bad IDs and repository errors safely.
#[tokio::test]
async fn decision_record_routes_map_errors_safely() {
    let missing_app = app(Arc::new(FakeRecords::with_records(Vec::new())));
    let bad_id = missing_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/decisions/not-a-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_id.status(), StatusCode::BAD_REQUEST);

    let missing = missing_app
        .oneshot(
            Request::builder()
                .uri(format!("/decisions/{}", Uuid::from_u128(404)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(missing).await,
        json!({"error": {"code": "not_found", "message": "resource not found"}})
    );

    let unavailable = app(Arc::new(FakeRecords::unavailable()))
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/investment-plans/{}/decisions",
                    Uuid::from_u128(7)
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
}

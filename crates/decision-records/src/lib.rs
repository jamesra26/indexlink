#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Decision record domain and application port.
//!
//! This crate stores audit-ready decision snapshots. It deliberately keeps
//! external provider details as JSON snapshots so Qwen, quant, and broker
//! adapters can evolve without losing the exact input and output seen at
//! decision time.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

/// Persisted decision record for one preview or execution.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DecisionRecord {
    /// Record ID.
    pub id: Uuid,
    /// Related investment plan ID.
    pub plan_id: Uuid,
    /// Investment symbol snapshot.
    pub symbol: String,
    /// Currency snapshot.
    pub currency: String,
    /// Execution status snapshot, for example `due`, `waiting`, or `inactive`.
    pub execution_status: String,
    /// Optional planned contribution encoded as a decimal string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned_contribution: Option<String>,
    /// Execution preview snapshot, including schedule and bucket split.
    pub execution_snapshot: Value,
    /// Fundamental input snapshot used by the decision engine.
    pub fundamental_snapshot: Value,
    /// Trend input snapshot used by the decision engine.
    pub trend_snapshot: Value,
    /// Optional AI sentiment snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentiment_snapshot: Option<Value>,
    /// Decision output snapshot.
    pub decision_snapshot: Value,
    /// Optional broker order request snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_order_request: Option<Value>,
    /// Optional broker order acknowledgement snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_order_ack: Option<Value>,
    /// User-facing summary generated for this decision.
    pub summary: String,
    /// Creation time.
    pub created_at: OffsetDateTime,
}

/// Input used to create a decision record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateDecisionRecord {
    /// Related investment plan ID.
    pub plan_id: Uuid,
    /// Investment symbol snapshot.
    pub symbol: String,
    /// Currency snapshot.
    pub currency: String,
    /// Execution status snapshot.
    pub execution_status: String,
    /// Optional planned contribution encoded as a decimal string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned_contribution: Option<String>,
    /// Execution preview snapshot.
    pub execution_snapshot: Value,
    /// Fundamental input snapshot.
    pub fundamental_snapshot: Value,
    /// Trend input snapshot.
    pub trend_snapshot: Value,
    /// Optional AI sentiment snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentiment_snapshot: Option<Value>,
    /// Decision output snapshot.
    pub decision_snapshot: Value,
    /// Optional broker order request snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_order_request: Option<Value>,
    /// Optional broker order acknowledgement snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_order_ack: Option<Value>,
    /// User-facing summary generated for this decision.
    pub summary: String,
}

/// Repository errors hidden behind the application layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecisionRecordRepositoryError {
    /// The requested decision record does not exist.
    #[error("decision record not found")]
    NotFound,
    /// The storage backend is unavailable.
    #[error("decision record repository unavailable")]
    Unavailable,
}

/// Application-layer errors for decision records.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecisionRecordApplicationError {
    /// The requested decision record does not exist.
    #[error("decision record not found")]
    NotFound,
    /// The decision record backend is unavailable.
    #[error("decision record backend unavailable")]
    Unavailable,
}

impl From<DecisionRecordRepositoryError> for DecisionRecordApplicationError {
    fn from(error: DecisionRecordRepositoryError) -> Self {
        match error {
            DecisionRecordRepositoryError::NotFound => Self::NotFound,
            DecisionRecordRepositoryError::Unavailable => Self::Unavailable,
        }
    }
}

/// Outbound repository port for decision records.
#[async_trait]
pub trait DecisionRecordRepository: Send + Sync {
    /// Persist one decision record snapshot.
    async fn create(
        &self,
        input: CreateDecisionRecord,
    ) -> Result<DecisionRecord, DecisionRecordRepositoryError>;

    /// List decision records for one investment plan.
    async fn list_by_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<DecisionRecord>, DecisionRecordRepositoryError>;

    /// Fetch one decision record by ID.
    async fn get(&self, id: Uuid) -> Result<DecisionRecord, DecisionRecordRepositoryError>;
}

/// Application service for decision record use cases.
#[derive(Clone)]
pub struct DecisionRecordService {
    repository: Arc<dyn DecisionRecordRepository>,
}

impl DecisionRecordService {
    /// Build the service from a repository implementation.
    #[must_use]
    pub fn new(repository: Arc<dyn DecisionRecordRepository>) -> Self {
        Self { repository }
    }

    /// Persist one decision record snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionRecordApplicationError::Unavailable`] if storage is unavailable.
    pub async fn create(
        &self,
        input: CreateDecisionRecord,
    ) -> Result<DecisionRecord, DecisionRecordApplicationError> {
        self.repository.create(input).await.map_err(Into::into)
    }

    /// List decision records for one investment plan.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionRecordApplicationError::Unavailable`] if storage is unavailable.
    pub async fn list_by_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<DecisionRecord>, DecisionRecordApplicationError> {
        self.repository
            .list_by_plan(plan_id)
            .await
            .map_err(Into::into)
    }

    /// Fetch one decision record by ID.
    ///
    /// # Errors
    ///
    /// Returns [`DecisionRecordApplicationError::NotFound`] when the record does not exist,
    /// or [`DecisionRecordApplicationError::Unavailable`] if storage is unavailable.
    pub async fn get(&self, id: Uuid) -> Result<DecisionRecord, DecisionRecordApplicationError> {
        self.repository.get(id).await.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;

    #[derive(Default)]
    struct FakeRepository {
        records: Mutex<Vec<DecisionRecord>>,
    }

    #[async_trait]
    impl DecisionRecordRepository for FakeRepository {
        async fn create(
            &self,
            input: CreateDecisionRecord,
        ) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
            let mut records = self.records.lock().unwrap();
            let record = record_from(Uuid::from_u128((records.len() + 1) as u128), input);
            records.push(record.clone());
            Ok(record)
        }

        async fn list_by_plan(
            &self,
            plan_id: Uuid,
        ) -> Result<Vec<DecisionRecord>, DecisionRecordRepositoryError> {
            Ok(self
                .records
                .lock()
                .unwrap()
                .iter()
                .filter(|record| record.plan_id == plan_id)
                .cloned()
                .collect())
        }

        async fn get(&self, id: Uuid) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
            self.records
                .lock()
                .unwrap()
                .iter()
                .find(|record| record.id == id)
                .cloned()
                .ok_or(DecisionRecordRepositoryError::NotFound)
        }
    }

    fn input(plan_id: Uuid) -> CreateDecisionRecord {
        CreateDecisionRecord {
            plan_id,
            symbol: "VOO".to_owned(),
            currency: "USD".to_owned(),
            execution_status: "due".to_owned(),
            planned_contribution: Some("1000.00".to_owned()),
            execution_snapshot: json!({"status": "due"}),
            fundamental_snapshot: json!({"score": 0.2}),
            trend_snapshot: json!({"score": 0.5}),
            sentiment_snapshot: Some(json!({"score": 0.1})),
            decision_snapshot: json!({"action": "standard"}),
            broker_order_request: Some(json!({"side": "buy"})),
            broker_order_ack: Some(json!({"status": "accepted"})),
            summary: "standard execution".to_owned(),
        }
    }

    fn record_from(id: Uuid, input: CreateDecisionRecord) -> DecisionRecord {
        DecisionRecord {
            id,
            plan_id: input.plan_id,
            symbol: input.symbol,
            currency: input.currency,
            execution_status: input.execution_status,
            planned_contribution: input.planned_contribution,
            execution_snapshot: input.execution_snapshot,
            fundamental_snapshot: input.fundamental_snapshot,
            trend_snapshot: input.trend_snapshot,
            sentiment_snapshot: input.sentiment_snapshot,
            decision_snapshot: input.decision_snapshot,
            broker_order_request: input.broker_order_request,
            broker_order_ack: input.broker_order_ack,
            summary: input.summary,
            created_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        }
    }

    #[tokio::test]
    async fn service_creates_lists_and_gets_records() {
        let repository = Arc::new(FakeRepository::default());
        let service = DecisionRecordService::new(repository);
        let plan_id = Uuid::from_u128(7);
        let created = service.create(input(plan_id)).await.unwrap();

        assert_eq!(created.plan_id, plan_id);
        assert_eq!(created.execution_snapshot, json!({"status": "due"}));
        assert_eq!(
            service.list_by_plan(plan_id).await.unwrap(),
            vec![created.clone()]
        );
        assert_eq!(service.get(created.id).await.unwrap(), created);
    }

    #[tokio::test]
    async fn service_maps_repository_not_found() {
        let service = DecisionRecordService::new(Arc::new(FakeRepository::default()));

        assert_eq!(
            service.get(Uuid::from_u128(404)).await,
            Err(DecisionRecordApplicationError::NotFound)
        );
    }
}

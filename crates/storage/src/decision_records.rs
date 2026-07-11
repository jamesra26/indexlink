//! PostgreSQL adapter for decision record repository port.

use async_trait::async_trait;
use decision_records::{
    CreateDecisionRecord, DecisionRecord, DecisionRecordRepository, DecisionRecordRepositoryError,
};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

const RECORD_COLUMNS: &str = "id::text AS id, plan_id::text AS plan_id, symbol, currency, \
    execution_status, planned_contribution, execution_snapshot, fundamental_snapshot, \
    trend_snapshot, sentiment_snapshot, decision_snapshot, broker_order_request, \
    broker_order_ack, summary, (EXTRACT(EPOCH FROM created_at) * 1000000)::bigint AS \
    created_at_micros";

/// PostgreSQL implementation of [`DecisionRecordRepository`].
#[derive(Clone, Debug)]
pub struct PostgresDecisionRecordRepository {
    pool: PgPool,
}

impl PostgresDecisionRecordRepository {
    /// Build a repository from an existing PostgreSQL pool.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DecisionRecordRepository for PostgresDecisionRecordRepository {
    /// Insert one audit-ready decision record snapshot.
    async fn create(
        &self,
        input: CreateDecisionRecord,
    ) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
        let row = sqlx::query(&format!(
            "INSERT INTO decision_records \
             (plan_id, symbol, currency, execution_status, planned_contribution, \
              execution_snapshot, fundamental_snapshot, trend_snapshot, sentiment_snapshot, \
              decision_snapshot, broker_order_request, broker_order_ack, summary) \
             VALUES ($1::uuid, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, \
                     $9::jsonb, $10::jsonb, $11::jsonb, $12::jsonb, $13) \
             RETURNING {RECORD_COLUMNS}"
        ))
        .bind(input.plan_id.to_string())
        .bind(input.symbol)
        .bind(input.currency)
        .bind(input.execution_status)
        .bind(input.planned_contribution)
        .bind(input.execution_snapshot)
        .bind(input.fundamental_snapshot)
        .bind(input.trend_snapshot)
        .bind(input.sentiment_snapshot)
        .bind(input.decision_snapshot)
        .bind(input.broker_order_request)
        .bind(input.broker_order_ack)
        .bind(input.summary)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        record_from_row(row)
    }

    /// List records for a plan from newest to oldest.
    async fn list_by_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<Vec<DecisionRecord>, DecisionRecordRepositoryError> {
        let rows = sqlx::query(&format!(
            "SELECT {RECORD_COLUMNS} FROM decision_records \
             WHERE plan_id = $1::uuid ORDER BY created_at DESC, id DESC"
        ))
        .bind(plan_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(record_from_row).collect()
    }

    /// Fetch one record by ID.
    async fn get(&self, id: Uuid) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
        let row = sqlx::query(&format!(
            "SELECT {RECORD_COLUMNS} FROM decision_records WHERE id = $1::uuid"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?
        .ok_or(DecisionRecordRepositoryError::NotFound)?;

        record_from_row(row)
    }
}

fn record_from_row(row: PgRow) -> Result<DecisionRecord, DecisionRecordRepositoryError> {
    Ok(DecisionRecord {
        id: parse_uuid(row.try_get("id").map_err(map_sqlx_error)?)?,
        plan_id: parse_uuid(row.try_get("plan_id").map_err(map_sqlx_error)?)?,
        symbol: row.try_get("symbol").map_err(map_sqlx_error)?,
        currency: row.try_get("currency").map_err(map_sqlx_error)?,
        execution_status: row.try_get("execution_status").map_err(map_sqlx_error)?,
        planned_contribution: row
            .try_get("planned_contribution")
            .map_err(map_sqlx_error)?,
        execution_snapshot: row
            .try_get::<Value, _>("execution_snapshot")
            .map_err(map_sqlx_error)?,
        fundamental_snapshot: row
            .try_get::<Value, _>("fundamental_snapshot")
            .map_err(map_sqlx_error)?,
        trend_snapshot: row
            .try_get::<Value, _>("trend_snapshot")
            .map_err(map_sqlx_error)?,
        sentiment_snapshot: row
            .try_get::<Option<Value>, _>("sentiment_snapshot")
            .map_err(map_sqlx_error)?,
        decision_snapshot: row
            .try_get::<Value, _>("decision_snapshot")
            .map_err(map_sqlx_error)?,
        broker_order_request: row
            .try_get::<Option<Value>, _>("broker_order_request")
            .map_err(map_sqlx_error)?,
        broker_order_ack: row
            .try_get::<Option<Value>, _>("broker_order_ack")
            .map_err(map_sqlx_error)?,
        summary: row.try_get("summary").map_err(map_sqlx_error)?,
        created_at: parse_micros(row.try_get("created_at_micros").map_err(map_sqlx_error)?)?,
    })
}

fn parse_uuid(value: String) -> Result<Uuid, DecisionRecordRepositoryError> {
    value
        .parse()
        .map_err(|_| DecisionRecordRepositoryError::Unavailable)
}

fn parse_micros(value: i64) -> Result<OffsetDateTime, DecisionRecordRepositoryError> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(value) * 1000)
        .map_err(|_| DecisionRecordRepositoryError::Unavailable)
}

fn map_sqlx_error(error: sqlx::Error) -> DecisionRecordRepositoryError {
    match error {
        sqlx::Error::RowNotFound => DecisionRecordRepositoryError::NotFound,
        _ => DecisionRecordRepositoryError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_sqlx_errors_to_safe_repository_errors() {
        assert_eq!(
            map_sqlx_error(sqlx::Error::RowNotFound),
            DecisionRecordRepositoryError::NotFound
        );
        assert_eq!(
            map_sqlx_error(sqlx::Error::PoolClosed),
            DecisionRecordRepositoryError::Unavailable
        );
    }

    #[test]
    fn rejects_invalid_database_uuid_snapshot() {
        assert_eq!(
            parse_uuid("not-a-uuid".to_owned()),
            Err(DecisionRecordRepositoryError::Unavailable)
        );
    }
}

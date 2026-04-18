//! PostgreSQL-backed durable outbox and process-manager offset storage.

use std::time::Duration;

use es_core::TenantId;
use es_outbox::{
    CommittedEventReader, DispatchBatchLimit, MessageKey, NewOutboxMessage, OutboxError,
    OutboxMessage, OutboxResult, OutboxStatus, OutboxStore, ProcessEvent, ProcessManagerName,
    ProcessManagerOffsetStore, RetryPolicy, RetryScheduleOutcome, SourceEventRef, Topic, WorkerId,
};
use futures::future::BoxFuture;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{PostgresEventStore, StoreError, StoreResult, StoredEvent};

/// PostgreSQL outbox repository.
#[derive(Clone, Debug)]
pub struct PostgresOutboxStore {
    pool: sqlx::PgPool,
}

impl PostgresOutboxStore {
    /// Creates an outbox repository backed by the provided PostgreSQL pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Returns the underlying PostgreSQL pool.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Inserts a pending outbox message for a committed source event.
    pub async fn insert_outbox_message(
        &self,
        tenant_id: &TenantId,
        message: &NewOutboxMessage,
        source_global_position: i64,
    ) -> StoreResult<OutboxMessage> {
        let row = sqlx::query_as::<_, OutboxRow>(
            r#"
            INSERT INTO outbox_messages (
                outbox_id,
                tenant_id,
                source_event_id,
                source_global_position,
                topic,
                message_key,
                payload,
                metadata,
                status
            )
            SELECT $1, $2, $3, $4, $5, $6, $7, $8, 'pending'
            WHERE EXISTS (
                SELECT 1
                FROM events
                WHERE tenant_id = $2
                  AND event_id = $3
                  AND global_position = $4
            )
            RETURNING *
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(tenant_id.as_str())
        .bind(message.source.event_id())
        .bind(source_global_position)
        .bind(message.topic.as_str())
        .bind(message.message_key.as_str())
        .bind(&message.payload)
        .bind(&message.metadata)
        .fetch_one(&self.pool)
        .await?;

        outbox_message_from_row(row)
    }

    /// Claims due pending outbox messages for one worker.
    pub async fn claim_pending(
        &self,
        tenant_id: &TenantId,
        worker_id: &WorkerId,
        limit: DispatchBatchLimit,
        lock_for: Duration,
    ) -> StoreResult<Vec<OutboxMessage>> {
        let lock_seconds = i64::try_from(lock_for.as_secs()).unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, OutboxRow>(
            r#"
            WITH claimed AS (
                SELECT outbox_id
                FROM outbox_messages
                WHERE tenant_id = $1
                  AND (
                      (status = 'pending' AND available_at <= now())
                      OR (status = 'publishing' AND locked_until <= now())
                  )
                ORDER BY source_global_position, outbox_id
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            UPDATE outbox_messages AS o
            SET status = 'publishing',
                locked_by = $3,
                locked_until = now() + ($4 * INTERVAL '1 second'),
                attempts = attempts + 1,
                updated_at = now()
            FROM claimed
            WHERE o.outbox_id = claimed.outbox_id
              AND o.tenant_id = $1
            RETURNING o.*
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(limit.value())
        .bind(worker_id.as_str())
        .bind(lock_seconds)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(outbox_message_from_row).collect()
    }

    /// Marks a publishing message as published.
    pub async fn mark_published(
        &self,
        tenant_id: &TenantId,
        outbox_id: Uuid,
        worker_id: &WorkerId,
    ) -> StoreResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE outbox_messages
            SET status = 'published',
                published_at = now(),
                locked_by = NULL,
                locked_until = NULL,
                last_error = NULL,
                updated_at = now()
            WHERE tenant_id = $1
              AND outbox_id = $2
              AND status = 'publishing'
              AND locked_by = $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(outbox_id)
        .bind(worker_id.as_str())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::Outbox {
                message: format!(
                    "outbox message {outbox_id} is not publishing for worker {}",
                    worker_id.as_str()
                ),
            });
        }

        Ok(())
    }

    /// Schedules another dispatch attempt or transitions the message to failed.
    pub async fn schedule_retry(
        &self,
        tenant_id: &TenantId,
        outbox_id: Uuid,
        worker_id: &WorkerId,
        error: &str,
        retry_policy: RetryPolicy,
    ) -> StoreResult<RetryScheduleOutcome> {
        let status = sqlx::query_scalar::<_, String>(
            r#"
            UPDATE outbox_messages
            SET status = CASE
                    WHEN attempts >= $4 THEN 'failed'
                    ELSE 'pending'
                END,
                available_at = now(),
                locked_by = NULL,
                locked_until = NULL,
                last_error = $5,
                updated_at = now()
            WHERE tenant_id = $1
              AND outbox_id = $2
              AND status = 'publishing'
              AND locked_by = $3
            RETURNING status
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(outbox_id)
        .bind(worker_id.as_str())
        .bind(retry_policy.max_attempts())
        .bind(error)
        .fetch_optional(&self.pool)
        .await?;

        let Some(status) = status else {
            return Err(StoreError::Outbox {
                message: format!(
                    "outbox message {outbox_id} is not publishing for worker {}",
                    worker_id.as_str()
                ),
            });
        };

        match status.as_str() {
            "pending" => Ok(RetryScheduleOutcome::RetryScheduled),
            "failed" => Ok(RetryScheduleOutcome::Failed),
            _ => Err(StoreError::Outbox {
                message: format!("unexpected retry status: {status}"),
            }),
        }
    }

    /// Marks a message as failed without scheduling another retry.
    pub async fn mark_failed(
        &self,
        tenant_id: &TenantId,
        outbox_id: Uuid,
        worker_id: &WorkerId,
        error: &str,
    ) -> StoreResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE outbox_messages
            SET status = 'failed',
                locked_by = NULL,
                locked_until = NULL,
                last_error = $4,
                updated_at = now()
            WHERE tenant_id = $1
              AND outbox_id = $2
              AND status = 'publishing'
              AND locked_by = $3
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(outbox_id)
        .bind(worker_id.as_str())
        .bind(error)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::Outbox {
                message: format!(
                    "outbox message {outbox_id} is not publishing for worker {}",
                    worker_id.as_str()
                ),
            });
        }

        Ok(())
    }

    /// Loads the saved tenant-scoped process-manager offset.
    pub async fn process_manager_offset(
        &self,
        tenant_id: &TenantId,
        name: &ProcessManagerName,
    ) -> StoreResult<Option<i64>> {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT last_global_position
            FROM process_manager_offsets
            WHERE tenant_id = $1
              AND process_manager_name = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(name.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(StoreError::from)
    }

    /// Advances a tenant-scoped process-manager offset monotonically.
    pub async fn advance_process_manager_offset(
        &self,
        tenant_id: &TenantId,
        name: &ProcessManagerName,
        last_global_position: i64,
    ) -> StoreResult<()> {
        sqlx::query(
            r#"
            INSERT INTO process_manager_offsets (
                tenant_id,
                process_manager_name,
                last_global_position
            )
            VALUES ($1, $2, $3)
            ON CONFLICT (tenant_id, process_manager_name) DO UPDATE
            SET last_global_position = GREATEST(
                    process_manager_offsets.last_global_position,
                    EXCLUDED.last_global_position
                ),
                updated_at = now()
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(name.as_str())
        .bind(last_global_position)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

impl OutboxStore for PostgresOutboxStore {
    fn claim_pending(
        &self,
        tenant_id: TenantId,
        worker_id: WorkerId,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<OutboxMessage>>> {
        Box::pin(async move {
            PostgresOutboxStore::claim_pending(
                self,
                &tenant_id,
                &worker_id,
                limit,
                Duration::from_secs(30),
            )
            .await
            .map_err(outbox_store_error)
        })
    }

    fn mark_published(
        &self,
        tenant_id: TenantId,
        outbox_id: Uuid,
        worker_id: WorkerId,
    ) -> BoxFuture<'_, OutboxResult<()>> {
        Box::pin(async move {
            PostgresOutboxStore::mark_published(self, &tenant_id, outbox_id, &worker_id)
                .await
                .map_err(outbox_store_error)
        })
    }

    fn schedule_retry(
        &self,
        tenant_id: TenantId,
        outbox_id: Uuid,
        worker_id: WorkerId,
        error: String,
        retry_policy: RetryPolicy,
    ) -> BoxFuture<'_, OutboxResult<RetryScheduleOutcome>> {
        Box::pin(async move {
            PostgresOutboxStore::schedule_retry(
                self,
                &tenant_id,
                outbox_id,
                &worker_id,
                &error,
                retry_policy,
            )
            .await
            .map_err(outbox_store_error)
        })
    }
}

impl ProcessManagerOffsetStore for PostgresOutboxStore {
    fn process_manager_offset(
        &self,
        tenant_id: TenantId,
        name: ProcessManagerName,
    ) -> BoxFuture<'_, OutboxResult<Option<i64>>> {
        Box::pin(async move {
            PostgresOutboxStore::process_manager_offset(self, &tenant_id, &name)
                .await
                .map_err(outbox_store_error)
        })
    }

    fn advance_process_manager_offset(
        &self,
        tenant_id: TenantId,
        name: ProcessManagerName,
        last_global_position: i64,
    ) -> BoxFuture<'_, OutboxResult<()>> {
        Box::pin(async move {
            PostgresOutboxStore::advance_process_manager_offset(
                self,
                &tenant_id,
                &name,
                last_global_position,
            )
            .await
            .map_err(outbox_store_error)
        })
    }
}

impl CommittedEventReader for PostgresEventStore {
    fn read_global(
        &self,
        tenant_id: TenantId,
        after_global_position: i64,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<ProcessEvent>>> {
        Box::pin(async move {
            PostgresEventStore::read_global(self, &tenant_id, after_global_position, limit.value())
                .await
                .map_err(outbox_store_error)
                .map(|events| events.into_iter().map(ProcessEvent::from).collect())
        })
    }
}

impl From<StoredEvent> for ProcessEvent {
    fn from(event: StoredEvent) -> Self {
        Self {
            global_position: event.global_position,
            event_id: event.event_id,
            event_type: event.event_type,
            schema_version: event.schema_version,
            payload: event.payload,
            metadata: event.metadata,
            tenant_id: event.tenant_id,
            command_id: event.command_id,
            correlation_id: event.correlation_id,
            causation_id: event.causation_id,
        }
    }
}

#[derive(sqlx::FromRow)]
struct OutboxRow {
    outbox_id: Uuid,
    tenant_id: String,
    source_event_id: Uuid,
    source_global_position: i64,
    topic: String,
    message_key: String,
    payload: Value,
    metadata: Value,
    status: String,
    attempts: i32,
    available_at: OffsetDateTime,
    locked_by: Option<String>,
    locked_until: Option<OffsetDateTime>,
    published_at: Option<OffsetDateTime>,
    last_error: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

fn outbox_message_from_row(row: OutboxRow) -> StoreResult<OutboxMessage> {
    Ok(OutboxMessage {
        outbox_id: row.outbox_id,
        tenant_id: TenantId::new(row.tenant_id).map_err(outbox_mapping_error)?,
        source: SourceEventRef::new(row.source_event_id, row.source_global_position)
            .map_err(outbox_mapping_error)?,
        topic: Topic::new(row.topic).map_err(outbox_mapping_error)?,
        message_key: MessageKey::new(row.message_key).map_err(outbox_mapping_error)?,
        payload: row.payload,
        metadata: row.metadata,
        status: OutboxStatus::try_from(row.status.as_str()).map_err(outbox_mapping_error)?,
        attempts: row.attempts,
        available_at: row.available_at,
        locked_by: row
            .locked_by
            .map(WorkerId::new)
            .transpose()
            .map_err(outbox_mapping_error)?,
        locked_until: row.locked_until,
        published_at: row.published_at,
        last_error: row.last_error,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn outbox_mapping_error(error: impl std::fmt::Display) -> StoreError {
    StoreError::Outbox {
        message: error.to_string(),
    }
}

fn outbox_store_error(error: StoreError) -> OutboxError {
    OutboxError::Store {
        message: error.to_string(),
    }
}

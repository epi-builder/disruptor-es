use crate::{
    AppendOutcome, AppendRequest, CommandReplayRecord, RehydrationBatch, SaveSnapshotRequest,
    SnapshotRecord, StoreError, StoreResult, StoredEvent, rehydrate, sql,
};
use metrics::{counter, histogram};
use tracing::info_span;

/// PostgreSQL-backed durable event store.
#[derive(Clone, Debug)]
pub struct PostgresEventStore {
    pool: sqlx::PgPool,
}

impl PostgresEventStore {
    /// Creates a store backed by the supplied PostgreSQL connection pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Returns the underlying PostgreSQL connection pool.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Appends events to a stream.
    pub async fn append(&self, request: AppendRequest) -> StoreResult<AppendOutcome> {
        if request.events.is_empty() {
            return Err(StoreError::EmptyAppend);
        }

        let started_at = std::time::Instant::now();
        let span = info_span!(
            "event_store.append",
            command_id = %request.command_metadata.command_id,
            correlation_id = %request.command_metadata.correlation_id,
            causation_id = ?request.command_metadata.causation_id,
            tenant_id = %request.command_metadata.tenant_id.as_str(),
            stream_id = %request.stream_id.as_str(),
            global_position = tracing::field::Empty,
        );
        let _entered = span.enter();

        let outcome = sql::append(&self.pool, request).await;
        match &outcome {
            Ok(AppendOutcome::Committed(committed)) => {
                if let Some(global_position) = committed.global_positions.last() {
                    span.record("global_position", global_position);
                }
                histogram!("es_append_latency_seconds", "outcome" => "committed")
                    .record(started_at.elapsed().as_secs_f64());
            }
            Ok(AppendOutcome::Duplicate(committed)) => {
                if let Some(global_position) = committed.global_positions.last() {
                    span.record("global_position", global_position);
                }
                counter!("es_dedupe_hits_total").increment(1);
                histogram!("es_append_latency_seconds", "outcome" => "duplicate")
                    .record(started_at.elapsed().as_secs_f64());
            }
            Err(StoreError::StreamConflict { .. }) => {
                counter!("es_occ_conflicts_total").increment(1);
                histogram!("es_append_latency_seconds", "outcome" => "conflict")
                    .record(started_at.elapsed().as_secs_f64());
            }
            Err(_) => {
                histogram!("es_append_latency_seconds", "outcome" => "error")
                    .record(started_at.elapsed().as_secs_f64());
            }
        }
        outcome
    }

    /// Reads stream events after an optional stream revision.
    pub async fn read_stream(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
        after_revision: Option<es_core::StreamRevision>,
        limit: i64,
    ) -> StoreResult<Vec<StoredEvent>> {
        let after_revision = after_revision.map(|revision| revision.value()).unwrap_or(0);
        let after_revision = i64::try_from(after_revision)
            .map_err(|_| StoreError::InvalidStoredRevision { value: i64::MAX })?;

        sql::read_stream_after(&self.pool, tenant_id, stream_id, after_revision, limit).await
    }

    /// Reads events by durable global position.
    pub async fn read_global(
        &self,
        tenant_id: &es_core::TenantId,
        after_global_position: i64,
        limit: i64,
    ) -> StoreResult<Vec<StoredEvent>> {
        sql::read_global(&self.pool, tenant_id, after_global_position, limit).await
    }

    /// Looks up a durable typed command replay record by tenant and idempotency key.
    pub async fn lookup_command_replay(
        &self,
        tenant_id: &es_core::TenantId,
        idempotency_key: &str,
    ) -> StoreResult<Option<CommandReplayRecord>> {
        sql::lookup_command_replay(&self.pool, tenant_id, idempotency_key).await
    }

    /// Saves a stream snapshot.
    pub async fn save_snapshot(&self, request: SaveSnapshotRequest) -> StoreResult<SnapshotRecord> {
        sql::save_snapshot(&self.pool, request).await
    }

    /// Loads the latest snapshot for a stream.
    pub async fn load_latest_snapshot(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> StoreResult<Option<SnapshotRecord>> {
        sql::load_latest_snapshot(&self.pool, tenant_id, stream_id).await
    }

    /// Loads the latest snapshot and subsequent stream events.
    pub async fn load_rehydration(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> StoreResult<RehydrationBatch> {
        rehydrate::load_rehydration(&self.pool, tenant_id, stream_id).await
    }
}

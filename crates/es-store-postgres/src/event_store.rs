use crate::{
    AppendOutcome, AppendRequest, RehydrationBatch, SaveSnapshotRequest, SnapshotRecord,
    StoreError, StoreResult, StoredEvent, rehydrate, sql,
};

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

        sql::append(&self.pool, request).await
    }

    /// Reads stream events after an optional stream revision.
    pub async fn read_stream(
        &self,
        _tenant_id: &es_core::TenantId,
        _stream_id: &es_core::StreamId,
        _after_revision: Option<es_core::StreamRevision>,
        _limit: i64,
    ) -> StoreResult<Vec<StoredEvent>> {
        pending_sql()
    }

    /// Reads events by durable global position.
    pub async fn read_global(
        &self,
        _tenant_id: &es_core::TenantId,
        _after_global_position: i64,
        _limit: i64,
    ) -> StoreResult<Vec<StoredEvent>> {
        pending_sql()
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

fn pending_sql<T>() -> StoreResult<T> {
    Err(StoreError::Database(sqlx::Error::RowNotFound))
}

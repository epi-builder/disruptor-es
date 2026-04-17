use futures::future::BoxFuture;

/// Runtime-facing event-store boundary.
pub trait RuntimeEventStore: Clone + Send + Sync + 'static {
    /// Appends events to durable storage.
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>>;

    /// Loads snapshot plus events for aggregate rehydration.
    fn load_rehydration(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>>;
}

/// PostgreSQL-backed runtime event-store adapter.
#[derive(Clone, Debug)]
pub struct PostgresRuntimeEventStore {
    inner: es_store_postgres::PostgresEventStore,
}

impl PostgresRuntimeEventStore {
    /// Creates a runtime store adapter from the durable PostgreSQL event store.
    pub fn new(inner: es_store_postgres::PostgresEventStore) -> Self {
        Self { inner }
    }

    /// Returns the wrapped PostgreSQL event store.
    pub fn inner(&self) -> &es_store_postgres::PostgresEventStore {
        &self.inner
    }
}

impl RuntimeEventStore for PostgresRuntimeEventStore {
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>> {
        Box::pin(async move { self.inner.append(request).await })
    }

    fn load_rehydration(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>> {
        let tenant_id = tenant_id.clone();
        let stream_id = stream_id.clone();

        Box::pin(async move { self.inner.load_rehydration(&tenant_id, &stream_id).await })
    }
}

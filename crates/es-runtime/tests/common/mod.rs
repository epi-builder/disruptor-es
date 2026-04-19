use std::sync::{Arc, Mutex};

use es_runtime::RuntimeEventStore;
use futures::future::BoxFuture;

#[derive(Clone)]
#[allow(dead_code)]
enum FakeResponse {
    Outcome(es_store_postgres::AppendOutcome),
    Error(Arc<Mutex<Option<es_store_postgres::StoreError>>>),
}

/// Test-only runtime event store that records append requests.
#[derive(Clone)]
pub struct FakeRuntimeEventStore {
    append_requests: Arc<Mutex<Vec<es_store_postgres::AppendRequest>>>,
    response: FakeResponse,
}

impl Default for FakeRuntimeEventStore {
    fn default() -> Self {
        Self::with_outcome(es_store_postgres::AppendOutcome::Committed(
            es_store_postgres::CommittedAppend {
                stream_id: es_core::StreamId::new("default-stream").expect("stream id"),
                first_revision: es_core::StreamRevision::new(1),
                last_revision: es_core::StreamRevision::new(1),
                global_positions: vec![1],
                event_ids: vec![uuid::Uuid::from_u128(1)],
            },
        ))
    }
}

impl FakeRuntimeEventStore {
    /// Creates a fake store that returns the supplied append outcome.
    pub fn with_outcome(outcome: es_store_postgres::AppendOutcome) -> Self {
        Self {
            append_requests: Arc::new(Mutex::new(Vec::new())),
            response: FakeResponse::Outcome(outcome),
        }
    }

    /// Creates a fake store that returns the supplied store error once.
    #[allow(dead_code)]
    pub fn with_error(error: es_store_postgres::StoreError) -> Self {
        Self {
            append_requests: Arc::new(Mutex::new(Vec::new())),
            response: FakeResponse::Error(Arc::new(Mutex::new(Some(error)))),
        }
    }

    /// Returns the number of append requests recorded by the fake.
    pub fn appended_len(&self) -> usize {
        self.append_requests.lock().expect("append requests").len()
    }

    /// Returns all append requests recorded by the fake.
    pub fn append_requests(&self) -> Vec<es_store_postgres::AppendRequest> {
        self.append_requests
            .lock()
            .expect("append requests")
            .clone()
    }
}

impl RuntimeEventStore for FakeRuntimeEventStore {
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>> {
        self.append_requests
            .lock()
            .expect("append requests")
            .push(request);

        let result = match &self.response {
            FakeResponse::Outcome(outcome) => Ok(outcome.clone()),
            FakeResponse::Error(error) => Err(error.lock().expect("store error").take().unwrap_or(
                es_store_postgres::StoreError::DedupeConflict {
                    tenant_id: "fake".to_owned(),
                    idempotency_key: "already-consumed".to_owned(),
                },
            )),
        };

        Box::pin(async move { result })
    }

    fn load_rehydration(
        &self,
        _tenant_id: &es_core::TenantId,
        _stream_id: &es_core::StreamId,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>> {
        let result = match &self.response {
            FakeResponse::Outcome(_) => Ok(es_store_postgres::RehydrationBatch {
                snapshot: None,
                events: Vec::new(),
            }),
            FakeResponse::Error(error) => Err(error.lock().expect("store error").take().unwrap_or(
                es_store_postgres::StoreError::DedupeConflict {
                    tenant_id: "fake".to_owned(),
                    idempotency_key: "already-consumed".to_owned(),
                },
            )),
        };

        Box::pin(async move { result })
    }

    fn lookup_command_replay(
        &self,
        _tenant_id: &es_core::TenantId,
        _idempotency_key: &str,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<Option<es_store_postgres::CommandReplayRecord>>>
    {
        Box::pin(async move { Ok(None) })
    }
}

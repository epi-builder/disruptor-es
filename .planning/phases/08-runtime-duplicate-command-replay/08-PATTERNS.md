# Phase 8: Runtime Duplicate Command Replay - Pattern Map

**Mapped:** 2026-04-19
**Files analyzed:** 16 likely new/modified files
**Analogs found:** 16 / 16

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/es-runtime/src/cache.rs` | utility | request-response | `crates/es-runtime/src/cache.rs` | exact |
| `crates/es-runtime/src/command.rs` | model/codec | transform | `crates/es-runtime/src/command.rs` | exact |
| `crates/es-runtime/src/store.rs` | service/port | request-response | `crates/es-runtime/src/store.rs` | exact |
| `crates/es-runtime/src/shard.rs` | service/runtime | request-response | `crates/es-runtime/src/shard.rs` | exact |
| `crates/es-runtime/src/lib.rs` | config/export | transform | `crates/es-runtime/src/lib.rs` | exact |
| `crates/es-runtime/tests/runtime_flow.rs` | test | request-response | `crates/es-runtime/tests/runtime_flow.rs` | exact |
| `crates/es-runtime/tests/common/mod.rs` | test utility | request-response | `crates/es-runtime/tests/runtime_flow.rs` | role-match |
| `crates/es-store-postgres/src/models.rs` | model | CRUD | `crates/es-store-postgres/src/models.rs` | exact |
| `crates/es-store-postgres/src/sql.rs` | service/repository | CRUD | `crates/es-store-postgres/src/sql.rs` | exact |
| `crates/es-store-postgres/src/event_store.rs` | service/facade | request-response | `crates/es-store-postgres/src/event_store.rs` | exact |
| `crates/es-store-postgres/src/error.rs` | model/error | transform | `crates/es-store-postgres/src/error.rs` | exact |
| `crates/es-store-postgres/src/lib.rs` | config/export | transform | `crates/es-store-postgres/src/lib.rs` | exact |
| `crates/es-store-postgres/tests/dedupe.rs` | test | CRUD | `crates/es-store-postgres/tests/dedupe.rs` | exact |
| `crates/adapter-http/tests/commerce_api.rs` | test | request-response | `crates/adapter-http/tests/commerce_api.rs` | exact |
| `crates/app/src/commerce_process_manager.rs` | service/test | event-driven | `crates/app/src/commerce_process_manager.rs` | exact |
| `crates/app/src/stress.rs` | adapter/wrapper | request-response | `crates/app/src/stress.rs` | exact |

## Pattern Assignments

### `crates/es-runtime/src/cache.rs` (utility, request-response)

**Analog:** `crates/es-runtime/src/cache.rs`

**Imports pattern** (lines 1-5):
```rust
use std::collections::HashMap;

use es_core::{StreamId, TenantId};
use es_kernel::Aggregate;
use es_store_postgres::CommittedAppend;
```

**Shard-owned cache pattern** (lines 7-34):
```rust
pub struct AggregateCache<A: Aggregate> {
    states: HashMap<StreamId, A::State>,
}

impl<A: Aggregate> AggregateCache<A> {
    pub fn get_or_default(&mut self, stream_id: &StreamId) -> A::State {
        self.states.entry(stream_id.clone()).or_default().clone()
    }

    pub fn commit_state(&mut self, stream_id: StreamId, state: A::State) {
        self.states.insert(stream_id, state);
    }
}
```

**Dedupe key/record pattern** (lines 52-66):
```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DedupeKey {
    pub tenant_id: TenantId,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DedupeRecord {
    pub append: CommittedAppend,
}
```

**Phase 8 copy rule:** Keep `DedupeKey` tenant-scoped. Extend `DedupeRecord` to carry the replayable typed command result, not just `CommittedAppend`. Preserve the owned `HashMap` inside `ShardState`; do not introduce shared global mutable dedupe state.

---

### `crates/es-runtime/src/command.rs` (model/codec, transform)

**Analog:** `crates/es-runtime/src/command.rs`

**Envelope/outcome pattern** (lines 5-23, 57-69):
```rust
pub type CommandReply<R> = tokio::sync::oneshot::Sender<RuntimeResult<CommandOutcome<R>>>;

pub struct CommandEnvelope<A: Aggregate> {
    pub command: A::Command,
    pub metadata: es_core::CommandMetadata,
    pub idempotency_key: String,
    pub stream_id: es_core::StreamId,
    pub partition_key: es_core::PartitionKey,
    pub expected_revision: es_core::ExpectedRevision,
    pub reply: CommandReply<A::Reply>,
}

pub struct CommandOutcome<R> {
    pub reply: R,
    pub append: es_store_postgres::CommittedAppend,
}
```

**Runtime codec boundary** (lines 72-89):
```rust
pub trait RuntimeEventCodec<A: Aggregate>: Clone + Send + Sync + 'static {
    fn encode(
        &self,
        event: &A::Event,
        metadata: &es_core::CommandMetadata,
    ) -> RuntimeResult<es_store_postgres::NewEvent>;

    fn decode(&self, stored: &es_store_postgres::StoredEvent) -> RuntimeResult<A::Event>;

    fn decode_snapshot(
        &self,
        snapshot: &es_store_postgres::SnapshotRecord,
    ) -> RuntimeResult<A::State>;
}
```

**Phase 8 copy rule:** Put reply serialization/deserialization at the runtime codec boundary because only runtime knows `A::Reply`. Add methods or a companion trait that encodes `A::Reply` into the store duplicate payload and decodes it back into `CommandOutcome<A::Reply>`.

---

### `crates/es-runtime/src/store.rs` (service/port, request-response)

**Analog:** `crates/es-runtime/src/store.rs`

**Runtime store port pattern** (lines 1-17):
```rust
use futures::future::BoxFuture;

pub trait RuntimeEventStore: Clone + Send + Sync + 'static {
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>>;

    fn load_rehydration(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>>;
}
```

**Postgres adapter pattern** (lines 37-54):
```rust
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
```

**Phase 8 copy rule:** Add a durable pre-decision dedupe lookup method to this trait using the same `BoxFuture` shape. Update all implementers, including `PostgresRuntimeEventStore`, fake stores in tests, and `MeasuredRuntimeEventStore` in `crates/app/src/stress.rs`.

---

### `crates/es-runtime/src/shard.rs` (service/runtime, request-response)

**Analog:** `crates/es-runtime/src/shard.rs`

**Imports and state pattern** (lines 1-14, 68-83):
```rust
use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use crate::{
    AggregateCache, CommandEnvelope, CommandOutcome, DedupeCache, DedupeKey, DedupeRecord,
    DisruptorPath, RoutedCommand, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
    RuntimeResult, ShardId,
};

pub struct ShardState<A: Aggregate> {
    shard_id: ShardId,
    cache: AggregateCache<A>,
    dedupe: DedupeCache,
    handoffs: VecDeque<ShardHandoff<A>>,
}
```

**Current ordering to change** (lines 171-188):
```rust
let current_state = if let Some(cached) = self.cache.get(&envelope.stream_id) {
    cached.clone()
} else {
    match rehydrate_state(store, codec, &envelope).await {
        Ok(rehydrated) => {
            self.cache
                .commit_state(envelope.stream_id.clone(), rehydrated.clone());
            rehydrated
        }
        Err(error) => {
            let _ = envelope.reply.send(Err(error));
            return Ok(true);
        }
    }
};

let decision_started_at = Instant::now();
let decision = match A::decide(&current_state, envelope.command, &envelope.metadata) {
```

**Commit-gated cache pattern** (lines 245-290):
```rust
match store.append(append_request).await {
    Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
        let mut staged_state = current_state;
        for event in &decision.events {
            A::apply(&mut staged_state, event);
        }
        self.cache
            .commit_state(envelope.stream_id.clone(), staged_state);
        self.dedupe.record(
            DedupeKey {
                tenant_id: envelope.metadata.tenant_id.clone(),
                idempotency_key: envelope.idempotency_key.clone(),
            },
            DedupeRecord {
                append: committed.clone(),
            },
        );
        let _ = envelope
            .reply
            .send(Ok(CommandOutcome::new(decision.reply, committed)));
    }
    Ok(es_store_postgres::AppendOutcome::Duplicate(committed)) => {
        self.dedupe.record(
            DedupeKey {
                tenant_id: envelope.metadata.tenant_id.clone(),
                idempotency_key: envelope.idempotency_key.clone(),
            },
            DedupeRecord {
                append: committed.clone(),
            },
        );
        let _ = envelope
            .reply
            .send(Ok(CommandOutcome::new(decision.reply, committed)));
    }
}
```

**Phase 8 copy rule:** Move dedupe lookup before line 171 rehydration and line 188 `A::decide`. On cache hit or durable hit, decode the original stored outcome, send it, record duplicate latency/metrics, and return without rehydration, decision, event encoding, append, or aggregate cache mutation. Continue to populate caches only after committed append or durable duplicate lookup.

---

### `crates/es-store-postgres/src/models.rs` (model, CRUD)

**Analog:** `crates/es-store-postgres/src/models.rs`

**Append request validation pattern** (lines 66-139):
```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AppendRequest {
    pub stream_id: StreamId,
    pub expected_revision: ExpectedRevision,
    pub command_metadata: CommandMetadata,
    pub idempotency_key: String,
    pub events: Vec<NewEvent>,
    pub outbox_messages: Vec<NewOutboxMessage>,
}

impl AppendRequest {
    pub fn new_with_outbox(
        stream_id: StreamId,
        expected_revision: ExpectedRevision,
        command_metadata: CommandMetadata,
        idempotency_key: impl Into<String>,
        events: Vec<NewEvent>,
        outbox_messages: Vec<NewOutboxMessage>,
    ) -> StoreResult<Self> {
        if events.is_empty() {
            return Err(StoreError::EmptyAppend);
        }

        let idempotency_key = idempotency_key.into();
        if idempotency_key.is_empty() {
            return Err(StoreError::InvalidIdempotencyKey);
        }
```

**Append outcome pattern** (lines 142-164):
```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedAppend {
    pub stream_id: StreamId,
    pub first_revision: StreamRevision,
    pub last_revision: StreamRevision,
    pub global_positions: Vec<i64>,
    pub event_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AppendOutcome {
    Committed(CommittedAppend),
    Duplicate(CommittedAppend),
}
```

**Phase 8 copy rule:** Add a store DTO for the full duplicate result, likely `{ append: CommittedAppend, reply_payload: serde_json::Value }`. Validate empty tenant/idempotency keys where new lookup request models are introduced. Keep the storage model generic; it should not depend on commerce or runtime aggregate types.

---

### `crates/es-store-postgres/src/sql.rs` (service/repository, CRUD)

**Analog:** `crates/es-store-postgres/src/sql.rs`

**Transaction ordering pattern** (lines 13-21, 23-70):
```rust
pub(crate) async fn append(pool: &PgPool, request: AppendRequest) -> StoreResult<AppendOutcome> {
    let mut tx = pool.begin().await?;

    acquire_dedupe_lock(&mut tx, &request).await?;

    if let Some(committed) = select_dedupe_result(&mut tx, &request).await? {
        tx.commit().await?;
        return Ok(AppendOutcome::Duplicate(committed));
    }

    acquire_stream_lock(&mut tx, &request).await?;
    let current_revision = select_stream_revision_for_update(&mut tx, &request).await?;
    validate_expected_revision(&request, current_revision)?;
    // insert stream/events/outbox...
    let dedupe_inserted = insert_dedupe_result(&mut tx, &request, &committed).await?;
    if !dedupe_inserted {
        tx.rollback().await?;
        return select_duplicate_after_late_conflict(pool, &request).await;
    }

    tx.commit().await?;
    Ok(AppendOutcome::Committed(committed))
}
```

**Current dedupe lookup pattern** (lines 111-157):
```rust
async fn select_dedupe_result(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
) -> StoreResult<Option<CommittedAppend>> {
    let response_payload = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT response_payload
        FROM command_dedup
        WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(&request.idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;

    response_payload
        .map(|payload| {
            serde_json::from_value(payload)
                .map_err(|source| StoreError::DedupeResultDecode { source })
        })
        .transpose()
}
```

**Insert response payload pattern** (lines 330-380):
```rust
async fn insert_dedupe_result(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
    committed: &CommittedAppend,
) -> StoreResult<bool> {
    let response_payload = serde_json::to_value(committed)
        .map_err(|source| StoreError::DedupeResultDecode { source })?;

    let inserted = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO command_dedup (
            tenant_id,
            idempotency_key,
            stream_id,
            first_revision,
            last_revision,
            first_global_position,
            last_global_position,
            event_ids,
            response_payload
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
        RETURNING 1::bigint
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(&request.idempotency_key)
    .bind(response_payload)
    .fetch_optional(&mut **tx)
    .await?;
```

**Phase 8 copy rule:** Expose a pool-level lookup by `(tenant_id, idempotency_key)` for runtime pre-decision reads. Decode the same full duplicate DTO used by append duplicate handling. Preserve transaction-scoped advisory locking inside append; the pre-decision lookup is an optimization/source-of-truth read, not a replacement for append's in-transaction recheck.

---

### `crates/es-store-postgres/src/event_store.rs` (service/facade, request-response)

**Analog:** `crates/es-store-postgres/src/event_store.rs`

**Facade and instrumentation pattern** (lines 25-70):
```rust
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

    let outcome = sql::append(&self.pool, request).await;
    match &outcome {
        Ok(AppendOutcome::Duplicate(committed)) => {
            if let Some(global_position) = committed.global_positions.last() {
                span.record("global_position", global_position);
            }
            counter!("es_dedupe_hits_total").increment(1);
            histogram!("es_append_latency_seconds", "outcome" => "duplicate")
                .record(started_at.elapsed().as_secs_f64());
        }
        _ => {}
    }
    outcome
}
```

**Phase 8 copy rule:** Add a public durable dedupe lookup method on `PostgresEventStore` that delegates to `sql`. Keep metric labels bounded; never label by tenant, idempotency key, stream ID, command ID, or event ID.

---

### `crates/es-runtime/tests/runtime_flow.rs` (test, request-response)

**Analog:** `crates/es-runtime/tests/runtime_flow.rs`

**Fake runtime store pattern** (lines 157-176, 227-275):
```rust
#[derive(Clone)]
struct FakeStore {
    inner: Arc<FakeStoreInner>,
}

struct FakeStoreInner {
    append_requests: Mutex<Vec<AppendRequest>>,
    append_outcomes: Mutex<VecDeque<Result<AppendOutcome, StoreError>>>,
    rehydration: Mutex<RehydrationBatch>,
    rehydration_error: Mutex<Option<StoreError>>,
    append_gate: Mutex<Option<oneshot::Receiver<()>>>,
    append_started: Notify,
}

impl RuntimeEventStore for FakeStore {
    fn append(&self, request: AppendRequest) -> BoxFuture<'_, StoreResult<AppendOutcome>> {
        self.inner.append_requests.lock().expect("append requests").push(request);
        Box::pin(async move { result })
    }

    fn load_rehydration(
        &self,
        _tenant_id: &TenantId,
        _stream_id: &StreamId,
    ) -> BoxFuture<'_, StoreResult<RehydrationBatch>> {
        Box::pin(async move { Ok(batch) })
    }
}
```

**Commit-gated reply test pattern** (lines 382-417):
```rust
#[tokio::test]
async fn reply_is_sent_after_append_commit() {
    let (release_append, wait_for_release) = oneshot::channel();
    let store = FakeStore::with_delayed_commit(wait_for_release);
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(3);
    record_handoff(&mut state, envelope);

    let task = tokio::spawn(async move {
        state.process_next_handoff(&store_for_task, &codec).await.expect("processed");
        state
    });

    store.wait_for_append_start().await;
    assert!(tokio::time::timeout(Duration::from_millis(20), receiver).await.is_err());
    release_append.send(()).expect("release append");
    let state = task.await.expect("task joined");
}
```

**Existing duplicate regression to replace/extend** (lines 572-623):
```rust
#[tokio::test]
async fn duplicate_append_returns_successful_command_outcome() {
    let store = FakeStore::duplicate();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(3);
    record_handoff(&mut state, envelope);

    assert!(state.process_next_handoff(&store, &codec).await.expect("processed"));

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(3, outcome.reply);
    assert_eq!(vec![1], outcome.append.global_positions);
    assert_eq!(1, state.dedupe().len());
}
```

**Phase 8 copy rule:** Add tests proving warm-cache duplicate hit and durable lookup hit do not call `load_rehydration`, do not call `A::decide`, do not call `append`, and return the original reply. Extend `FakeStore` with lookup counters and stored duplicate outcomes.

---

### `crates/es-store-postgres/tests/dedupe.rs` (test, CRUD)

**Analog:** `crates/es-store-postgres/tests/dedupe.rs`

**Imports/test helper pattern** (lines 1-11, 43-58):
```rust
mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_store_postgres::{
    AppendOutcome, AppendRequest, CommittedAppend, NewEvent, PostgresEventStore,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

fn append_request(
    tenant: TenantId,
    stream: StreamId,
    idempotency_key: &str,
    command_seed: u128,
    event_seed: u128,
) -> AppendRequest {
    AppendRequest::new(
        stream,
        ExpectedRevision::NoStream,
        command_metadata(tenant, command_seed),
        idempotency_key,
        vec![new_event(event_seed, "order-1")],
    )
    .expect("valid append request")
}
```

**Dedupe contract tests** (lines 78-107, 167-196):
```rust
#[tokio::test]
async fn duplicate_idempotency_key_returns_original_result() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    let first = store.append(append_request(..., "idempotency-1", 10, 100)).await?;
    let second = store.append(append_request(..., "idempotency-1", 20, 101)).await?;

    let AppendOutcome::Committed(first_committed) = first else { panic!("first append should commit"); };
    let AppendOutcome::Duplicate(second_committed) = second else { panic!("duplicate append should return original result"); };

    assert_eq!(first_committed, second_committed);
    Ok(())
}

#[tokio::test]
async fn concurrent_duplicate_idempotency_key_appends_only_once() -> anyhow::Result<()> {
    let (outcome_a, outcome_b) = tokio::join!(
        store_a.append(request.clone()),
        store_b.append(request.clone())
    );
    assert!(matches!(
        (&outcome_a?, &outcome_b?),
        (AppendOutcome::Committed(_), AppendOutcome::Duplicate(_))
            | (AppendOutcome::Duplicate(_), AppendOutcome::Committed(_))
    ));
    Ok(())
}
```

**Phase 8 copy rule:** Add durable lookup tests that assert the full stored response payload includes append metadata plus typed reply JSON. Add a tenant-scope test for the lookup path, not just append.

---

### `crates/adapter-http/tests/commerce_api.rs` (test, request-response)

**Analog:** `crates/adapter-http/tests/commerce_api.rs`

**HTTP adapter contract pattern** (lines 19-91):
```rust
#[tokio::test]
async fn commerce_api_place_order_submits_command_and_returns_response_contract() {
    let (order_gateway, mut order_rx) =
        CommandGateway::<Order>::new(PartitionRouter::new(4).expect("router"), 8)
            .expect("order gateway");

    let app = router(HttpState {
        order_gateway,
        product_gateway,
        user_gateway,
    });

    let response_task = tokio::spawn(async move {
        let routed = order_rx.recv().await.expect("routed command");
        assert_eq!("tenant-a", routed.envelope.metadata.tenant_id.as_str());
        assert_eq!("idem-place-1", routed.envelope.idempotency_key);

        let sent = routed.envelope.reply.send(Ok(CommandOutcome::new(
            OrderReply::Placed { order_id: OrderId::new("order-1").expect("order id") },
            committed_append("order-order-1", 10),
        )));
        assert!(sent.is_ok(), "send reply");
    });

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(StatusCode::OK, response.status());
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""global_positions":[10]"#));
    assert!(body.contains(r#""reply":{"type":"placed","order_id":"order-1"}"#));
    response_task.await.expect("reply task");
}
```

**Request helper pattern** (lines 150-173):
```rust
fn place_order_request(idempotency_key: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/commands/orders/place")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{
                "tenant_id": "tenant-a",
                "idempotency_key": "{idempotency_key}",
                "order_id": "order-1",
                "user_id": "user-1",
                "user_active": true,
                "lines": [...]
            }}"#
        )))
        .expect("request")
}
```

**Phase 8 copy rule:** Test duplicate HTTP retry by sending two requests with the same idempotency key through a runtime-backed app or by simulating the gateway returning the prior `CommandOutcome`. Assert the response preserves original append fields and typed reply DTO. Do not add adapter-local dedupe state.

---

### `crates/adapter-http/src/commerce.rs` (controller, request-response)

**Analog:** `crates/adapter-http/src/commerce.rs`

**Thin state and routing pattern** (lines 17-46):
```rust
#[derive(Clone)]
pub struct HttpState {
    pub order_gateway: CommandGateway<Order>,
    pub product_gateway: CommandGateway<Product>,
    pub user_gateway: CommandGateway<User>,
}

pub fn commerce_routes(state: HttpState) -> Router {
    Router::new()
        .route("/commands/orders/place", post(place_order))
        .route("/commands/products/reserve", post(reserve_inventory))
        .route("/commands/users/register", post(register_user))
        .with_state(state)
}
```

**Submit-through-gateway pattern** (lines 464-507):
```rust
async fn submit_command<A, F, R>(
    gateway: CommandGateway<A>,
    command: A::Command,
    metadata: CommandMetadata,
    idempotency_key: String,
    map_reply: F,
) -> Result<CommandSuccess<R>, ApiError>
where
    A: es_runtime::Aggregate,
    F: FnOnce(A::Reply) -> R,
{
    let (reply, receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<A>::new(command, metadata, idempotency_key, reply)?;
    gateway.try_submit(envelope)?;
    let outcome = receiver.await.map_err(|_| ApiError::ReplyDropped)??;
    Ok(CommandSuccess::from_outcome(stream_id, outcome, map_reply))
}
```

**Response mapping pattern** (lines 533-554):
```rust
impl<R> CommandSuccess<R> {
    fn from_outcome<A, F>(stream_id: String, outcome: CommandOutcome<A>, map_reply: F) -> Self
    where
        F: FnOnce(A) -> R,
    {
        let stream_revision = outcome.append.last_revision.value();
        Self {
            correlation_id: Uuid::nil(),
            stream_id,
            stream_revision,
            first_revision: outcome.append.first_revision.value(),
            last_revision: stream_revision,
            global_positions: outcome.append.global_positions,
            event_ids: outcome.append.event_ids,
            reply: map_reply(outcome.reply),
        }
    }
}
```

**Phase 8 copy rule:** Prefer tests-only changes here unless runtime response shape requires compile fixes. Keep the adapter thin: decode request, build metadata, submit envelope, await reply, map `CommandOutcome`.

---

### `crates/app/src/commerce_process_manager.rs` (service/test, event-driven)

**Analog:** `crates/app/src/commerce_process_manager.rs`

**Deterministic follow-up key pattern** (lines 68-90, 135-176):
```rust
let envelope = CommandEnvelope::<Product>::new(
    ProductCommand::ReserveInventory {
        product_id: product_id.clone(),
        quantity,
    },
    follow_up_metadata(event),
    format!(
        "pm:{}:{}:reserve:{}",
        self.name.as_str(),
        event.event_id,
        product_id.as_str()
    ),
    reply,
)
.map_err(command_submit_error)?;
self.product_gateway.try_submit(envelope).map_err(command_submit_error)?;

let (command, idempotency_key) = if inventory_reserved {
    (
        OrderCommand::ConfirmOrder { order_id: order_id.clone() },
        format!("pm:{}:{}:confirm:{}", self.name.as_str(), event.event_id, order_id.as_str()),
    )
} else {
    (
        OrderCommand::RejectOrder { order_id: order_id.clone(), reason: "inventory reservation failed".to_owned() },
        format!("pm:{}:{}:reject:{}", self.name.as_str(), event.event_id, order_id.as_str()),
    )
};
```

**Current deterministic-key test** (lines 595-662):
```rust
#[tokio::test]
async fn process_manager_uses_deterministic_idempotency_keys() -> OutboxResult<()> {
    let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
    let manager = CommerceOrderProcessManager::new(
        process_manager_name(),
        product_gateway,
        order_gateway,
    );

    let task = tokio::spawn(async move { manager.process(&process_event).await });

    let reserve = receive_product(&mut product_rx).await;
    assert_eq!(
        format!("pm:{}:{}:reserve:{}", process_manager_name().as_str(), event.event_id, product.as_str()),
        reserve.envelope.idempotency_key
    );

    let confirm = receive_order(&mut order_rx).await;
    assert_eq!(
        format!("pm:{}:{}:confirm:{}", process_manager_name().as_str(), event.event_id, order_id().as_str()),
        confirm.envelope.idempotency_key
    );

    task.await.expect("process task")?;
    Ok(())
}
```

**Phase 8 copy rule:** Add a crash/retry test that runs the same `ProcessEvent` twice with the same deterministic keys. The second pass should receive duplicate successes from runtime/store replay and complete without fresh side effects or offset assumptions.

---

### `crates/app/src/stress.rs` (adapter/wrapper, request-response)

**Analog:** `crates/app/src/stress.rs`

**Runtime store wrapper pattern** (lines 170-225):
```rust
#[derive(Clone, Debug)]
struct MeasuredRuntimeEventStore {
    inner: PostgresRuntimeEventStore,
    append_durations: Arc<Mutex<Vec<u64>>>,
}

impl RuntimeEventStore for MeasuredRuntimeEventStore {
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> Pin<Box<dyn Future<Output = es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>> + Send + '_>> {
        Box::pin(async move {
            let started = Instant::now();
            let outcome = self.inner.append(request).await;
            if outcome.is_ok() {
                self.append_durations.lock().expect("append durations mutex poisoned").push(micros(started.elapsed()));
            }
            outcome
        })
    }

    fn load_rehydration(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> Pin<Box<dyn Future<Output = es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>> + Send + '_>> {
        self.inner.load_rehydration(tenant_id, stream_id)
    }
}
```

**Phase 8 copy rule:** If `RuntimeEventStore` gains `load_dedupe_result`, forward it to `self.inner` here and leave append timing behavior untouched.

## Shared Patterns

### Tenant-Scoped Idempotency
**Source:** `crates/es-runtime/src/cache.rs` lines 52-59; `crates/es-store-postgres/src/sql.rs` lines 111-124  
**Apply to:** runtime cache, durable lookup, append transaction, HTTP retry tests, process-manager replay tests
```rust
pub struct DedupeKey {
    pub tenant_id: TenantId,
    pub idempotency_key: String,
}

WHERE tenant_id = $1 AND idempotency_key = $2
```

### Pre-Decision Duplicate Replay
**Source:** bug location in `crates/es-runtime/src/shard.rs` lines 171-188  
**Apply to:** `ShardState::process_next_handoff`
```rust
// Phase 8 target ordering:
// 1. pop handoff and create DedupeKey
// 2. check shard-local DedupeCache
// 3. check RuntimeEventStore durable dedupe
// 4. only on miss: rehydrate state and call A::decide
```

### Commit-Gated Replies And Cache Mutation
**Source:** `crates/es-runtime/src/shard.rs` lines 245-290; `crates/es-runtime/tests/runtime_flow.rs` lines 382-417  
**Apply to:** committed path, duplicate path, durable lookup replay
```rust
match store.append(append_request).await {
    Ok(AppendOutcome::Committed(committed)) => {
        self.cache.commit_state(envelope.stream_id.clone(), staged_state);
        self.dedupe.record(dedupe_key, DedupeRecord { append: committed.clone() });
        let _ = envelope.reply.send(Ok(CommandOutcome::new(decision.reply, committed)));
    }
}
```

### Durable Dedupe Payload
**Source:** `crates/es-store-postgres/src/sql.rs` lines 330-380; migration lines 27-39  
**Apply to:** store models, SQL insert/select, runtime decode
```sql
CREATE TABLE command_dedup (
    tenant_id text NOT NULL,
    idempotency_key text NOT NULL,
    stream_id text NOT NULL,
    first_revision bigint NOT NULL CHECK (first_revision >= 1),
    last_revision bigint NOT NULL CHECK (last_revision >= first_revision),
    first_global_position bigint NOT NULL CHECK (first_global_position >= 1),
    last_global_position bigint NOT NULL CHECK (last_global_position >= first_global_position),
    event_ids uuid[] NOT NULL,
    response_payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, idempotency_key)
);
```

### Thin HTTP Adapter
**Source:** `crates/adapter-http/src/commerce.rs` lines 464-507  
**Apply to:** adapter duplicate tests and any compile changes in HTTP layer
```rust
let envelope = CommandEnvelope::<A>::new(command, metadata, idempotency_key, reply)?;
gateway.try_submit(envelope)?;
let outcome = receiver.await.map_err(|_| ApiError::ReplyDropped)??;
Ok(CommandSuccess::from_outcome(stream_id, outcome, map_reply))
```

### Process-Manager Deterministic Keys
**Source:** `crates/app/src/commerce_process_manager.rs` lines 68-90, 135-176  
**Apply to:** process-manager duplicate replay tests
```rust
format!("pm:{}:{}:reserve:{}", self.name.as_str(), event.event_id, product_id.as_str())
format!("pm:{}:{}:confirm:{}", self.name.as_str(), event.event_id, order_id.as_str())
format!("pm:{}:{}:reject:{}", self.name.as_str(), event.event_id, order_id.as_str())
```

### Bounded Metrics Labels
**Source:** `crates/es-store-postgres/src/event_store.rs` lines 43-70; `crates/es-runtime/src/shard.rs` lines 268-296  
**Apply to:** duplicate hit metrics in runtime/store
```rust
counter!("es_dedupe_hits_total").increment(1);
histogram!("es_append_latency_seconds", "outcome" => "duplicate")
    .record(started_at.elapsed().as_secs_f64());
histogram!(
    "es_command_latency_seconds",
    "aggregate" => aggregate,
    "outcome" => "duplicate",
)
.record(command_started_at.elapsed().as_secs_f64());
```

## No Analog Found

All likely Phase 8 files have close analogs in the current codebase. No planner fallback to research-only patterns is required.

## Metadata

**Analog search scope:** `crates/es-runtime`, `crates/es-store-postgres`, `crates/adapter-http`, `crates/app`, root and crate migrations  
**Files scanned:** 46 files from the focused crates, plus phase research/validation, roadmap, requirements, and project instructions  
**Pattern extraction date:** 2026-04-19  
**Validation anchors:** `cargo test -p es-runtime duplicate -- --nocapture`; `cargo test -p es-store-postgres --test dedupe duplicate_ -- --test-threads=1 --nocapture`; `cargo test -p adapter-http duplicate -- --nocapture`; `cargo test -p app process_manager_duplicate -- --nocapture`

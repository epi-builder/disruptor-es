# Phase 08: Runtime Duplicate Command Replay - Pattern Map

**Mapped:** 2026-04-19
**Files analyzed:** 18
**Analogs found:** 18 / 18

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/es-runtime/src/cache.rs` | utility | request-response | `crates/es-runtime/src/cache.rs` | exact |
| `crates/es-runtime/src/command.rs` | model | request-response | `crates/es-runtime/src/command.rs` | exact |
| `crates/es-runtime/src/store.rs` | service | request-response | `crates/es-runtime/src/store.rs` | exact |
| `crates/es-runtime/src/shard.rs` | service | event-driven | `crates/es-runtime/src/shard.rs` | exact |
| `crates/es-runtime/src/lib.rs` | config | transform | `crates/es-runtime/src/lib.rs` | exact |
| `crates/es-runtime/tests/runtime_flow.rs` | test | request-response | `crates/es-runtime/tests/runtime_flow.rs` | exact |
| `crates/es-runtime/tests/common/mod.rs` | test utility | request-response | `crates/es-runtime/tests/runtime_flow.rs` | role-match |
| `crates/es-store-postgres/src/models.rs` | model | CRUD | `crates/es-store-postgres/src/models.rs` | exact |
| `crates/es-store-postgres/src/sql.rs` | service | CRUD | `crates/es-store-postgres/src/sql.rs` | exact |
| `crates/es-store-postgres/src/event_store.rs` | service | request-response | `crates/es-store-postgres/src/event_store.rs` | exact |
| `crates/es-store-postgres/src/lib.rs` | config | transform | `crates/es-store-postgres/src/lib.rs` | exact |
| `crates/es-store-postgres/migrations/20260419*_command_replay_payload.sql` | migration | CRUD | `crates/es-store-postgres/migrations/20260417000000_event_store.sql` | role-match |
| `crates/app/migrations/20260419*_command_replay_payload.sql` | migration | CRUD | `crates/app/migrations/20260417000000_event_store.sql` | role-match |
| `migrations/20260419*_command_replay_payload.sql` | migration | CRUD | `migrations/20260417000000_event_store.sql` | role-match |
| `crates/es-store-postgres/tests/dedupe.rs` | test | CRUD | `crates/es-store-postgres/tests/dedupe.rs` | exact |
| `crates/adapter-http/tests/commerce_api.rs` | test | request-response | `crates/adapter-http/tests/commerce_api.rs` | exact |
| `crates/app/src/commerce_process_manager.rs` | component | event-driven | `crates/app/src/commerce_process_manager.rs` | exact |
| `crates/app/tests/*process_manager*.rs` if split from source tests | test | event-driven | `crates/app/src/commerce_process_manager.rs` | role-match |

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

**Shard-local state pattern** (lines 7-39):
```rust
pub struct AggregateCache<A: Aggregate> {
    states: HashMap<StreamId, A::State>,
}

impl<A: Aggregate> AggregateCache<A> {
    pub fn get(&self, stream_id: &StreamId) -> Option<&A::State> {
        self.states.get(stream_id)
    }
}
```

**Dedupe cache pattern to extend** (lines 52-89):
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

pub fn get(&self, key: &DedupeKey) -> Option<&DedupeRecord> {
    self.records.get(key)
}

pub fn record(&mut self, key: DedupeKey, record: DedupeRecord) {
    self.records.insert(key, record);
}
```

**Planner note:** Extend `DedupeRecord` to carry replayable reply payload/outcome data. Preserve tenant-scoped `DedupeKey`; PostgreSQL remains authoritative.

---

### `crates/es-runtime/src/command.rs` (model, request-response)

**Analog:** `crates/es-runtime/src/command.rs`

**Imports pattern** (lines 1-3):
```rust
use es_kernel::Aggregate;

use crate::{RuntimeError, RuntimeResult};
```

**Envelope validation pattern** (lines 28-53):
```rust
pub fn new(
    command: A::Command,
    metadata: es_core::CommandMetadata,
    idempotency_key: impl Into<String>,
    reply: CommandReply<A::Reply>,
) -> RuntimeResult<Self> {
    let idempotency_key = idempotency_key.into();
    if idempotency_key.is_empty() {
        return Err(RuntimeError::Codec {
            message: "idempotency key cannot be empty".to_owned(),
        });
    }

    let stream_id = A::stream_id(&command);
    let partition_key = A::partition_key(&command);
    let expected_revision = A::expected_revision(&command);
```

**Outcome/reply pattern** (lines 57-69):
```rust
pub struct CommandOutcome<R> {
    pub reply: R,
    pub append: es_store_postgres::CommittedAppend,
}

impl<R> CommandOutcome<R> {
    pub fn new(reply: R, append: es_store_postgres::CommittedAppend) -> Self {
        Self { reply, append }
    }
}
```

**Codec contract pattern to extend** (lines 72-89):
```rust
pub trait RuntimeEventCodec<A: Aggregate>: Clone + Send + Sync + 'static {
    fn encode(&self, event: &A::Event, metadata: &es_core::CommandMetadata)
        -> RuntimeResult<es_store_postgres::NewEvent>;

    fn decode(&self, stored: &es_store_postgres::StoredEvent) -> RuntimeResult<A::Event>;

    fn decode_snapshot(
        &self,
        snapshot: &es_store_postgres::SnapshotRecord,
    ) -> RuntimeResult<A::State>;
}
```

**Planner note:** If durable replay persists typed replies, add encode/decode reply methods or a separate replay payload type here. Do not reconstruct replies by calling `A::decide`.

---

### `crates/es-runtime/src/store.rs` (service, request-response)

**Analog:** `crates/es-runtime/src/store.rs`

**Trait boundary pattern** (lines 1-17):
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

    fn load_rehydration(...) -> BoxFuture<'_, es_store_postgres::StoreResult<...>> {
        let tenant_id = tenant_id.clone();
        let stream_id = stream_id.clone();

        Box::pin(async move { self.inner.load_rehydration(&tenant_id, &stream_id).await })
    }
}
```

**Planner note:** Add a `lookup_dedupe`/`lookup_command_replay` method here using the same boxed-future style. Update all fake stores implementing this trait.

---

### `crates/es-runtime/src/shard.rs` (service, event-driven)

**Analog:** `crates/es-runtime/src/shard.rs`

**Imports pattern** (lines 1-14):
```rust
use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use es_kernel::Aggregate;
use metrics::{gauge, histogram};
use tracing::info_span;

use crate::{
    AggregateCache, CommandEnvelope, CommandOutcome, DedupeCache, DedupeKey, DedupeRecord,
    DisruptorPath, RoutedCommand, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
    RuntimeResult, ShardId,
};
```

**Shard-owned state pattern** (lines 68-83):
```rust
pub struct ShardState<A: Aggregate> {
    shard_id: ShardId,
    cache: AggregateCache<A>,
    dedupe: DedupeCache,
    handoffs: VecDeque<ShardHandoff<A>>,
}

pub fn new(shard_id: ShardId) -> Self {
    Self {
        shard_id,
        cache: AggregateCache::new(),
        dedupe: DedupeCache::new(),
        handoffs: VecDeque::new(),
    }
}
```

**Current broken ordering to replace** (lines 171-188):
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

**Commit-gated reply pattern** (lines 245-273):
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
```

**Duplicate branch anti-pattern** (lines 275-290):
```rust
Ok(es_store_postgres::AppendOutcome::Duplicate(committed)) => {
    self.dedupe.record(...);
    let _ = envelope
        .reply
        .send(Ok(CommandOutcome::new(decision.reply, committed)));
}
```

**Planner note:** Insert local dedupe lookup and durable lookup immediately after `let envelope = handoff.envelope;` and before cache rehydration. The duplicate branch must replay stored outcome, not `decision.reply`.

---

### `crates/es-store-postgres/src/models.rs` (model, CRUD)

**Analog:** `crates/es-store-postgres/src/models.rs`

**Serde DTO pattern** (lines 1-9):
```rust
use std::collections::HashSet;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_outbox::NewOutboxMessage;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
```

**Validated append request pattern** (lines 83-139):
```rust
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

**Planner note:** Add any durable replay model beside `CommittedAppend` and export it from `lib.rs`. Keep derives `Clone, Debug, Deserialize, PartialEq, Serialize`.

---

### `crates/es-store-postgres/src/sql.rs` (service, CRUD)

**Analog:** `crates/es-store-postgres/src/sql.rs`

**Transaction ordering pattern** (lines 13-23):
```rust
pub(crate) async fn append(pool: &PgPool, request: AppendRequest) -> StoreResult<AppendOutcome> {
    let mut tx = pool.begin().await?;

    acquire_dedupe_lock(&mut tx, &request).await?;

    if let Some(committed) = select_dedupe_result(&mut tx, &request).await? {
        tx.commit().await?;
        return Ok(AppendOutcome::Duplicate(committed));
    }

    acquire_stream_lock(&mut tx, &request).await?;
```

**Advisory lock pattern** (lines 73-89):
```rust
sqlx::query(
    r#"
    SELECT pg_advisory_xact_lock(
        hashtextextended($1 || ':' || $2, 0)
    )
    "#,
)
.bind(request.command_metadata.tenant_id.as_str())
.bind(&request.idempotency_key)
.execute(&mut **tx)
.await?;
```

**Dedupe select pattern** (lines 111-133):
```rust
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
```

**Dedupe insert pattern** (lines 330-384):
```rust
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
```

**Planner note:** Expose a read-only durable replay lookup that uses `tenant_id` plus `idempotency_key`. Do not introduce stream lock ordering before the dedupe key. If changing payload shape, decode old `CommittedAppend` payloads or add a compatible version wrapper.

---

### `crates/es-store-postgres/src/event_store.rs` (service, request-response)

**Analog:** `crates/es-store-postgres/src/event_store.rs`

**Public service method pattern** (lines 25-43):
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
    let _entered = span.enter();

    let outcome = sql::append(&self.pool, request).await;
```

**Metrics/error handling pattern** (lines 44-70):
```rust
match &outcome {
    Ok(AppendOutcome::Committed(committed)) => { ... }
    Ok(AppendOutcome::Duplicate(committed)) => {
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
```

**Planner note:** Add a public `lookup_dedupe`/`lookup_command_replay` method here and instrument duplicate lookup hits separately or reuse `es_dedupe_hits_total` with labels if adding labels matches local metric style.

---

### `crates/es-runtime/tests/runtime_flow.rs` (test, request-response)

**Analog:** `crates/es-runtime/tests/runtime_flow.rs`

**Test aggregate/codec pattern** (lines 25-57 and 105-127):
```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CounterState {
    value: i64,
}

struct CounterAggregate;

impl Aggregate for CounterAggregate {
    type State = CounterState;
    type Command = CounterCommand;
    type Event = CounterEvent;
    type Reply = i64;
    type Error = &'static str;
}

impl RuntimeEventCodec<CounterAggregate> for CounterCodec {
    fn encode(...) -> es_runtime::RuntimeResult<NewEvent> {
        ...
    }
}
```

**Fake store pattern** (lines 157-275):
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
    fn append(&self, request: AppendRequest) -> BoxFuture<'_, StoreResult<AppendOutcome>> { ... }
    fn load_rehydration(...) -> BoxFuture<'_, StoreResult<RehydrationBatch>> { ... }
}
```

**Commit-gated reply test pattern** (lines 382-416):
```rust
let task = tokio::spawn(async move {
    state
        .process_next_handoff(&store_for_task, &codec)
        .await
        .expect("processed");
    state
});

store.wait_for_append_start().await;
assert!(
    tokio::time::timeout(Duration::from_millis(20), receiver)
        .await
        .is_err(),
    "reply resolved before durable append completed"
);
```

**Duplicate regression test pattern to strengthen** (lines 572-596):
```rust
let store = FakeStore::duplicate();
...
let outcome = receiver.await.expect("reply").expect("success");
assert_eq!(3, outcome.reply);
assert_eq!(vec![1], outcome.append.global_positions);
assert_eq!(1, state.dedupe().len());
```

**Planner note:** Add tests proving local dedupe and durable dedupe skip rehydration, skip `A::decide`, skip encode, and skip append. Instrument `FakeStore` with lookup counts and decide/encode counters.

---

### `crates/es-store-postgres/tests/dedupe.rs` (test, CRUD)

**Analog:** `crates/es-store-postgres/tests/dedupe.rs`

**Testcontainers setup pattern** (lines 78-82):
```rust
#[tokio::test]
async fn duplicate_idempotency_key_returns_original_result() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
```

**Append helper pattern** (lines 43-58):
```rust
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

**Duplicate assertion pattern** (lines 85-105):
```rust
let first = store
    .append(append_request(tenant.clone(), stream.clone(), "idempotency-1", 10, 100))
    .await?;
let second = store
    .append(append_request(tenant, stream, "idempotency-1", 20, 101))
    .await?;

let AppendOutcome::Committed(first_committed) = first else {
    panic!("first append should commit");
};
let AppendOutcome::Duplicate(second_committed) = second else {
    panic!("duplicate append should return original result");
};

assert_eq!(first_committed, second_committed);
```

**Concurrent dedupe pattern** (lines 167-193):
```rust
let (outcome_a, outcome_b) = tokio::join!(
    store_a.append(request.clone()),
    store_b.append(request.clone())
);
...
assert_eq!(committed(outcome_a), committed(outcome_b));
assert_eq!(1, event_count(&harness.pool, "tenant-a", "order-1").await?);
```

**Planner note:** Add durable replay lookup tests after first append and after constructing a new `PostgresEventStore` over the same pool. Assert typed reply payload is original and tenant-scoped.

---

### `crates/adapter-http/tests/commerce_api.rs` (test, request-response)

**Analog:** `crates/adapter-http/tests/commerce_api.rs`

**Router/gateway harness pattern** (lines 19-33):
```rust
let (order_gateway, mut order_rx) =
    CommandGateway::<Order>::new(PartitionRouter::new(4).expect("router"), 8)
        .expect("order gateway");
let (product_gateway, _product_rx) =
    CommandGateway::new(PartitionRouter::new(4).expect("router"), 8).expect("product gateway");
let (user_gateway, _user_rx) =
    CommandGateway::new(PartitionRouter::new(4).expect("router"), 8).expect("user gateway");

let app = router(HttpState {
    order_gateway,
    product_gateway,
    user_gateway,
});
```

**Command capture and reply pattern** (lines 60-78):
```rust
let response_task = tokio::spawn(async move {
    let routed = order_rx.recv().await.expect("routed command");
    assert_eq!("tenant-a", routed.envelope.metadata.tenant_id.as_str());
    assert_eq!("idem-place-1", routed.envelope.idempotency_key);
    assert_eq!("order-order-1", routed.envelope.stream_id.as_str());

    let sent = routed.envelope.reply.send(Ok(CommandOutcome::new(
        OrderReply::Placed {
            order_id: OrderId::new("order-1").expect("order id"),
        },
        committed_append("order-order-1", 10),
    )));
    assert!(sent.is_ok(), "send reply");
});
```

**Response contract assertion pattern** (lines 80-91):
```rust
let response = app.oneshot(request).await.expect("response");
assert_eq!(StatusCode::OK, response.status());
let body = body_string(response.into_body()).await;

assert!(body.contains(r#""correlation_id":"018f3212-9299-7a4b-8bd3-3f3cc48c0f46""#));
assert!(body.contains(r#""stream_id":"order-order-1""#));
assert!(body.contains(r#""global_positions":[10]"#));
assert!(body.contains(r#""reply":{"type":"placed","order_id":"order-1"}"#));
```

**Request helper pattern** (lines 150-173):
```rust
fn place_order_request(idempotency_key: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/commands/orders/place")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{
            "tenant_id": "tenant-a",
            "idempotency_key": "{idempotency_key}",
            ...
        }}"#)))
        .expect("request")
}
```

**Planner note:** Duplicate HTTP retry tests should submit two identical requests with the same idempotency key and assert the second response body keeps first append positions/event IDs/reply. Adapter should remain thin; runtime/store own replay.

---

### `crates/app/src/commerce_process_manager.rs` (component/test, event-driven)

**Analog:** `crates/app/src/commerce_process_manager.rs`

**Process manager gateway pattern** (lines 12-31):
```rust
pub struct CommerceOrderProcessManager {
    name: ProcessManagerName,
    product_gateway: CommandGateway<Product>,
    order_gateway: CommandGateway<Order>,
}

pub fn new(
    name: ProcessManagerName,
    product_gateway: CommandGateway<Product>,
    order_gateway: CommandGateway<Order>,
) -> Self {
    Self {
        name,
        product_gateway,
        order_gateway,
    }
}
```

**Deterministic reserve key pattern** (lines 69-89):
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
self.product_gateway
    .try_submit(envelope)
    .map_err(command_submit_error)?;
```

**Confirm/reject key pattern** (lines 135-176):
```rust
let (command, idempotency_key) = if inventory_reserved {
    (
        OrderCommand::ConfirmOrder {
            order_id: order_id.clone(),
        },
        format!(
            "pm:{}:{}:confirm:{}",
            self.name.as_str(),
            event.event_id,
            order_id.as_str()
        ),
    )
} else {
    (
        OrderCommand::RejectOrder { ... },
        format!(
            "pm:{}:{}:reject:{}",
            self.name.as_str(),
            event.event_id,
            order_id.as_str()
        ),
    )
};
```

**Existing deterministic-key test pattern** (lines 595-659):
```rust
let reserve = receive_product(&mut product_rx).await;
assert_eq!(
    format!(
        "pm:{}:{}:reserve:{}",
        process_manager_name().as_str(),
        event.event_id,
        product.as_str()
    ),
    reserve.envelope.idempotency_key
);

let confirm = receive_order(&mut order_rx).await;
assert_eq!(
    format!(
        "pm:{}:{}:confirm:{}",
        process_manager_name().as_str(),
        event.event_id,
        order_id().as_str()
    ),
    confirm.envelope.idempotency_key
);
```

**Planner note:** Add a retry test that processes the same event twice before offset advancement. The second run must submit the same idempotency keys and receive original outcomes from runtime/store replay.

---

### Migration files (migration, CRUD)

**Analogs:** `migrations/20260417000000_event_store.sql`, `crates/es-store-postgres/migrations/20260417000000_event_store.sql`, `crates/app/migrations/20260417000000_event_store.sql`

**Existing command dedupe schema** (root migration lines 27-39):
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

**Planner note:** If Phase 8 changes schema, duplicate the migration in all migration roots currently used by the repo: root `migrations/`, `crates/es-store-postgres/migrations/`, and `crates/app/migrations/`. Use additive `ALTER TABLE` where possible.

## Shared Patterns

### Tenant-Scoped Idempotency
**Source:** `crates/es-runtime/src/cache.rs` lines 52-59 and `migrations/20260417000000_event_store.sql` lines 27-39  
**Apply to:** Runtime cache, runtime store lookup, SQL lookup, adapter/process-manager tests.
```rust
pub struct DedupeKey {
    pub tenant_id: TenantId,
    pub idempotency_key: String,
}
```

### Commit-Gated Replies
**Source:** `crates/es-runtime/src/shard.rs` lines 245-267  
**Apply to:** All first-attempt command processing and replay cache recording.
```rust
Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
    self.cache.commit_state(envelope.stream_id.clone(), staged_state);
    self.dedupe.record(...);
    let _ = envelope
        .reply
        .send(Ok(CommandOutcome::new(decision.reply, committed)));
}
```

### Dedupe-First SQL Lock Ordering
**Source:** `crates/es-store-postgres/src/sql.rs` lines 13-23 and 73-89  
**Apply to:** Any durable dedupe lookup/write path.
```rust
let mut tx = pool.begin().await?;
acquire_dedupe_lock(&mut tx, &request).await?;
if let Some(committed) = select_dedupe_result(&mut tx, &request).await? {
    tx.commit().await?;
    return Ok(AppendOutcome::Duplicate(committed));
}
acquire_stream_lock(&mut tx, &request).await?;
```

### Runtime Trait Style
**Source:** `crates/es-runtime/src/store.rs` lines 3-17  
**Apply to:** New durable replay lookup on `RuntimeEventStore` and all fake stores.
```rust
pub trait RuntimeEventStore: Clone + Send + Sync + 'static {
    fn append(...) -> BoxFuture<'_, es_store_postgres::StoreResult<...>>;
    fn load_rehydration(...) -> BoxFuture<'_, es_store_postgres::StoreResult<...>>;
}
```

### HTTP Adapter Thin Boundary
**Source:** `crates/adapter-http/src/commerce.rs` lines 464-503  
**Apply to:** HTTP duplicate response tests; do not add adapter-side dedupe state.
```rust
gateway.try_submit(envelope)?;
let outcome = receiver.await.map_err(|_| ApiError::ReplyDropped)??;
Ok(CommandSuccess::from_outcome(stream_id, outcome, map_reply))
```

### Process-Manager Follow-Up Keys
**Source:** `crates/app/src/commerce_process_manager.rs` lines 78-83 and 141-146  
**Apply to:** Process-manager retry replay tests.
```rust
format!(
    "pm:{}:{}:reserve:{}",
    self.name.as_str(),
    event.event_id,
    product_id.as_str()
)
```

## No Analog Found

No files lack a codebase analog. The only design gap is durable typed reply persistence; planner should derive its shape from `CommandOutcome<R>` and the existing serde DTO style in `models.rs`.

## Metadata

**Analog search scope:** `crates/`, `migrations/`, `benches/`  
**Files scanned:** 63 Rust/SQL/benchmark files from `rg --files crates migrations benches`  
**Pattern extraction date:** 2026-04-19  
**Primary phase inputs:** `.planning/phases/08-runtime-duplicate-command-replay/08-RESEARCH.md`, `.planning/phases/08-runtime-duplicate-command-replay/08-VALIDATION.md`  

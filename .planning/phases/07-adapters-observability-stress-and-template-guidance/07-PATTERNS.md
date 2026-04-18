# Phase 07: Adapters, Observability, Stress, and Template Guidance - Pattern Map

**Mapped:** 2026-04-18
**Files analyzed:** 19 new/modified files inferred from `07-RESEARCH.md`
**Analogs found:** 19 / 19

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `Cargo.toml` | config | batch | `Cargo.toml` | exact |
| `crates/adapter-http/Cargo.toml` | config | request-response | `crates/es-runtime/Cargo.toml` | role-match |
| `crates/adapter-http/src/lib.rs` | route/component | request-response | `crates/adapter-http/src/lib.rs`, `crates/adapter-grpc/src/lib.rs` | partial |
| `crates/adapter-http/src/commerce.rs` | route/controller | request-response | `crates/app/src/commerce_process_manager.rs` | data-flow match |
| `crates/adapter-http/src/error.rs` | utility | request-response | `crates/es-runtime/src/error.rs` | role-match |
| `crates/adapter-http/tests/commerce_api.rs` | test | request-response | `crates/es-runtime/tests/router_gateway.rs` | data-flow match |
| `crates/app/Cargo.toml` | config | batch | `crates/app/Cargo.toml` | exact |
| `crates/app/src/lib.rs` | provider | event-driven | `crates/app/src/lib.rs` | exact |
| `crates/app/src/main.rs` | config/bootstrap | batch | `crates/app/src/main.rs` | exact |
| `crates/app/src/observability.rs` | provider/utility | event-driven | `crates/es-runtime/src/engine.rs`, `crates/es-runtime/src/shard.rs`, `crates/es-outbox/src/dispatcher.rs` | partial |
| `crates/app/src/stress.rs` | service/utility | batch | `crates/es-runtime/tests/runtime_flow.rs` | data-flow match |
| `benches/ring_only.rs` | test/benchmark | batch | `crates/es-runtime/src/disruptor_path.rs`, `crates/es-runtime/tests/shard_disruptor.rs` | partial |
| `benches/domain_only.rs` | test/benchmark | batch | `crates/example-commerce/src/product.rs` tests | partial |
| `benches/adapter_only.rs` | test/benchmark | request-response | `crates/es-runtime/tests/router_gateway.rs` | partial |
| `benches/storage_only.rs` | test/benchmark | CRUD | `crates/es-store-postgres/tests/append_occ.rs` | partial |
| `benches/projector_outbox.rs` | test/benchmark | event-driven | `crates/es-store-postgres/tests/projections.rs`, `crates/es-store-postgres/tests/outbox.rs` | partial |
| `docs/template-guide.md` | docs | batch | `.planning/PROJECT.md`, `crates/example-commerce/src/lib.rs` | partial |
| `docs/hot-path-rules.md` | docs | batch | `.planning/PROJECT.md`, `crates/es-runtime/src/shard.rs` | partial |
| `docs/stress-results.md` | docs | batch | `.planning/REQUIREMENTS.md`, `07-RESEARCH.md` | partial |

## Pattern Assignments

### `Cargo.toml` (config, batch)

**Analog:** `Cargo.toml`

**Workspace dependency policy** (lines 1-24):

```toml
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1.0.102"
disruptor = "4.0.0"
futures = "0.3.32"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "postgres", "uuid", "time", "json", "migrate"] }
testcontainers = "0.25.0"
testcontainers-modules = { version = "0.13.0", features = ["postgres"] }
thiserror = "2.0.18"
tokio = { version = "1.52.0", features = ["rt-multi-thread", "macros", "time", "sync"] }
tracing = "0.1.44"
twox-hash = "2.1.2"
uuid = { version = "1.23.0", features = ["serde", "v7"] }
time = { version = "=0.3.44", features = ["serde", "formatting", "parsing"] }
```

**Apply:** Add Phase 7 crates (`axum`, `tower`, `tower-http`, `metrics`, `hdrhistogram`, compatible `criterion = "0.7.0"`, compatible `sysinfo = "0.36.1"`) under `[workspace.dependencies]` first. Individual crates should reference `.workspace = true` or path dependencies.

---

### `crates/adapter-http/Cargo.toml` (config, request-response)

**Analog:** `crates/es-runtime/Cargo.toml`

**Crate dependency style** (lines 8-17):

```toml
[dependencies]
disruptor.workspace = true
es-core = { path = "../es-core" }
es-kernel = { path = "../es-kernel" }
es-store-postgres = { path = "../es-store-postgres" }
futures.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
twox-hash.workspace = true
```

**Placeholder style to replace** from `crates/adapter-http/Cargo.toml` (lines 1-11):

```toml
[package]
name = "adapter-http"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]

[lints]
workspace = true
```

**Apply:** Keep package metadata and lint inheritance. Add only adapter dependencies (`axum`, `tower`, `tower-http`, `es-core`, `es-runtime`, `example-commerce`, `serde`, `time`, `uuid`, `tracing`) and avoid storage/projector/outbox repository dependencies in the HTTP crate unless query endpoints are explicitly added.

---

### `crates/adapter-http/src/lib.rs` (route/component, request-response)

**Analog:** `crates/adapter-http/src/lib.rs` and `crates/adapter-grpc/src/lib.rs`

**Boundary marker to preserve as design rule** (adapter-http lines 1-5):

```rust
//! Request decoding boundary for the future HTTP adapter.

/// Phase ownership marker for the HTTP adapter crate.
pub const PHASE_BOUNDARY: &str =
    "Future phases decode HTTP requests here without owning aggregate state.";
```

**Sibling adapter rule** (adapter-grpc lines 1-5):

```rust
//! Request decoding boundary for the future gRPC adapter.

/// Phase ownership marker for the gRPC adapter crate.
pub const PHASE_BOUNDARY: &str =
    "Future phases decode gRPC requests here without owning aggregate state.";
```

**Apply:** Turn `lib.rs` into the HTTP router factory and module exports. Keep state limited to runtime gateways and optional query clients. Do not import `ShardState`, `AggregateCache`, `PostgresOutboxStore`, projector mutation APIs, or `Arc<Mutex<HashMap<...>>>`.

---

### `crates/adapter-http/src/commerce.rs` (route/controller, request-response)

**Analog:** `crates/app/src/commerce_process_manager.rs`

**Imports pattern for command ingress** (lines 4-10):

```rust
use es_core::CommandMetadata;
use es_outbox::{
    OutboxError, OutboxResult, ProcessEvent, ProcessManager, ProcessManagerName, ProcessOutcome,
};
use es_runtime::{CommandEnvelope, CommandGateway};
use example_commerce::{Order, OrderCommand, OrderEvent, Product, ProductCommand};
use uuid::Uuid;
```

**Core command submission pattern** (lines 69-90):

```rust
let (reply, receiver) = tokio::sync::oneshot::channel();
let product_id = line.product_id.clone();
let quantity = line.quantity;
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

**Await reply before success pattern** (lines 92-103):

```rust
match receiver
    .await
    .map_err(|_| OutboxError::CommandReplyDropped)?
{
    Ok(_) => {
        reserved_lines.push((product_id, quantity));
    }
    Err(_) => {
        inventory_reserved = false;
        break;
    }
}
```

**Metadata construction pattern** (lines 193-201):

```rust
fn follow_up_metadata(event: &ProcessEvent) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::now_v7(),
        correlation_id: event.correlation_id,
        causation_id: Some(event.event_id),
        tenant_id: event.tenant_id.clone(),
        requested_at: time::OffsetDateTime::now_utc(),
    }
}
```

**Domain command shapes to map from DTOs** from `crates/example-commerce/src/order.rs` (lines 54-67):

```rust
pub enum OrderCommand {
    /// Places a new order.
    PlaceOrder { order_id: OrderId, user_id: UserId, user_active: bool, lines: Vec<OrderLine> },
    /// Confirms a placed order.
    ConfirmOrder { order_id: OrderId },
    /// Rejects a placed order.
    RejectOrder { order_id: OrderId, reason: String },
    /// Cancels a placed order.
    CancelOrder { order_id: OrderId },
}
```

**Apply:** HTTP handlers should create `CommandMetadata`, a oneshot reply, `CommandEnvelope::<Order|Product|User>::new(...)`, call `CommandGateway::try_submit`, then await the receiver. Return durable positions from `CommandOutcome`, not projection freshness.

---

### `crates/adapter-http/src/error.rs` (utility, request-response)

**Analog:** `crates/es-runtime/src/error.rs`

**Typed runtime error variants** (lines 4-49):

```rust
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("runtime is overloaded")]
    Overloaded,
    #[error("runtime is unavailable")]
    Unavailable,
    #[error("shard {shard_id} is overloaded")]
    #[allow(missing_docs)]
    ShardOverloaded { shard_id: usize },
    #[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
    #[allow(missing_docs)]
    Conflict {
        stream_id: String,
        expected: String,
        actual: Option<u64>,
    },
    #[error("domain error: {message}")]
    Domain {
        message: String,
    },
    #[error("codec error: {message}")]
    Codec {
        message: String,
    },
    #[error("store error")]
    Store(#[from] es_store_postgres::StoreError),
}
```

**Store conflict preservation** (lines 51-67):

```rust
impl RuntimeError {
    /// Converts store errors into runtime-visible errors, preserving conflicts as structured data.
    pub fn from_store_error(error: es_store_postgres::StoreError) -> Self {
        match error {
            es_store_postgres::StoreError::StreamConflict {
                stream_id,
                expected,
                actual,
            } => Self::Conflict {
                stream_id,
                expected,
                actual,
            },
            error => Self::Store(error),
        }
    }
}
```

**Apply:** Implement `ApiError` with `IntoResponse`. Map `Overloaded`/`ShardOverloaded` to overload status, `Unavailable` to service unavailable, `Conflict` to conflict, `Domain`/DTO validation to client errors, and preserve structured fields in JSON. Avoid `anyhow::Error` as the HTTP error surface.

---

### `crates/adapter-http/tests/commerce_api.rs` (test, request-response)

**Analog:** `crates/es-runtime/tests/router_gateway.rs`

**Gateway test imports and aggregate fixture style** (lines 3-8):

```rust
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId, TenantId};
use es_kernel::{Aggregate, Decision};
use es_runtime::{CommandEnvelope, CommandGateway, PartitionRouter, RuntimeError};
use time::OffsetDateTime;
use tokio::sync::oneshot;
use uuid::Uuid;
```

**Envelope helper** (lines 64-80):

```rust
fn envelope(
    tenant_id: &'static str,
    stream_id: &'static str,
    partition_key: &'static str,
) -> CommandEnvelope<GatewayAggregate> {
    let (reply, _rx) = oneshot::channel();
    CommandEnvelope::<GatewayAggregate>::new(
        GatewayCommand {
            stream_id,
            partition_key,
        },
        metadata(tenant_id),
        format!("idem-{tenant_id}-{stream_id}"),
        reply,
    )
    .expect("envelope")
}
```

**Bounded ingress assertion** (lines 126-141):

```rust
#[test]
fn bounded_ingress_returns_overloaded_when_full() {
    let router = PartitionRouter::new(8).expect("router");
    let (gateway, _receiver) =
        CommandGateway::<GatewayAggregate>::new(router, 1).expect("capacity-one gateway");

    gateway
        .try_submit(envelope("tenant-a", "order-123", "order-123"))
        .expect("first submit accepted");

    let error = gateway
        .try_submit(envelope("tenant-a", "order-456", "order-456"))
        .expect_err("second submit overloads bounded ingress");

    assert!(matches!(error, RuntimeError::Overloaded));
}
```

**Apply:** Adapter tests should use fake/in-memory gateways or real `CommandGateway` receivers to assert JSON decode, metadata propagation, idempotency key use, overload mapping, conflict mapping, reply-dropped mapping, and success response position fields.

---

### `crates/app/src/lib.rs` (provider, event-driven)

**Analog:** `crates/app/src/lib.rs`

**Composition export pattern** (lines 1-4):

```rust
//! Application composition library.

/// Commerce process-manager workflow composition.
pub mod commerce_process_manager;
```

**Apply:** Export `observability` and `stress` modules here. Keep application composition in `app`, not in lower crates.

---

### `crates/app/src/main.rs` (bootstrap, batch)

**Analog:** `crates/app/src/main.rs`

**Current shell** (lines 1-5):

```rust
//! Composition binary shell for later service wiring.

fn main() {
    // Phase 01 intentionally limits app composition to crate visibility.
}
```

**Apply:** If Phase 7 adds `serve` or `stress` modes, keep `main.rs` as a thin bootstrap shell that delegates to `app` library functions. Do not put command runtime, projector, or outbox logic directly in `main.rs`.

---

### `crates/app/src/observability.rs` (provider/utility, event-driven)

**Analogs:** `crates/es-runtime/src/engine.rs`, `crates/es-runtime/src/shard.rs`, `crates/es-outbox/src/dispatcher.rs`

**Gateway boundary to instrument** from `engine.rs` (lines 88-113):

```rust
pub async fn process_one(&mut self) -> RuntimeResult<bool>
where
    A::Error: std::fmt::Display,
{
    let Some(routed) = self.receiver.recv().await else {
        return Ok(false);
    };

    let shard_index = routed.shard_id.value();
    let Some(shard) = self.shards.get_mut(shard_index) else {
        let _ = routed.envelope.reply.send(Err(RuntimeError::Unavailable));
        return Ok(true);
    };

    shard.accept_routed(routed)?;
    shard.drain_released_handoffs()?;

    while shard
        .state_mut()
        .process_next_handoff(&self.store, &self.codec)
        .await?
    {}

    Ok(true)
}
```

**Decision/append boundary to instrument** from `shard.rs` (lines 155-212):

```rust
let decision = match A::decide(&current_state, envelope.command, &envelope.metadata) {
    Ok(decision) => decision,
    Err(error) => {
        let _ = envelope.reply.send(Err(RuntimeError::Domain {
            message: error.to_string(),
        }));
        return Ok(true);
    }
};

let mut new_events = Vec::with_capacity(decision.events.len());
for event in &decision.events {
    match codec.encode(event, &envelope.metadata) {
        Ok(encoded) => new_events.push(encoded),
        Err(error) => {
            let _ = envelope.reply.send(Err(error));
            return Ok(true);
        }
    }
}

match store.append(append_request).await {
    Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
        let mut staged_state = current_state;
        for event in &decision.events {
            A::apply(&mut staged_state, event);
        }
        self.cache
            .commit_state(envelope.stream_id.clone(), staged_state);
        let _ = envelope
            .reply
            .send(Ok(CommandOutcome::new(decision.reply, committed)));
    }
```

**Outbox boundary to instrument** from `dispatcher.rs` (lines 41-99):

```rust
pub async fn dispatch_once<S, P>(
    store: &S,
    publisher: &P,
    tenant_id: TenantId,
    worker_id: WorkerId,
    limit: DispatchBatchLimit,
    retry_policy: RetryPolicy,
) -> OutboxResult<DispatchOutcome>
where
    S: OutboxStore,
    P: Publisher,
{
    let claimed = store
        .claim_pending(tenant_id.clone(), worker_id.clone(), limit)
        .await?;
    if claimed.is_empty() {
        return Ok(DispatchOutcome::Idle);
    }

    let mut published = 0;
    let mut retried = 0;
    let mut failed = 0;
```

**Apply:** Observability should initialize subscribers/exporters in `app`, while lower crates emit spans/metrics at these boundaries. Use traces for command IDs/correlation IDs; keep metric labels bounded to aggregate/command/outcome/shard/projector/topic.

---

### `crates/app/src/stress.rs` (service/utility, batch)

**Analog:** `crates/es-runtime/tests/runtime_flow.rs`

**Production-shaped engine composition** (lines 651-669):

```rust
#[tokio::test]
async fn runtime_engine_processes_submitted_command_end_to_end_after_durable_commit() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut engine: CommandEngine<CounterAggregate, _, _> = CommandEngine::new(
        CommandEngineConfig::new(1, 4, 4).expect("config"),
        store,
        codec,
    )
    .expect("engine");
    let gateway = engine.gateway();
    let (envelope, receiver) = envelope(3);

    gateway.try_submit(envelope).expect("submitted");
    assert!(engine.process_one().await.expect("processed"));

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(outcome.append.global_positions, vec![1]);
}
```

**Overload/conflict path coverage** (lines 671-723):

```rust
#[tokio::test]
async fn runtime_flow_covers_overload_disruptor_handoff_conflict_and_commit_paths() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut engine: CommandEngine<CounterAggregate, _, _> = CommandEngine::new(
        CommandEngineConfig::new(1, 1, 4).expect("config"),
        store,
        codec,
    )
    .expect("engine");
    let gateway = engine.gateway();
    let (accepted, accepted_receiver) = envelope(3);
    let (overloaded, _overloaded_receiver) = envelope(4);

    gateway.try_submit(accepted).expect("first submit accepted");
    let error = gateway
        .try_submit(overloaded)
        .expect_err("second submit overloads ingress");
    assert!(matches!(error, RuntimeError::Overloaded));

    assert!(engine.process_one().await.expect("processed"));
    let outcome = accepted_receiver.await.expect("reply").expect("success");
    assert_eq!(outcome.append.global_positions, vec![1]);
```

**Apply:** Stress runner should compose adapter DTO path, gateways, engines, event store, projection catch-up, outbox dispatch, and query path in one process. Record throughput, p50/p95/p99, max, queue depths, append latency, projection/outbox lag, reject rate, and CPU/core utilization.

---

### `benches/ring_only.rs` (benchmark, batch)

**Analog:** `crates/es-runtime/src/disruptor_path.rs`

**Disruptor publication path** (lines 34-68):

```rust
impl<E: Clone + Send + Sync + 'static> DisruptorPath<E> {
    /// Creates a single-producer disruptor path with a caller-controlled event poller.
    pub fn new(
        shard_id: ShardId,
        ring_size: usize,
        event_factory: impl Fn() -> E + Send + Sync + 'static,
    ) -> RuntimeResult<Self> {
        if ring_size == 0 {
            return Err(RuntimeError::InvalidRingSize);
        }

        let builder =
            disruptor::build_single_producer(ring_size, event_factory, BusySpinWithSpinLoopHint);
        let (poller, builder) = builder.new_event_poller();
        let producer = builder.build();

        Ok(Self {
            shard_id,
            producer,
            poller,
            next_release_sequence: 0,
        })
    }

    /// Attempts to publish without waiting for ring capacity.
    pub fn try_publish(&mut self, event: E) -> RuntimeResult<u64> {
        self.producer
            .try_publish(|slot| {
                *slot = event;
            })
            .map(|sequence| sequence as u64)
```

**Apply:** Benchmark only ring publication/polling here. Do not include domain decisions, storage append, HTTP JSON, projection, or outbox.

---

### `benches/domain_only.rs` (benchmark, batch)

**Analog:** `crates/example-commerce/src/product.rs`

**Domain-only decide/apply loop pattern** (lines 690-730):

```rust
proptest! {
    #[test]
    fn product_inventory_sequence_never_goes_negative(steps in prop::collection::vec(inventory_step_strategy(), 0..64)) {
        let create = create_command(5);
        let created = Product::decide(&ProductState::default(), create, &metadata()).expect("created");
        let mut state = ProductState::default();
        let mut events = Vec::new();

        for event in &created.events {
            Product::apply(&mut state, event);
            events.push(event.clone());
        }

        for step in steps {
            let command = match step {
                InventoryStep::Adjust(delta) => ProductCommand::AdjustInventory {
                    product_id: product_id(),
                    delta,
                },
                InventoryStep::Reserve(quantity) => ProductCommand::ReserveInventory {
                    product_id: product_id(),
                    quantity: Quantity::new(quantity).expect("quantity"),
                },
                InventoryStep::Release(quantity) => ProductCommand::ReleaseInventory {
                    product_id: product_id(),
                    quantity: Quantity::new(quantity).expect("quantity"),
                },
            };

            if let Ok(decision) = Product::decide(&state, command, &metadata()) {
```

**Apply:** Criterion benchmark body should run deterministic `decide`/`apply` command sequences against in-memory state only.

---

### `benches/adapter_only.rs` (benchmark, request-response)

**Analog:** `crates/es-runtime/tests/router_gateway.rs`

**Adapter-like envelope construction and bounded submission** (lines 64-80, 126-141):

```rust
let (reply, _rx) = oneshot::channel();
let envelope = CommandEnvelope::<GatewayAggregate>::new(
    GatewayCommand {
        stream_id,
        partition_key,
    },
    metadata(tenant_id),
    format!("idem-{tenant_id}-{stream_id}"),
    reply,
)
.expect("envelope");
```

```rust
gateway
    .try_submit(envelope("tenant-a", "order-123", "order-123"))
    .expect("first submit accepted");

let error = gateway
    .try_submit(envelope("tenant-a", "order-456", "order-456"))
    .expect_err("second submit overloads bounded ingress");
```

**Apply:** Include HTTP JSON decode/DTO mapping/envelope creation/gateway admission. Exclude engine processing and database append.

---

### `benches/storage_only.rs` (benchmark, CRUD)

**Analog:** `crates/es-store-postgres/tests/append_occ.rs`

**Append request helper** (lines 43-59):

```rust
fn append_request(
    tenant: TenantId,
    stream: StreamId,
    expected_revision: ExpectedRevision,
    idempotency_key: &str,
    command_seed: u128,
    events: Vec<NewEvent>,
) -> AppendRequest {
    AppendRequest::new(
        stream,
        expected_revision,
        command_metadata(tenant, command_seed),
        idempotency_key,
        events,
    )
    .expect("valid append request")
}
```

**Append/OCC assertions to preserve in stress validation** (lines 145-183):

```rust
#[tokio::test]
async fn wrong_expected_revision_returns_stream_conflict() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![new_event(100, "OrderPlaced", "order-1")],
        ))
        .await?;

    let error = store
        .append(append_request(
            tenant,
            stream,
            ExpectedRevision::Exact(StreamRevision::new(99)),
            "command-2",
            20,
            vec![new_event(101, "OrderConfirmed", "order-1")],
        ))
        .await
        .expect_err("wrong expected revision should conflict");
```

**Apply:** Storage benchmark should use real PostgreSQL/Testcontainers or a clearly documented external `DATABASE_URL`, measuring append/OCC/dedupe/read paths separately from runtime.

---

### `benches/projector_outbox.rs` (benchmark, event-driven)

**Analogs:** `crates/es-store-postgres/tests/projections.rs`, `crates/es-store-postgres/tests/outbox.rs`

**Projection catch-up pattern** from `projections.rs` (lines 285-317):

```rust
#[tokio::test]
async fn projections_offset_commits_with_read_models() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();

    let positions = append_order_lifecycle(&events, tenant.clone(), "order-1", 100).await?;
    let last_position = *positions.last().expect("positions");

    let outcome = projections.catch_up(&tenant, &projector, limit()).await?;
    assert_eq!(
        CatchUpOutcome::Applied {
            event_count: 2,
            last_global_position: last_position,
        },
        outcome
    );
```

**Outbox dispatcher pattern** from `outbox.rs` (lines 743-783):

```rust
#[tokio::test]
async fn dispatcher_marks_successful_rows_published() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-dispatch-success", 1_000).await?;
    let inserted = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(source_event_id, "orders.placed"),
            source_global_position,
        )
        .await?;
    let publisher = InMemoryPublisher::default();

    let outcome = dispatch_once(
        &outbox,
        &publisher,
        tenant.clone(),
        worker_id("worker-a"),
        batch_limit(10),
        retry_policy(2),
    )
    .await?;
```

**Apply:** Keep projector and outbox benchmarks separate or at least separately reported inside one benchmark file. Use durable global positions, not disruptor sequence numbers.

---

### PostgreSQL Integration Tests for TEST-02 (test, CRUD/event-driven)

**Analog:** `crates/es-store-postgres/tests/common/mod.rs`

**Harness pattern** (lines 1-27):

```rust
use sqlx::{PgPool, postgres::PgPoolOptions};
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

pub struct PostgresHarness {
    _container: ContainerAsync<Postgres>,
    pub pool: PgPool,
}

pub async fn start_postgres() -> anyhow::Result<PostgresHarness> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(PostgresHarness {
        _container: container,
        pool,
    })
}
```

**Dedupe pattern** from `dedupe.rs` (lines 78-107):

```rust
#[tokio::test]
async fn duplicate_idempotency_key_returns_original_result() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    let first = store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            "idempotency-1",
            10,
            100,
        ))
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

**Snapshot pattern** from `snapshots.rs` (lines 210-241):

```rust
#[tokio::test]
async fn rehydration_returns_latest_snapshot_plus_subsequent_events() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![
                new_event(100, "OrderPlaced", "order-1"),
                new_event(101, "OrderConfirmed", "order-1"),
                new_event(102, "OrderPacked", "order-1"),
            ],
        ))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 2, 2))
        .await?;

    let batch = store.load_rehydration(&tenant, &stream).await?;
```

**Apply:** Expand integration coverage by composing existing real repositories. Keep DB tests async `#[tokio::test]`; serialize shared PostgreSQL tests with a static `tokio::sync::Mutex` when repository behavior shares global table state.

---

### `docs/template-guide.md`, `docs/hot-path-rules.md`, `docs/stress-results.md` (docs, batch)

**Analogs:** `.planning/PROJECT.md`, `crates/example-commerce/src/lib.rs`, runtime/outbox boundaries

**Template export pattern** from `crates/example-commerce/src/lib.rs` (lines 1-15):

```rust
//! Commerce fixture aggregates for the typed event-sourcing kernel.

mod ids;
mod order;
mod product;
mod user;

pub use ids::{OrderId, ProductId, Quantity, Sku, UserId};
pub use order::{
    Order, OrderCommand, OrderError, OrderEvent, OrderLine, OrderReply, OrderState, OrderStatus,
};
pub use product::{
    Product, ProductCommand, ProductError, ProductEvent, ProductReply, ProductState,
};
pub use user::{User, UserCommand, UserError, UserEvent, UserReply, UserState, UserStatus};
```

**Hot-path ownership pattern** from `crates/es-runtime/src/shard.rs` (lines 61-78):

```rust
/// Shard-owned state and processable handoff queue.
pub struct ShardState<A: Aggregate> {
    shard_id: ShardId,
    cache: AggregateCache<A>,
    dedupe: DedupeCache,
    handoffs: VecDeque<ShardHandoff<A>>,
}

impl<A: Aggregate> ShardState<A> {
    /// Creates empty state owned by one local shard.
    pub fn new(shard_id: ShardId) -> Self {
        Self {
            shard_id,
            cache: AggregateCache::new(),
            dedupe: DedupeCache::new(),
            handoffs: VecDeque::new(),
        }
    }
```

**Storage-neutral outbox boundary** from `crates/es-outbox/src/dispatcher.rs` (lines 12-39):

```rust
/// Storage boundary used by the outbox dispatcher.
pub trait OutboxStore: Clone + Send + Sync + 'static {
    /// Claims due pending rows for a dispatcher worker.
    fn claim_pending(
        &self,
        tenant_id: TenantId,
        worker_id: WorkerId,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<OutboxMessage>>>;

    /// Marks a row as published after the publisher completes successfully.
    fn mark_published(
        &self,
        tenant_id: TenantId,
        outbox_id: Uuid,
        worker_id: WorkerId,
    ) -> BoxFuture<'_, OutboxResult<()>>;
```

**Apply:** Docs should state the architecture as rules: adapters decode and submit only; shard runtime owns hot aggregate/dedupe state; event store commit is command success; projectors/outbox read committed global positions; stress results must not compare ring-only numbers directly to HTTP E2E latency.

## Shared Patterns

### Command Ingress

**Source:** `crates/es-runtime/src/command.rs`
**Apply to:** HTTP handlers, process managers, adapter/stress tests

```rust
pub struct CommandEnvelope<A: Aggregate> {
    pub command: A::Command,
    pub metadata: es_core::CommandMetadata,
    pub idempotency_key: String,
    pub stream_id: es_core::StreamId,
    pub partition_key: es_core::PartitionKey,
    pub expected_revision: es_core::ExpectedRevision,
    pub reply: CommandReply<A::Reply>,
}

impl<A: Aggregate> CommandEnvelope<A> {
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
```

### Durable Success Response

**Source:** `crates/es-runtime/src/command.rs`
**Apply to:** HTTP success DTOs, stress result accounting

```rust
/// Successful command result returned only after durable append succeeds.
pub struct CommandOutcome<R> {
    /// Aggregate reply.
    pub reply: R,
    /// Durable append result assigned by the event store.
    pub append: es_store_postgres::CommittedAppend,
}
```

**Source:** `crates/es-store-postgres/src/models.rs`

```rust
pub struct CommittedAppend {
    pub stream_id: StreamId,
    pub first_revision: StreamRevision,
    pub last_revision: StreamRevision,
    pub global_positions: Vec<i64>,
    pub event_ids: Vec<Uuid>,
}
```

### Bounded Backpressure

**Source:** `crates/es-runtime/src/gateway.rs`
**Apply to:** HTTP overload behavior, adapter benchmarks, stress reject-rate reporting

```rust
pub fn try_submit(&self, envelope: CommandEnvelope<A>) -> RuntimeResult<()> {
    let shard_id = self
        .router
        .route(&envelope.metadata.tenant_id, &envelope.partition_key);
    let routed = RoutedCommand { shard_id, envelope };

    self.sender.try_send(routed).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => RuntimeError::Overloaded,
        mpsc::error::TrySendError::Closed(_) => RuntimeError::Unavailable,
    })
}
```

### Metadata

**Source:** `crates/es-core/src/lib.rs`
**Apply to:** HTTP request metadata, trace fields, persisted event metadata

```rust
pub struct CommandMetadata {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: TenantId,
    pub requested_at: OffsetDateTime,
}
```

### Storage-Neutral Boundaries

**Source:** `crates/es-outbox/src/process_manager.rs`
**Apply to:** process-manager tests, stress composition, documentation

```rust
pub trait CommittedEventReader: Send + Sync {
    fn read_global(
        &self,
        tenant_id: TenantId,
        after_global_position: i64,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<ProcessEvent>>>;
}

pub trait ProcessManagerOffsetStore: Send + Sync {
    fn process_manager_offset(
        &self,
        tenant_id: TenantId,
        name: ProcessManagerName,
    ) -> BoxFuture<'_, OutboxResult<Option<i64>>>;
```

### Real PostgreSQL Tests

**Source:** `crates/es-store-postgres/tests/common/mod.rs`
**Apply to:** TEST-02 integration tests and storage/stress harnesses

```rust
let container = Postgres::default().with_tag("18").start().await?;
let port = container.get_host_port_ipv4(5432).await?;
let database_url =
    format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

sqlx::migrate!("./migrations").run(&pool).await?;
```

## No Exact Analog Found

| File | Role | Data Flow | Closest Available Pattern |
|------|------|-----------|---------------------------|
| `crates/adapter-http/src/lib.rs` router factory | route/component | request-response | Placeholder adapter boundary plus `CommandGateway` tests; no existing Axum router code exists. |
| `crates/adapter-http/src/error.rs` `IntoResponse` implementation | utility | request-response | `RuntimeError` typed enum; no existing HTTP response mapper exists. |
| `crates/app/src/observability.rs` exporter setup | provider/utility | event-driven | Runtime/outbox/projection boundaries exist; no metrics or subscriber setup module exists yet. |
| `crates/app/src/stress.rs` stress runner | service/utility | batch | Runtime flow tests compose engine/gateway/store; no long-running stress runner exists. |
| `benches/*.rs` Criterion files | benchmark | batch/request-response/event-driven | Layer tests exist; no benchmark files exist yet. |
| `docs/*.md` template guidance | docs | batch | Planning docs and crate boundaries exist; no user-facing template docs exist yet. |

## Metadata

**Analog search scope:** `Cargo.toml`, `crates/**/Cargo.toml`, `crates/**/*.rs`, `crates/**/tests/**/*.rs`, `.planning/*.md`, phase `07-RESEARCH.md`
**Files scanned:** 64 repository files from `rg --files` plus required phase/project planning files
**Pattern extraction date:** 2026-04-18

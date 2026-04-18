# Phase 06: Outbox and Process Manager Workflows - Pattern Map

**Mapped:** 2026-04-18  
**Files analyzed:** 13  
**Analogs found:** 13 / 13

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/es-outbox/Cargo.toml` | config | dependency wiring | `crates/es-projection/Cargo.toml`, `crates/es-store-postgres/Cargo.toml` | role-match |
| `crates/es-outbox/src/lib.rs` | config | facade/re-export | `crates/es-projection/src/lib.rs` | exact |
| `crates/es-outbox/src/error.rs` | utility | request-response | `crates/es-projection/src/error.rs`, `crates/es-store-postgres/src/error.rs` | exact |
| `crates/es-outbox/src/models.rs` | model | CRUD / event-driven | `crates/es-store-postgres/src/models.rs`, `crates/es-projection/src/checkpoint.rs` | exact |
| `crates/es-outbox/src/publisher.rs` | service | event-driven | `crates/es-runtime/src/store.rs`, `crates/es-projection/src/projector.rs` | role-match |
| `crates/es-outbox/src/dispatcher.rs` | service | batch / event-driven | `crates/es-store-postgres/src/projection.rs` | role-match |
| `crates/es-outbox/src/process_manager.rs` | service | event-driven / request-response | `crates/es-projection/src/projector.rs`, `crates/es-runtime/src/command.rs`, `crates/es-runtime/src/gateway.rs` | role-match |
| `crates/es-store-postgres/Cargo.toml` | config | dependency wiring | `crates/es-store-postgres/Cargo.toml` | exact |
| `crates/es-store-postgres/migrations/*_outbox.sql` | migration | CRUD / event-driven | `crates/es-store-postgres/migrations/20260417000000_event_store.sql`, `crates/es-store-postgres/migrations/20260418000000_projection_read_models.sql` | exact |
| `crates/es-store-postgres/src/models.rs` | model | CRUD / event-driven | `crates/es-store-postgres/src/models.rs` | exact |
| `crates/es-store-postgres/src/sql.rs` | utility | CRUD / transaction | `crates/es-store-postgres/src/sql.rs` | exact |
| `crates/es-store-postgres/src/outbox.rs` | service | CRUD / batch | `crates/es-store-postgres/src/projection.rs` | role-match |
| `crates/es-store-postgres/tests/outbox.rs` | test | CRUD / event-driven | `crates/es-store-postgres/tests/projections.rs`, `crates/es-store-postgres/tests/dedupe.rs`, `crates/es-store-postgres/tests/append_occ.rs` | exact |

## Pattern Assignments

### `crates/es-outbox/Cargo.toml` (config, dependency wiring)

**Analog:** `crates/es-store-postgres/Cargo.toml`

**Workspace dependency pattern** (lines 8-17):
```toml
[dependencies]
es-core = { path = "../es-core" }
es-projection = { path = "../es-projection" }
example-commerce = { path = "../example-commerce" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
time.workspace = true
uuid.workspace = true
```

**Apply:** Add only the needed workspace deps to `es-outbox`: `es-core`, `es-kernel`/`es-runtime` if process-manager contracts need gateway types, `example-commerce` only for example/test support, plus `futures`, `serde`, `serde_json`, `thiserror`, `time`, and `uuid`. Keep `[lints] workspace = true` as in `crates/es-outbox/Cargo.toml` lines 10-11.

---

### `crates/es-outbox/src/lib.rs` (config, facade/re-export)

**Analog:** `crates/es-projection/src/lib.rs`

**Module facade pattern** (lines 1-15):
```rust
//! Projector and read-model catch-up boundary.

mod checkpoint;
mod error;
mod projector;
mod query;

pub use checkpoint::{MinimumGlobalPosition, ProjectionBatchLimit, ProjectorName, ProjectorOffset};
pub use error::{ProjectionError, ProjectionResult};
pub use projector::{CatchUpOutcome, ProjectionEvent, Projector};
pub use query::{FreshnessCheck, WaitPolicy, wait_for_minimum_position};

/// Phase ownership marker for the projection crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 5 owns query-side projection catch-up contracts and must not gate command success.";
```

**Apply:** Use private modules plus explicit `pub use` re-exports for `error`, `models`, `publisher`, `dispatcher`, and `process_manager`. Preserve the phase marker already present in `crates/es-outbox/src/lib.rs` lines 1-5.

---

### `crates/es-outbox/src/error.rs` (utility, request-response)

**Analog:** `crates/es-projection/src/error.rs`

**Typed error/result pattern** (lines 1-44):
```rust
/// Result alias for projection and query-side catch-up operations.
pub type ProjectionResult<T> = Result<T, ProjectionError>;

/// Errors returned by projection contracts.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectionError {
    /// Projector name was invalid.
    #[error("projector name cannot be empty")]
    InvalidProjectorName,
    /// A projection batch limit was invalid.
    #[error("batch limit must be between 1 and 1000: {value}")]
    InvalidBatchLimit {
        /// Rejected batch limit value.
        value: i64,
    },
    /// Projection storage returned an infrastructure error.
    #[error("store error: {message}")]
    Store {
        /// Storage error message.
        message: String,
    },
}
```

**Database error pattern** (from `crates/es-store-postgres/src/error.rs` lines 95-97):
```rust
/// SQLx returned a database error.
#[error("database error")]
Database(#[from] sqlx::Error),
```

**Apply:** Prefer `OutboxResult<T> = Result<T, OutboxError>` and concrete variants for invalid topic/worker/batch limit, publisher failure, store failure, command submit failure, command reply dropped, and payload decode. Use `Clone, Debug, Eq, PartialEq` only when variants do not carry non-clone sources; otherwise mirror `StoreError` with `#[derive(Debug, thiserror::Error)]`.

---

### `crates/es-outbox/src/models.rs` (model, CRUD / event-driven)

**Analog:** `crates/es-store-postgres/src/models.rs`

**Validated DTO constructor pattern** (lines 11-60):
```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NewEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

impl NewEvent {
    pub fn new(
        event_id: Uuid,
        event_type: impl Into<String>,
        schema_version: i32,
        payload: serde_json::Value,
        metadata: serde_json::Value,
    ) -> StoreResult<Self> {
        let event_type = event_type.into();
        if event_type.is_empty() {
            return Err(StoreError::InvalidEventType);
        }
        if schema_version <= 0 {
            return Err(StoreError::InvalidSchemaVersion { schema_version });
        }
        Ok(Self { event_id, event_type, schema_version, payload, metadata })
    }
}
```

**Opaque value validation pattern** (from `crates/es-projection/src/checkpoint.rs` lines 47-65):
```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectionBatchLimit(i64);

impl ProjectionBatchLimit {
    pub fn new(value: i64) -> ProjectionResult<Self> {
        if !(1..=1000).contains(&value) {
            return Err(ProjectionError::InvalidBatchLimit { value });
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> i64 {
        self.0
    }
}
```

**Apply:** Define `NewOutboxMessage`, `OutboxMessage`, `PublishEnvelope`, `OutboxStatus`, `DispatchBatchLimit`, `WorkerId`, `DispatchOutcome`, and `ProcessManagerName` with the same constructor style. Validate non-empty topic/message key/worker/name, positive global positions, positive limits, and JSON payload size if a limit is introduced. Use `TenantId` from `es-core`, `Uuid`, `OffsetDateTime`, and `serde_json::Value`.

---

### `crates/es-outbox/src/publisher.rs` (service, event-driven)

**Analog:** `crates/es-runtime/src/store.rs`

**BoxFuture port pattern** (lines 1-17):
```rust
use futures::future::BoxFuture;

/// Runtime-facing event-store boundary.
pub trait RuntimeEventStore: Clone + Send + Sync + 'static {
    /// Appends events to durable storage.
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>>;
}
```

**Apply:** Define `Publisher: Clone + Send + Sync + 'static` with `fn publish(&self, envelope: PublishEnvelope) -> BoxFuture<'_, OutboxResult<()>>`. Do not add `async-trait`; the local async port pattern is boxed futures.

**Test fake pattern** (from `crates/es-runtime/tests/runtime_flow.rs` lines 163-193 and 227-253):
```rust
struct FakeStoreInner {
    append_requests: Mutex<Vec<AppendRequest>>,
    append_outcomes: Mutex<VecDeque<Result<AppendOutcome, StoreError>>>,
}

impl RuntimeEventStore for FakeStore {
    fn append(
        &self,
        request: AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<AppendOutcome>> {
        self.inner.append_requests.lock().expect("append requests").push(request);
        let result = self.inner.append_outcomes.lock().expect("append outcomes").pop_front();
        Box::pin(async move { result.unwrap_or_else(|| Ok(AppendOutcome::Committed(committed_append(1)))) })
    }
}
```

**Apply:** Add an in-memory/idempotent test publisher in `publisher.rs` or tests that records `PublishEnvelope`s behind `Arc<Mutex<_>>` and can return queued failures.

---

### `crates/es-outbox/src/dispatcher.rs` (service, batch / event-driven)

**Analog:** `crates/es-store-postgres/src/projection.rs`

**Catch-up orchestration pattern** (lines 66-115):
```rust
pub async fn catch_up(
    &self,
    tenant_id: &TenantId,
    projector_name: &ProjectorName,
    limit: ProjectionBatchLimit,
) -> ProjectionResult<CatchUpOutcome> {
    let current_offset = self
        .projector_offset(tenant_id, projector_name)
        .await?
        .map(|offset| offset.last_global_position)
        .unwrap_or(0);

    let stored_events = self
        .event_store
        .read_global(tenant_id, current_offset, limit.value())
        .await
        .map_err(store_error)?;
    if stored_events.is_empty() {
        return Ok(CatchUpOutcome::Idle);
    }

    let mut tx = self.pool.begin().await.map_err(projection_store_error)?;
    let apply_result = async {
        for event in &events {
            apply_projection_event(&mut tx, event).await?;
        }
        upsert_projector_offset(&mut tx, tenant_id, projector_name, last_global_position).await
    }
    .await;
    if let Err(error) = apply_result {
        tx.rollback().await.map_err(projection_store_error)?;
        return Err(error);
    }
    tx.commit().await.map_err(projection_store_error)?;

    Ok(CatchUpOutcome::Applied { event_count: events.len(), last_global_position })
}
```

**Apply:** `dispatch_once` should claim a bounded batch, return `DispatchOutcome::Idle` for empty, publish each message, mark success after `publish` returns `Ok(())`, and schedule retry/failure on error. Keep the dispatcher storage-neutral by depending on an `OutboxStore` trait, not `sqlx`.

---

### `crates/es-outbox/src/process_manager.rs` (service, event-driven / request-response)

**Analog:** `crates/es-projection/src/projector.rs`

**Committed-event input and async handler pattern** (lines 8-23 and 39-52):
```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectionEvent {
    pub global_position: i64,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub tenant_id: TenantId,
}

pub trait Projector: Send + Sync {
    fn name(&self) -> &ProjectorName;
    fn handles(&self, event_type: &str, schema_version: i32) -> bool;
    fn apply<'a>(
        &'a self,
        event: &'a ProjectionEvent,
    ) -> Pin<Box<dyn Future<Output = ProjectionResult<()>> + Send + 'a>>;
}
```

**Command envelope and gateway pattern** (from `crates/es-runtime/src/command.rs` lines 26-54 and `crates/es-runtime/src/gateway.rs` lines 43-54):
```rust
let idempotency_key = idempotency_key.into();
if idempotency_key.is_empty() {
    return Err(RuntimeError::Codec {
        message: "idempotency key cannot be empty".to_owned(),
    });
}

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

**Commerce workflow event shapes** (from `crates/example-commerce/src/order.rs` lines 54-82 and `crates/example-commerce/src/product.rs` lines 25-60, 66-101):
```rust
pub enum OrderCommand {
    PlaceOrder { order_id: OrderId, user_id: UserId, user_active: bool, lines: Vec<OrderLine> },
    ConfirmOrder { order_id: OrderId },
    RejectOrder { order_id: OrderId, reason: String },
    CancelOrder { order_id: OrderId },
}

pub enum OrderEvent {
    OrderPlaced { order_id: OrderId, user_id: UserId, lines: Vec<OrderLine> },
    OrderConfirmed { order_id: OrderId },
    OrderRejected { order_id: OrderId, reason: String },
    OrderCancelled { order_id: OrderId },
}

pub enum ProductCommand {
    ReserveInventory { product_id: ProductId, quantity: Quantity },
    ReleaseInventory { product_id: ProductId, quantity: Quantity },
}

pub enum ProductEvent {
    InventoryReserved { product_id: ProductId, quantity: Quantity },
    InventoryReleased { product_id: ProductId, quantity: Quantity },
}
```

**Apply:** The process-manager contract should consume committed events by global position, decode relevant `OrderEvent::OrderPlaced`, submit `ProductCommand::ReserveInventory` through `CommandGateway<Product>`, await one-shot replies, then submit `OrderCommand::ConfirmOrder` or `OrderCommand::RejectOrder`. Derive idempotency keys from process manager name + source event ID + target aggregate/action. Do not advance offsets before reply completion.

---

### `crates/es-store-postgres/migrations/*_outbox.sql` (migration, CRUD / event-driven)

**Analog:** `crates/es-store-postgres/migrations/20260417000000_event_store.sql`

**Tenant-scoped tables and constraints** (lines 1-25 and 27-39):
```sql
CREATE TABLE streams (
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    revision bigint NOT NULL CHECK (revision >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, stream_id)
);

CREATE TABLE events (
    global_position bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    stream_revision bigint NOT NULL CHECK (stream_revision >= 1),
    event_type text NOT NULL CHECK (event_type <> ''),
    payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, stream_id, stream_revision)
);

CREATE TABLE command_dedup (
    tenant_id text NOT NULL,
    idempotency_key text NOT NULL,
    response_payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, idempotency_key)
);
```

**Offset table pattern** (from `20260418000000_projection_read_models.sql` lines 1-7):
```sql
CREATE TABLE projector_offsets (
    tenant_id text NOT NULL,
    projector_name text NOT NULL CHECK (projector_name <> ''),
    last_global_position bigint NOT NULL CHECK (last_global_position >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, projector_name)
);
```

**Apply:** Add `outbox_messages` with checks for non-empty `topic`, `message_key`, statuses, attempts, `source_global_position >= 1`, unique `(tenant_id, source_event_id, topic)`, and FK to `events(event_id)`. Add `process_manager_offsets` mirroring projector offsets with `process_manager_name`. Add pending claim index ordered by status/available/global position.

---

### `crates/es-store-postgres/src/models.rs` (model, CRUD / event-driven)

**Analog:** `crates/es-store-postgres/src/models.rs`

**Append request extension point** (lines 63-104):
```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AppendRequest {
    pub stream_id: StreamId,
    pub expected_revision: ExpectedRevision,
    pub command_metadata: CommandMetadata,
    pub idempotency_key: String,
    pub events: Vec<NewEvent>,
}

impl AppendRequest {
    pub fn new(
        stream_id: StreamId,
        expected_revision: ExpectedRevision,
        command_metadata: CommandMetadata,
        idempotency_key: impl Into<String>,
        events: Vec<NewEvent>,
    ) -> StoreResult<Self> {
        if events.is_empty() {
            return Err(StoreError::EmptyAppend);
        }
        let idempotency_key = idempotency_key.into();
        if idempotency_key.is_empty() {
            return Err(StoreError::InvalidIdempotencyKey);
        }
        Ok(Self { stream_id, expected_revision, command_metadata, idempotency_key, events })
    }
}
```

**Apply:** Either add an `outbox_messages: Vec<NewOutboxMessage>` field to `AppendRequest` or add `AppendRequest::new_with_outbox`. Preserve current `AppendRequest::new` compatibility by defaulting to an empty outbox list.

---

### `crates/es-store-postgres/src/sql.rs` (utility, CRUD / transaction)

**Analog:** `crates/es-store-postgres/src/sql.rs`

**Append transaction boundary** (lines 10-57):
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
    upsert_stream_revision(&mut tx, &request, last_revision).await?;

    for (index, event) in request.events.iter().enumerate() {
        let stream_revision = first_revision + i64::try_from(index).unwrap_or(i64::MAX);
        let inserted = insert_event(&mut tx, &request, event, stream_revision).await?;
        global_positions.push(inserted.global_position);
        event_ids.push(inserted.event_id);
    }

    let dedupe_inserted = insert_dedupe_result(&mut tx, &request, &committed).await?;
    tx.commit().await?;
    Ok(AppendOutcome::Committed(committed))
}
```

**Parameterized SQL insert pattern** (lines 238-270):
```rust
let (event_id, global_position, _stream_revision, _recorded_at) =
    sqlx::query_as::<_, (Uuid, i64, i64, time::OffsetDateTime)>(
        r#"
        INSERT INTO events (
            event_id, tenant_id, stream_id, stream_revision, command_id,
            correlation_id, causation_id, event_type, schema_version, payload, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING event_id, global_position, stream_revision, recorded_at
        "#,
    )
    .bind(event.event_id)
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(request.stream_id.as_str())
    .bind(stream_revision)
    .bind(request.command_metadata.command_id)
    .bind(request.command_metadata.correlation_id)
    .bind(request.command_metadata.causation_id)
    .bind(&event.event_type)
    .bind(event.schema_version)
    .bind(&event.payload)
    .bind(&event.metadata)
    .fetch_one(&mut **tx)
    .await?;
```

**Apply:** Insert outbox rows after `insert_event` returns `event_id/global_position` and before `insert_dedupe_result`/`commit`. On duplicate command replay, return existing dedupe result and do not create new outbox rows. Use parameter binding only.

---

### `crates/es-store-postgres/src/outbox.rs` (service, CRUD / batch)

**Analog:** `crates/es-store-postgres/src/projection.rs`

**Repository struct pattern** (lines 15-34):
```rust
/// PostgreSQL projection repository.
#[derive(Clone, Debug)]
pub struct PostgresProjectionStore {
    pool: sqlx::PgPool,
    event_store: PostgresEventStore,
}

impl PostgresProjectionStore {
    /// Creates a projection repository backed by the provided PostgreSQL pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            event_store: PostgresEventStore::new(pool.clone()),
            pool,
        }
    }

    /// Returns the underlying PostgreSQL pool.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}
```

**Offset load/upsert pattern** (lines 36-64 and 524-551):
```rust
let position = sqlx::query_scalar::<_, i64>(
    r#"
    SELECT last_global_position
    FROM projector_offsets
    WHERE tenant_id = $1 AND projector_name = $2
    "#,
)
.bind(tenant_id.as_str())
.bind(projector_name.as_str())
.fetch_optional(&self.pool)
.await
.map_err(projection_store_error)?;

sqlx::query(
    r#"
    INSERT INTO projector_offsets (tenant_id, projector_name, last_global_position)
    VALUES ($1, $2, $3)
    ON CONFLICT (tenant_id, projector_name) DO UPDATE
    SET last_global_position = GREATEST(
            projector_offsets.last_global_position,
            EXCLUDED.last_global_position
        ),
        updated_at = now()
    "#,
)
.bind(tenant_id.as_str())
.bind(projector_name.as_str())
.bind(last_global_position)
.execute(&mut **tx)
.await
```

**Apply:** Implement `PostgresOutboxStore::new(pool)` and methods for `claim_pending`, `mark_published`, `schedule_retry`, `mark_failed`, `process_manager_offset`, and `advance_process_manager_offset`. Use tenant filters for tenant-owned queries and monotonic offset upserts with `GREATEST`.

**No local exact analog:** The codebase does not yet contain `FOR UPDATE SKIP LOCKED`. Use the research SQL shape for queue claims while preserving the repository/SQLx style above.

---

### `crates/es-store-postgres/tests/outbox.rs` (test, CRUD / event-driven)

**Analog:** `crates/es-store-postgres/tests/projections.rs`

**Container harness pattern** (from `tests/common/mod.rs` lines 10-22):
```rust
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
```

**Test helper style** (from `tests/projections.rs` lines 23-74 and 118-142):
```rust
static POSTGRES_TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn tenant_id(value: &str) -> TenantId {
    TenantId::new(value).expect("valid tenant id")
}

async fn append_events(
    store: &PostgresEventStore,
    tenant: TenantId,
    stream: StreamId,
    expected_revision: ExpectedRevision,
    idempotency_key: &str,
    command_seed: u128,
    events: Vec<NewEvent>,
) -> anyhow::Result<Vec<i64>> {
    let outcome = store
        .append(AppendRequest::new(
            stream,
            expected_revision,
            command_metadata(tenant, command_seed),
            idempotency_key,
            events,
        )?)
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append should commit");
    };

    Ok(committed.global_positions)
}
```

**Dedupe assertion pattern** (from `tests/dedupe.rs` lines 78-107 and 110-132):
```rust
let first = store.append(append_request(..., "idempotency-1", ...)).await?;
let second = store.append(append_request(..., "idempotency-1", ...)).await?;

let AppendOutcome::Committed(first_committed) = first else {
    panic!("first append should commit");
};
let AppendOutcome::Duplicate(second_committed) = second else {
    panic!("duplicate append should return original result");
};

assert_eq!(first_committed, second_committed);
assert_eq!(1, event_count(&harness.pool, "tenant-a", "order-1").await?);
```

**Concurrent test pattern** (from `tests/append_occ.rs` lines 297-370):
```rust
let store = Arc::new(PostgresEventStore::new(harness.pool.clone()));
let left = {
    let store = Arc::clone(&store);
    tokio::spawn(async move { store.append(append_request(...)).await })
};
let right = {
    let store = Arc::clone(&store);
    tokio::spawn(async move { store.append(append_request(...)).await })
};

let left = left.await.expect("left task joins");
let right = right.await.expect("right task joins");
assert_eq!(1, committed);
assert_eq!(1, conflicts);
```

**Apply:** Cover append atomic outbox creation, duplicate command no duplicate outbox, unique `(tenant_id, source_event_id, topic)`, concurrent claims skip locked rows, successful publish marking, retry scheduling, failed after max attempts, tenant isolation, and PM offset monotonicity.

## Shared Patterns

### Storage-Neutral Async Ports

**Source:** `crates/es-runtime/src/store.rs` lines 1-17  
**Apply to:** `publisher.rs`, `dispatcher.rs`, `process_manager.rs`
```rust
use futures::future::BoxFuture;

pub trait RuntimeEventStore: Clone + Send + Sync + 'static {
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>>;
}
```

### Tenant-Scoped Idempotency

**Source:** `crates/es-store-postgres/src/sql.rs` lines 98-120 and 296-332  
**Apply to:** append outbox insert, outbox unique constraints, publisher idempotency key
```rust
SELECT response_payload
FROM command_dedup
WHERE tenant_id = $1 AND idempotency_key = $2

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
```

### Durable Offset Monotonicity

**Source:** `crates/es-store-postgres/src/projection.rs` lines 530-551  
**Apply to:** process-manager offsets
```rust
INSERT INTO projector_offsets (
    tenant_id,
    projector_name,
    last_global_position
)
VALUES ($1, $2, $3)
ON CONFLICT (tenant_id, projector_name) DO UPDATE
SET last_global_position = GREATEST(
        projector_offsets.last_global_position,
        EXCLUDED.last_global_position
    ),
    updated_at = now()
```

### Command Replies After Durable Append

**Source:** `crates/es-runtime/tests/runtime_flow.rs` lines 383-408  
**Apply to:** process-manager follow-up command handling
```rust
store.wait_for_append_start().await;
assert!(
    tokio::time::timeout(Duration::from_millis(20), receiver)
        .await
        .is_err(),
    "reply resolved before durable append completed"
);

release_append.send(()).expect("release append");
```

### Tenant Metadata and Correlation

**Source:** `crates/es-core/src/lib.rs` lines 116-129  
**Apply to:** outbox row metadata and process-manager follow-up commands
```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandMetadata {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: TenantId,
    pub requested_at: OffsetDateTime,
}
```

## No Analog Found

| File / Pattern | Role | Data Flow | Reason |
|---|---|---|---|
| `FOR UPDATE SKIP LOCKED` claim query inside `crates/es-store-postgres/src/outbox.rs` | service | batch / queue claim | No existing queue-claim SQL exists. Use PostgreSQL research pattern, but keep local SQLx repository style. |
| Broker-specific publisher adapter | service | event-driven | Phase 6 defers real broker adapters. Implement only the storage-neutral trait and in-memory/test publisher. |

## Metadata

**Analog search scope:** `crates/**/*.rs`, `crates/**/*.sql`, crate `Cargo.toml`, phase research  
**Files scanned:** 50+ source, migration, manifest, and integration-test files  
**Pattern extraction date:** 2026-04-18

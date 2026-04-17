# Phase 05: CQRS Projection and Query Catch-Up - Pattern Map

**Mapped:** 2026-04-18  
**Files analyzed:** 14  
**Analogs found:** 14 / 14

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/es-projection/Cargo.toml` | config | request-response | `crates/es-store-postgres/Cargo.toml` | exact |
| `crates/es-projection/src/lib.rs` | config | request-response | `crates/es-runtime/src/lib.rs` | exact |
| `crates/es-projection/src/error.rs` | utility | request-response | `crates/es-runtime/src/error.rs` | exact |
| `crates/es-projection/src/checkpoint.rs` | model | CRUD | `crates/es-store-postgres/src/models.rs` | exact |
| `crates/es-projection/src/projector.rs` | service | batch | `crates/es-store-postgres/src/event_store.rs` | role-match |
| `crates/es-projection/src/query.rs` | service | request-response | `crates/es-runtime/tests/runtime_flow.rs` | partial |
| `crates/es-projection/tests/minimum_position.rs` | test | request-response | `crates/es-core/src/lib.rs` test module | role-match |
| `crates/es-store-postgres/migrations/*_projection_read_models.sql` | migration | CRUD | `crates/es-store-postgres/migrations/20260417000000_event_store.sql` | exact |
| `crates/es-store-postgres/src/projection.rs` | service | batch + CRUD | `crates/es-store-postgres/src/sql.rs` | exact |
| `crates/es-store-postgres/src/lib.rs` | config | request-response | `crates/es-store-postgres/src/lib.rs` | exact |
| `crates/es-store-postgres/Cargo.toml` | config | request-response | `crates/es-runtime/Cargo.toml` | exact |
| `crates/es-store-postgres/tests/projections.rs` | test | batch + CRUD | `crates/es-store-postgres/tests/global_reads.rs` | exact |
| `crates/example-commerce/Cargo.toml` | config | transform | `crates/es-store-postgres/Cargo.toml` | role-match |
| `crates/example-commerce/src/{ids,order,product}.rs` | model | transform | `crates/es-store-postgres/src/models.rs` | role-match |

## Pattern Assignments

### `crates/es-projection/Cargo.toml` (config, request-response)

**Analog:** `crates/es-store-postgres/Cargo.toml`

**Workspace dependency pattern** (lines 8-15):

```toml
[dependencies]
es-core = { path = "../es-core" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
time.workspace = true
uuid.workspace = true
```

**Apply:** Add only phase-needed dependencies to `es-projection`: `es-core`, `serde`, `serde_json`, `thiserror`, `tokio`, and `time` if query wait policies expose durations/timestamps. Do not add `es-store-postgres` here; `es-projection` must remain storage-neutral and PostgreSQL-specific conversion belongs in `crates/es-store-postgres/src/projection.rs`. Keep versions inherited from workspace.

**Dev dependency pattern** (lines 17-21):

```toml
[dev-dependencies]
anyhow.workspace = true
testcontainers.workspace = true
testcontainers-modules.workspace = true
tokio.workspace = true
```

**Apply:** For `crates/es-projection/tests/minimum_position.rs`, prefer `tokio.workspace = true` and `anyhow.workspace = true` only if tests return `anyhow::Result`.

---

### `crates/es-projection/src/lib.rs` (config, request-response)

**Analog:** `crates/es-runtime/src/lib.rs`

**Module and re-export pattern** (lines 3-21):

```rust
mod cache;
mod command;
mod disruptor_path;
mod engine;
mod error;
mod gateway;
mod router;
mod shard;
mod store;

pub use cache::{AggregateCache, DedupeCache, DedupeKey, DedupeRecord};
pub use command::{CommandEnvelope, CommandOutcome, CommandReply, RuntimeEventCodec};
pub use error::{RuntimeError, RuntimeResult};
```

**Apply:** Keep implementation modules private by default, then re-export the public contracts:
`ProjectionError`, `ProjectionResult`, `ProjectorName`, `ProjectorOffset`, `MinimumGlobalPosition`, `WaitPolicy`, `Projector`, `CatchUpOutcome`, and query freshness result types.

**Crate boundary documentation pattern** (lines 1-1 and 23-25):

```rust
//! Local command routing, shard ownership, and in-process execution boundary.

/// Phase ownership marker for the runtime crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 3 owns local command routing, shard ownership, and in-process execution.";
```

**Apply:** State that projection owns query-side catch-up contracts and must not become command success gating.

---

### `crates/es-projection/src/error.rs` (utility, request-response)

**Analog:** `crates/es-runtime/src/error.rs`

**Typed error alias pattern** (lines 1-6):

```rust
/// Result alias for runtime command execution.
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Errors returned by the local command runtime.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
```

**Structured error pattern** (lines 26-49):

```rust
/// Storage reported an optimistic-concurrency conflict.
#[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
#[allow(missing_docs)]
Conflict {
    stream_id: String,
    expected: String,
    actual: Option<u64>,
},
/// Projection storage returned an infrastructure error.
#[error("store error: {message}")]
Store { message: String },
```

**Apply:** Define `ProjectionResult<T> = Result<T, ProjectionError>`. Include typed variants for invalid projector name, invalid/minimum global position, invalid batch limit, projection lag timeout, payload decode, and storage/database passthrough as a storage-neutral message variant. Do not depend on `es_store_postgres::StoreError` from `es-projection`; PostgreSQL code maps concrete store errors at the storage boundary.

**Error conversion test pattern** (lines 91-107):

```rust
#[test]
fn runtime_error_maps_store_conflict_to_structured_conflict() {
    let error = RuntimeError::from_store_error(StoreError::StreamConflict {
        stream_id: "order-1".to_owned(),
        expected: "no stream".to_owned(),
        actual: Some(7),
    });

    assert!(matches!(
        error,
        RuntimeError::Conflict {
            stream_id,
            expected,
            actual: Some(7),
        } if stream_id == "order-1" && expected == "no stream"
    ));
}
```

---

### `crates/es-projection/src/checkpoint.rs` (model, CRUD)

**Analog:** `crates/es-store-postgres/src/models.rs`

**Validated constructor pattern** (lines 26-60):

```rust
impl NewEvent {
    /// Creates a validated event append DTO.
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

        Ok(Self {
            event_id,
            event_type,
            schema_version,
            payload,
            metadata,
        })
    }
}
```

**DTO derive pattern** (lines 106-128):

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedAppend {
    pub stream_id: StreamId,
    pub first_revision: StreamRevision,
    pub last_revision: StreamRevision,
    pub global_positions: Vec<i64>,
    pub event_ids: Vec<Uuid>,
}
```

**Apply:** Model `ProjectorName` as a nonempty string-backed type, `ProjectorOffset` as tenant/name/last position, and `MinimumGlobalPosition` as a validated nonnegative position. Use `serde` derives for public DTOs that cross storage/query boundaries.

---

### `crates/es-projection/src/projector.rs` (service, batch)

**Analog:** `crates/es-store-postgres/src/event_store.rs`

**Thin public service wrapper pattern** (lines 6-20):

```rust
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
}
```

**Global-position catch-up source pattern** (lines 47-55):

```rust
/// Reads events by durable global position.
pub async fn read_global(
    &self,
    tenant_id: &es_core::TenantId,
    after_global_position: i64,
    limit: i64,
) -> StoreResult<Vec<StoredEvent>> {
    sql::read_global(&self.pool, tenant_id, after_global_position, limit).await
}
```

**Apply:** `es-projection` should define storage-neutral `ProjectionEvent` and `Projector` contracts. The PostgreSQL projector runner in `es-store-postgres` should call `read_global(tenant_id, offset.last_global_position, limit)`, convert each `StoredEvent` to `ProjectionEvent`, and return an explicit `CatchUpOutcome` such as `Idle` or `Applied { event_count, last_global_position }`.

---

### `crates/es-projection/src/query.rs` (service, request-response)

**Analog:** `crates/es-runtime/tests/runtime_flow.rs`

**Bounded wait pattern** (lines 400-406):

```rust
store.wait_for_append_start().await;
assert!(
    tokio::time::timeout(Duration::from_millis(20), receiver)
        .await
        .is_err(),
    "reply resolved before durable append completed"
);
```

**Apply:** Query waiting must be bounded. Use `tokio::time::timeout`, `sleep`, or an explicit deadline; return `ProjectionError::ProjectionLag { required, actual }` when the offset never reaches the requested minimum position.

---

### `crates/es-projection/tests/minimum_position.rs` (test, request-response)

**Analog:** `crates/es-core/src/lib.rs`

**Constructor validation test pattern** (lines 152-183):

```rust
#[test]
fn constructors_return_valid_opaque_newtypes() {
    let stream_id = StreamId::new("order-1").expect("valid stream id");
    let partition_key = PartitionKey::new("order-1").expect("valid partition key");
    let tenant_id = TenantId::new("tenant-a").expect("valid tenant id");

    assert_eq!("order-1", stream_id.as_str());
    assert_eq!("order-1", partition_key.as_str());
    assert_eq!("tenant-a", tenant_id.as_str());
}

#[test]
fn empty_strings_return_typed_errors() {
    assert_eq!(
        CoreError::EmptyValue {
            type_name: "StreamId",
        },
        StreamId::new("").expect_err("empty stream id")
    );
}
```

**Apply:** Cover nonnegative `MinimumGlobalPosition`, nonempty `ProjectorName`, timeout behavior, already-fresh behavior, and lag error behavior. Name tests so `cargo test -p es-projection minimum_position -- --nocapture` selects them.

---

### `crates/es-store-postgres/migrations/*_projection_read_models.sql` (migration, CRUD)

**Analog:** `crates/es-store-postgres/migrations/20260417000000_event_store.sql`

**Tenant-scoped primary key pattern** (lines 1-7):

```sql
CREATE TABLE streams (
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    revision bigint NOT NULL CHECK (revision >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, stream_id)
);
```

**Position/index pattern** (lines 9-25 and 51-55):

```sql
CREATE TABLE events (
    global_position bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    stream_revision bigint NOT NULL CHECK (stream_revision >= 1),
    event_type text NOT NULL CHECK (event_type <> ''),
    schema_version integer NOT NULL CHECK (schema_version > 0),
    payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, stream_id, stream_revision)
);

CREATE INDEX events_tenant_global_position_idx
    ON events (tenant_id, global_position);
```

**Apply:** Add `projector_offsets` with `PRIMARY KEY (tenant_id, projector_name)` and `last_global_position bigint NOT NULL CHECK (last_global_position >= 0)`. Add `order_summary_read_models` and `product_inventory_read_models` with tenant-scoped primary keys and `last_applied_global_position bigint NOT NULL CHECK (last_applied_global_position >= 1)`. Index query lookup columns only when not already covered by primary keys.

---

### `crates/es-store-postgres/src/projection.rs` (service, batch + CRUD)

**Analog:** `crates/es-store-postgres/src/sql.rs`

**Transaction pattern** (lines 10-57):

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

    tx.commit().await?;

    Ok(AppendOutcome::Committed(committed))
}
```

**SQLx binding and upsert pattern** (lines 206-218):

```rust
let result = sqlx::query(
    r#"
    INSERT INTO streams (tenant_id, stream_id, revision)
    VALUES ($1, $2, $3)
    ON CONFLICT (tenant_id, stream_id)
    DO UPDATE SET revision = EXCLUDED.revision, updated_at = now()
    "#,
)
.bind(request.command_metadata.tenant_id.as_str())
.bind(request.stream_id.as_str())
.bind(last_revision)
.execute(&mut **tx)
.await?;
```

**Global read query pattern** (lines 420-461):

```rust
pub(crate) async fn read_global(
    pool: &PgPool,
    tenant_id: &TenantId,
    after_global_position: i64,
    limit: i64,
) -> StoreResult<Vec<StoredEvent>> {
    if after_global_position < 0 {
        return Err(StoreError::InvalidGlobalPosition {
            value: after_global_position,
        });
    }
    validate_limit(limit)?;

    let rows = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT
            global_position,
            stream_id,
            stream_revision,
            event_id,
            event_type,
            schema_version,
            payload,
            metadata,
            tenant_id,
            command_id,
            correlation_id,
            causation_id,
            recorded_at
        FROM events
        WHERE tenant_id = $1 AND global_position > $2
        ORDER BY global_position ASC
        LIMIT $3
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(after_global_position)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(EventRow::try_into).collect()
}
```

**Apply:** Keep all projection SQL parameterized. Batch application must begin one transaction, upsert read-model rows, then upsert `projector_offsets` inside the same transaction before commit. Expose repository methods from a `PostgresProjectionStore` wrapper, not directly from free SQL functions.

---

### `crates/es-store-postgres/src/lib.rs` (config, request-response)

**Analog:** `crates/es-store-postgres/src/lib.rs`

**Boundary and re-export pattern** (lines 1-22):

```rust
//! PostgreSQL-backed durable event-store boundary.
//!
//! This crate owns durable PostgreSQL storage for event appends, stream/global
//! reads, command deduplication results, snapshots, and rehydration DTOs. It is
//! not a runtime, adapter, projection worker, outbox dispatcher, broker client,
//! or disruptor execution crate.

mod error;
mod event_store;
pub mod ids;
mod models;
mod rehydrate;
mod sql;

pub use error::{StoreError, StoreResult};
pub use event_store::PostgresEventStore;
```

**Apply:** Add `mod projection;` and re-export only public projection repository/read-model DTOs. Keep lower-level SQL helpers private.

---

### `crates/es-store-postgres/Cargo.toml` (config, request-response)

**Analog:** `crates/es-runtime/Cargo.toml`

**Path dependency pattern** (lines 8-17):

```toml
[dependencies]
disruptor.workspace = true
es-core = { path = "../es-core" }
es-kernel = { path = "../es-kernel" }
es-projection = { path = "../es-projection" }
example-commerce = { path = "../example-commerce" }
futures.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
twox-hash.workspace = true
```

**Apply:** Add `es-projection = { path = "../es-projection" }` and `example-commerce = { path = "../example-commerce" }` only if `projection.rs` decodes concrete commerce event DTOs. Avoid pulling runtime or adapter crates into storage.

---

### `crates/es-store-postgres/tests/projections.rs` (test, batch + CRUD)

**Analog:** `crates/es-store-postgres/tests/global_reads.rs`

**Integration test helper pattern** (lines 1-9 and 11-39):

```rust
//! Durable global-position read integration tests.

mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_store_postgres::{AppendOutcome, AppendRequest, NewEvent, PostgresEventStore};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

fn tenant_id(value: &str) -> TenantId {
    TenantId::new(value).expect("valid tenant id")
}
```

**Append fixture pattern** (lines 58-80):

```rust
async fn append_one(
    store: &PostgresEventStore,
    tenant: TenantId,
    stream: StreamId,
    idempotency_key: &str,
    command_seed: u128,
    event_seed: u128,
) -> anyhow::Result<i64> {
    let outcome = store
        .append(append_request(
            tenant,
            stream,
            idempotency_key,
            command_seed,
            event_seed,
        ))
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append should commit");
    };

    Ok(committed.global_positions[0])
}
```

**Tenant isolation assertion pattern** (lines 168-190):

```rust
#[tokio::test]
async fn global_reads_are_scoped_by_tenant() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant_a = tenant_id("tenant-a");
    let tenant_b = tenant_id("tenant-b");

    let tenant_a_position = append_one(
        &store,
        tenant_a.clone(),
        stream_id("order-1"),
        "command-1",
        10,
        100,
    )
    .await?;
    append_one(&store, tenant_b, stream_id("order-1"), "command-1", 20, 200).await?;

    let events = store.read_global(&tenant_a, 0, 100).await?;

    assert_eq!(vec![tenant_a_position], global_positions(&events));
    assert!(events.iter().all(|event| event.tenant_id == tenant_a));

    Ok(())
}
```

**Harness pattern** from `crates/es-store-postgres/tests/common/mod.rs` (lines 10-22):

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

**Apply:** Tests should append committed events, run projection catch-up, assert read model rows and offsets in PostgreSQL, run catch-up twice to prove idempotence, and assert tenant-scoped isolation.

---

### `crates/example-commerce/Cargo.toml` (config, transform)

**Analog:** `crates/es-store-postgres/Cargo.toml`

**Workspace serde dependency pattern** (lines 8-15):

```toml
[dependencies]
es-core = { path = "../es-core" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
time.workspace = true
uuid.workspace = true
```

**Apply:** Add `serde.workspace = true` if Phase 5 chooses to serialize/deserialize concrete commerce event payloads directly. Keep `serde_json` out of example-commerce unless the domain crate itself must own JSON fixtures; storage projection can own JSON decoding.

---

### `crates/example-commerce/src/{ids,order,product}.rs` (model, transform)

**Analog:** `crates/es-store-postgres/src/models.rs`

**Serde derive pattern** (lines 11-24 and 130-158):

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NewEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StoredEvent {
    pub global_position: i64,
    pub stream_id: StreamId,
    pub stream_revision: StreamRevision,
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub tenant_id: TenantId,
}
```

**Commerce event shapes to decode** from `crates/example-commerce/src/order.rs` (lines 68-80):

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum OrderEvent {
    /// Order was placed.
    OrderPlaced { order_id: OrderId, user_id: UserId, lines: Vec<OrderLine> },
    /// Order was confirmed.
    OrderConfirmed { order_id: OrderId },
    /// Order was rejected.
    OrderRejected { order_id: OrderId, reason: String },
    /// Order was cancelled.
    OrderCancelled { order_id: OrderId },
}
```

**Product event shapes to decode** from `crates/example-commerce/src/product.rs` (lines 65-100):

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {
    ProductCreated {
        product_id: ProductId,
        sku: Sku,
        name: String,
        initial_quantity: Quantity,
    },
    InventoryAdjusted {
        product_id: ProductId,
        delta: i32,
    },
    InventoryReserved {
        product_id: ProductId,
        quantity: Quantity,
    },
    InventoryReleased {
        product_id: ProductId,
        quantity: Quantity,
    },
}
```

**Apply:** If deriving serde, add `serde::{Deserialize, Serialize}` imports and derive `Deserialize, Serialize` on IDs, `Quantity`, `OrderLine`, `OrderStatus`, `OrderEvent`, and `ProductEvent`. Keep domain behavior synchronous and free of SQLx/Tokio.

## Shared Patterns

### Tenant Scoping

**Source:** `crates/es-store-postgres/src/sql.rs`  
**Apply to:** projection offsets, read-model writes, read-model queries, integration tests

```rust
WHERE tenant_id = $1 AND global_position > $2
ORDER BY global_position ASC
LIMIT $3
```

Every query must bind `tenant_id.as_str()` and every table that stores projection state must include `tenant_id` in its primary key or lookup predicate.

### Atomic Offset + Read Model Commit

**Source:** `crates/es-store-postgres/src/sql.rs`  
**Apply to:** `crates/es-store-postgres/src/projection.rs`

```rust
let mut tx = pool.begin().await?;
// apply all row effects with .execute(&mut **tx)
// upsert projector offset with .execute(&mut **tx)
tx.commit().await?;
```

Offset update belongs in the same transaction as read-model writes. Do not update `projector_offsets` from a separate transaction or after commit.

### Parameterized SQL

**Source:** `crates/es-store-postgres/src/sql.rs`  
**Apply to:** every projection repository method

```rust
sqlx::query(
    r#"
    INSERT INTO streams (tenant_id, stream_id, revision)
    VALUES ($1, $2, $3)
    ON CONFLICT (tenant_id, stream_id)
    DO UPDATE SET revision = EXCLUDED.revision, updated_at = now()
    "#,
)
.bind(request.command_metadata.tenant_id.as_str())
.bind(request.stream_id.as_str())
.bind(last_revision)
```

No string-built SQL for event types, projector names, tenant IDs, or read-model keys.

### Typed Constructor Validation

**Source:** `crates/es-core/src/lib.rs` and `crates/es-store-postgres/src/models.rs`  
**Apply to:** `ProjectorName`, `MinimumGlobalPosition`, batch limits, read-model DTO constructors

```rust
let value = value.into();
if value.is_empty() {
    return Err(CoreError::EmptyValue { type_name });
}
Ok(value)
```

Projection public API should reject invalid names/positions before hitting PostgreSQL.

### Bounded Query Wait

**Source:** `crates/es-runtime/tests/runtime_flow.rs`  
**Apply to:** `crates/es-projection/src/query.rs`, `crates/es-projection/tests/minimum_position.rs`

```rust
tokio::time::timeout(Duration::from_millis(20), receiver)
    .await
    .is_err()
```

Minimum-position query support must have a deadline and return typed lag/timeout instead of blocking indefinitely.

### Containerized PostgreSQL Tests

**Source:** `crates/es-store-postgres/tests/common/mod.rs`  
**Apply to:** `crates/es-store-postgres/tests/projections.rs`

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

Use the existing harness; do not add SQLite, mocks, or a separate migration runner for projection repository integration tests.

## No Analog Found

No files are fully without analogs. `crates/es-projection/src/query.rs` has only a partial local analog for bounded waits; planner should rely on the Phase 5 research wait-policy contract plus Tokio timer APIs for implementation details.

## Metadata

**Analog search scope:** `crates/**/*.rs`, `crates/**/Cargo.toml`, `crates/es-store-postgres/migrations/*.sql`, Phase 05 research/validation docs  
**Files scanned:** 43 source/config/migration/test files from `crates/` plus 4 planning inputs  
**Pattern extraction date:** 2026-04-18

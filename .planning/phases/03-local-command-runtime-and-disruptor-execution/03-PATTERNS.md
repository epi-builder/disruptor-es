# Phase 03: Local Command Runtime and Disruptor Execution - Pattern Map

**Mapped:** 2026-04-17  
**Files analyzed:** 13 new/modified files  
**Analogs found:** 12 / 13

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `Cargo.toml` | config | transform | `Cargo.toml` | exact |
| `crates/es-runtime/Cargo.toml` | config | transform | `crates/es-store-postgres/Cargo.toml` | role-match |
| `crates/es-runtime/src/lib.rs` | config | transform | `crates/es-store-postgres/src/lib.rs` | exact |
| `crates/es-runtime/src/error.rs` | utility | request-response | `crates/es-store-postgres/src/error.rs` | exact |
| `crates/es-runtime/src/command.rs` | model | request-response | `crates/es-store-postgres/src/models.rs` | role-match |
| `crates/es-runtime/src/gateway.rs` | service | request-response | `crates/es-store-postgres/src/event_store.rs` | role-match |
| `crates/es-runtime/src/router.rs` | utility | transform | `crates/es-core/src/lib.rs` | role-match |
| `crates/es-runtime/src/shard.rs` | service | CRUD | `crates/es-kernel/src/lib.rs` | partial |
| `crates/es-runtime/src/disruptor_path.rs` | utility | event-driven | none | no-local-analog |
| `crates/es-runtime/src/cache.rs` | store | CRUD | `crates/es-kernel/src/lib.rs` | partial |
| `crates/es-runtime/src/store.rs` | service | CRUD | `crates/es-store-postgres/src/event_store.rs` | exact |
| `crates/es-runtime/tests/*.rs` | test | request-response | `crates/es-store-postgres/tests/append_occ.rs` | role-match |
| `crates/es-runtime/tests/common/mod.rs` or inline fakes | test | request-response | `crates/es-store-postgres/tests/common/mod.rs` | role-match |

## Pattern Assignments

### `Cargo.toml` (config, transform)

**Analog:** `Cargo.toml`

**Workspace dependency pattern** (lines 11-23):
```toml
[workspace.dependencies]
anyhow = "1.0.102"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "postgres", "uuid", "time", "json", "migrate"] }
testcontainers = "0.25.0"
testcontainers-modules = { version = "0.13.0", features = ["postgres"] }
thiserror = "2.0.18"
tokio = { version = "1.52.0", features = ["rt-multi-thread", "macros", "time"] }
uuid = { version = "1.23.0", features = ["serde", "v7"] }
time = { version = "=0.3.44", features = ["serde", "formatting", "parsing"] }
proptest = "1.11.0"
insta = "1.47.2"
```

**Lint pattern** (lines 25-27):
```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"
```

Apply by adding runtime dependencies to the workspace catalog rather than pinning unrelated versions in crate manifests. Phase research expects `disruptor`, `twox-hash`, and `tracing`; `tokio` may need the `sync` feature added to the existing workspace dependency.

---

### `crates/es-runtime/Cargo.toml` (config, transform)

**Analog:** `crates/es-runtime/Cargo.toml`, then follow populated crate manifests such as `crates/es-store-postgres/Cargo.toml`

**Crate manifest shell pattern** (lines 1-11):
```toml
[package]
name = "es-runtime"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]

[lints]
workspace = true
```

Keep package metadata inherited. Add only dependencies needed by the runtime crate: `es-core`, `es-kernel`, `es-store-postgres`, `thiserror`, `tokio`, `disruptor`, `twox-hash`, `tracing`, and optional test-only helpers if fake storage requires them.

---

### `crates/es-runtime/src/lib.rs` (config, transform)

**Analog:** `crates/es-store-postgres/src/lib.rs`

**Module/private implementation pattern** (lines 8-14):
```rust
mod error;
mod event_store;
/// Identifier generation helpers.
pub mod ids;
mod models;
mod rehydrate;
mod sql;
```

**Public re-export pattern** (lines 16-21):
```rust
pub use error::{StoreError, StoreResult};
pub use event_store::PostgresEventStore;
pub use ids::{IdGenerator, UuidV7Generator};
pub use models::{
    AppendOutcome, AppendRequest, CommittedAppend, MAX_JSON_PAYLOAD_BYTES, NewEvent,
    RehydrationBatch, SaveSnapshotRequest, SnapshotRecord, StoredEvent,
};
```

Apply by keeping runtime implementation modules private unless adapters need the type. Re-export `CommandGateway`, `CommandEnvelope`, `CommandOutcome`, `RuntimeError`, `RuntimeResult`, `PartitionRouter`, `ShardId`, and any store trait intended as public API.

---

### `crates/es-runtime/src/error.rs` (utility, request-response)

**Analog:** `crates/es-store-postgres/src/error.rs`

**Result alias and typed error enum pattern** (lines 1-8):
```rust
/// Result alias for PostgreSQL event-store operations.
pub type StoreResult<T> = Result<T, StoreError>;

/// Errors returned by the PostgreSQL event-store API.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Append request contained no events.
    #[error("append request must contain at least one event")]
```

**Structured conflict error pattern** (lines 30-39):
```rust
/// Stream revision did not match the requested optimistic-concurrency expectation.
#[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
StreamConflict {
    /// Conflicting stream identifier.
    stream_id: String,
    /// Expected revision description.
    expected: String,
    /// Actual stream revision, or `None` when the stream does not exist.
    actual: Option<u64>,
},
```

**Source-wrapping pattern** (lines 95-97):
```rust
/// SQLx returned a database error.
#[error("database error")]
Database(#[from] sqlx::Error),
```

Runtime errors should follow this shape: `RuntimeResult<T>`, `RuntimeError::Overloaded`, `Unavailable`, `ShardOverloaded { shard_id }`, `InvalidShardCount`, `Conflict { stream_id, expected, actual }`, `Domain(...)` if representable, and `Store(#[from] StoreError)`. Map `StoreError::StreamConflict` into the runtime conflict variant where the caller needs retry semantics.

---

### `crates/es-runtime/src/command.rs` (model, request-response)

**Analog:** `crates/es-store-postgres/src/models.rs`

**DTO struct pattern** (lines 63-76):
```rust
/// Request to append one or more events to a stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AppendRequest {
    /// Stream receiving the events.
    pub stream_id: StreamId,
    /// Optimistic-concurrency expectation for the stream.
    pub expected_revision: ExpectedRevision,
    /// Command metadata, including the tenant that owns the append.
    pub command_metadata: CommandMetadata,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
    /// Events to append atomically.
    pub events: Vec<NewEvent>,
}
```

**Validated constructor pattern** (lines 78-103):
```rust
impl AppendRequest {
    /// Creates a validated append request.
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

        Ok(Self {
            stream_id,
            expected_revision,
            command_metadata,
            idempotency_key,
            events,
        })
    }
}
```

**Outcome enum pattern** (lines 121-128):
```rust
/// Outcome of an append request after idempotency handling.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AppendOutcome {
    /// New events were committed.
    Committed(CommittedAppend),
    /// A prior result was returned for the same idempotency key.
    Duplicate(CommittedAppend),
}
```

Define `CommandEnvelope<A>` with command, metadata, idempotency key, reply sender, and routing fields. Define `CommandOutcome<R>` around typed reply plus `CommittedAppend` so callers receive durable stream/global positions, not ring sequence numbers.

---

### `crates/es-runtime/src/gateway.rs` (service, request-response)

**Analog:** `crates/es-store-postgres/src/event_store.rs`

**Thin public service wrapper pattern** (lines 6-16):
```rust
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
```

**Async boundary method pattern** (lines 23-30):
```rust
/// Appends events to a stream.
pub async fn append(&self, request: AppendRequest) -> StoreResult<AppendOutcome> {
    if request.events.is_empty() {
        return Err(StoreError::EmptyAppend);
    }

    sql::append(&self.pool, request).await
}
```

Gateway should expose the adapter-facing command submission API and hide queue internals. Use the research pattern for bounded `tokio::sync::mpsc::Sender::try_send`; no existing local queue analog exists. Do not use `send().await` in overload-sensitive paths.

---

### `crates/es-runtime/src/router.rs` (utility, transform)

**Analog:** `crates/es-core/src/lib.rs`

**Opaque newtype and constructor pattern** (lines 39-58):
```rust
/// Ordered partition routing key.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PartitionKey(String);

impl PartitionKey {
    /// Creates a partition key.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "PartitionKey").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}
```

**Validation helper pattern** (lines 81-87):
```rust
fn string_value(value: impl Into<String>, type_name: &'static str) -> Result<String, CoreError> {
    let value = value.into();
    if value.is_empty() {
        return Err(CoreError::EmptyValue { type_name });
    }
    Ok(value)
}
```

`ShardId` should be a small opaque value with accessors. `PartitionRouter::new(shard_count)` should reject zero, store algorithm/seed explicitly, and route using `TenantId::as_str()` plus `PartitionKey::as_str()`.

---

### `crates/es-runtime/src/shard.rs` (service, CRUD)

**Analog:** `crates/es-kernel/src/lib.rs`

**Aggregate contract pattern** (lines 19-50):
```rust
/// Deterministic aggregate contract implemented by domain aggregate types.
pub trait Aggregate {
    /// Aggregate state type.
    type State: Default + Clone + PartialEq;
    /// Command input type.
    type Command;
    /// Event output type.
    type Event: Clone;
    /// Reply type returned after a successful decision.
    type Reply;
    /// Domain error type returned by decisions.
    type Error;

    /// Returns the stream identifier affected by the command.
    fn stream_id(command: &Self::Command) -> es_core::StreamId;

    /// Returns the ordered partition key for the command.
    fn partition_key(command: &Self::Command) -> es_core::PartitionKey;

    /// Returns the expected stream revision for optimistic concurrency.
    fn expected_revision(command: &Self::Command) -> es_core::ExpectedRevision;

    /// Decides events and reply for a command against current state.
    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &es_core::CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error>;

    /// Applies one event to aggregate state.
    fn apply(state: &mut Self::State, event: &Self::Event);
}
```

**Replay/apply ordering pattern** (lines 52-59):
```rust
/// Replays events from a default aggregate state.
pub fn replay<A: Aggregate>(events: impl IntoIterator<Item = A::Event>) -> A::State {
    let mut state = A::State::default();
    for event in events {
        A::apply(&mut state, &event);
    }
    state
}
```

Shard processing should call `A::stream_id`, `A::expected_revision`, load/cache state, call `A::decide`, append to storage, then apply events to the cached state only after `AppendOutcome::Committed` or `AppendOutcome::Duplicate`.

**Storage conflict analog** (from `crates/es-store-postgres/tests/append_occ.rs` lines 269-281):
```rust
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

assert!(matches!(error, StoreError::StreamConflict { .. }));
```

Use this conflict behavior to keep cache unchanged when storage rejects the append.

---

### `crates/es-runtime/src/disruptor_path.rs` (utility, event-driven)

**Analog:** none in local codebase.

No crate currently imports or wraps `disruptor`. Planner should use `03-RESEARCH.md` patterns and schedule an early compile spike. The closest project-local boundary pattern is still the runtime facade in `crates/es-runtime/src/lib.rs` lines 1-5:
```rust
//! Local command routing, shard ownership, and in-process execution boundary.

/// Phase ownership marker for the runtime crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 3 owns local command routing, shard ownership, and in-process execution.";
```

Required external pattern from research:
```rust
match producer.try_publish(|slot| {
    *slot = runtime_event;
}) {
    Ok(sequence) => Ok(sequence),
    Err(disruptor::RingBufferFull) => Err(RuntimeError::ShardOverloaded { shard_id }),
}
```

Keep this module narrow: build ring/producers, publish non-blockingly, map full ring to typed overload, and never expose disruptor sequence numbers as durable event positions.

---

### `crates/es-runtime/src/cache.rs` (store, CRUD)

**Analog:** `crates/es-kernel/src/lib.rs`

**State mutation after apply pattern** (lines 52-59):
```rust
pub fn replay<A: Aggregate>(events: impl IntoIterator<Item = A::Event>) -> A::State {
    let mut state = A::State::default();
    for event in events {
        A::apply(&mut state, &event);
    }
    state
}
```

**Domain apply pattern** (from `crates/example-commerce/src/lib.rs` lines 123-130):
```rust
fn apply(state: &mut Self::State, event: &Self::Event) {
    match event {
        ProductEvent::ProductCreated { sku, name } => {
            state.sku = Some(sku.clone());
            state.name = Some(name.clone());
        }
    }
}
```

Cache should be shard-owned and stream-keyed. Use `HashMap` first unless planner adds explicit eviction requirements. Stage cloned state for decisions and write it back only after durable append success.

---

### `crates/es-runtime/src/store.rs` (service, CRUD)

**Analog:** `crates/es-store-postgres/src/event_store.rs`

**Runtime-facing storage methods to wrap or trait-adapt** (lines 23-30, 71-78):
```rust
/// Appends events to a stream.
pub async fn append(&self, request: AppendRequest) -> StoreResult<AppendOutcome> {
    if request.events.is_empty() {
        return Err(StoreError::EmptyAppend);
    }

    sql::append(&self.pool, request).await
}
```

```rust
/// Loads the latest snapshot and subsequent stream events.
pub async fn load_rehydration(
    &self,
    tenant_id: &es_core::TenantId,
    stream_id: &es_core::StreamId,
) -> StoreResult<RehydrationBatch> {
    rehydrate::load_rehydration(&self.pool, tenant_id, stream_id).await
}
```

**Rehydration helper pattern** (from `crates/es-store-postgres/src/rehydrate.rs` lines 6-22):
```rust
pub(crate) async fn load_rehydration(
    pool: &PgPool,
    tenant_id: &TenantId,
    stream_id: &StreamId,
) -> StoreResult<RehydrationBatch> {
    let snapshot = sql::load_latest_snapshot(pool, tenant_id, stream_id).await?;
    let after_revision = snapshot
        .as_ref()
        .map(|record| record.stream_revision.value())
        .unwrap_or(0);
    let after_revision = i64::try_from(after_revision)
        .map_err(|_| crate::StoreError::InvalidStoredRevision { value: i64::MAX })?;

    let events =
        sql::read_stream_after(pool, tenant_id, stream_id, after_revision, i64::MAX).await?;

    Ok(RehydrationBatch { snapshot, events })
}
```

If Phase 3 introduces a trait for fake stores, keep it tiny: append and rehydrate only. Do not move PostgreSQL transaction logic into runtime.

---

### `crates/es-runtime/tests/*.rs` (test, request-response)

**Analog:** `crates/es-store-postgres/tests/append_occ.rs`

**Integration test imports and helper pattern** (lines 3-11, 21-40):
```rust
mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_store_postgres::{AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, StoreError};
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;
```

```rust
fn command_metadata(tenant: TenantId, seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: Some(Uuid::from_u128(seed + 2)),
        tenant_id: tenant,
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .expect("valid requested_at"),
    }
}

fn new_event(seed: u128, event_type: &str, order_id: &str) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        json!({ "order_id": order_id }),
        json!({ "source": "append_occ" }),
    )
    .expect("valid event")
}
```

**Async test shape** (lines 61-80):
```rust
#[tokio::test]
async fn first_append_commits_event_with_no_stream_expectation() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    let outcome = store
        .append(append_request(
            tenant_id("tenant-a"),
            stream_id("order-1"),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![new_event(100, "OrderPlaced", "order-1")],
        ))
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("first append should commit new events");
    };
```

Runtime tests should include unit tests for router golden mappings, bounded ingress overload, full ring overload, conflict leaves cache unchanged, and reply-after-commit. Prefer fake stores for unit tests unless a behavior depends on PostgreSQL.

---

### `crates/es-runtime/tests/common/mod.rs` or inline fakes (test, request-response)

**Analog:** `crates/es-store-postgres/tests/common/mod.rs`

**Shared async harness pattern** (lines 5-27):
```rust
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

Use a local fake store harness for reply/cache tests when possible. Reuse the PostgreSQL harness only for runtime-store integration tests that need real append/dedupe/OCC semantics.

## Shared Patterns

### Public Crate Facade
**Source:** `crates/es-store-postgres/src/lib.rs`  
**Apply to:** `crates/es-runtime/src/lib.rs`
```rust
mod error;
mod event_store;
mod models;

pub use error::{StoreError, StoreResult};
pub use event_store::PostgresEventStore;
pub use models::{AppendOutcome, AppendRequest, CommittedAppend};
```

Keep internals private by default and re-export the adapter-facing runtime contract.

### Typed Errors
**Source:** `crates/es-store-postgres/src/error.rs`  
**Apply to:** `error.rs`, `gateway.rs`, `shard.rs`, `disruptor_path.rs`
```rust
pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
    StreamConflict {
        stream_id: String,
        expected: String,
        actual: Option<u64>,
    },
}
```

Prefer structured variants over string errors. Preserve source errors with `#[from]` when the variant is a transparent boundary.

### Core Identity Accessors
**Source:** `crates/es-core/src/lib.rs`  
**Apply to:** `router.rs`, `command.rs`, `shard.rs`
```rust
impl TenantId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "TenantId").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

Route with `tenant_id.as_str()` and `partition_key.as_str()`; do not parse or expose inner strings except through existing constructors/accessors.

### Aggregate Decide/Apply
**Source:** `crates/es-kernel/src/lib.rs`  
**Apply to:** `shard.rs`, `cache.rs`
```rust
fn decide(
    state: &Self::State,
    command: Self::Command,
    metadata: &es_core::CommandMetadata,
) -> Result<Decision<Self::Event, Self::Reply>, Self::Error>;

fn apply(state: &mut Self::State, event: &Self::Event);
```

Keep domain decisions synchronous and deterministic. Runtime may do async storage before/after, but not inside aggregate code.

### Append Commit Boundary
**Source:** `crates/es-store-postgres/src/event_store.rs`  
**Apply to:** `store.rs`, `shard.rs`, `gateway.rs`
```rust
pub async fn append(&self, request: AppendRequest) -> StoreResult<AppendOutcome> {
    if request.events.is_empty() {
        return Err(StoreError::EmptyAppend);
    }

    sql::append(&self.pool, request).await
}
```

Reply to callers only after this returns `AppendOutcome::Committed` or `AppendOutcome::Duplicate`.

### Test Fixtures
**Source:** `crates/es-store-postgres/tests/append_occ.rs`  
**Apply to:** runtime unit and integration tests
```rust
fn command_metadata(tenant: TenantId, seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: Some(Uuid::from_u128(seed + 2)),
        tenant_id: tenant,
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .expect("valid requested_at"),
    }
}
```

Use deterministic IDs and timestamps in tests.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/es-runtime/src/disruptor_path.rs` | utility | event-driven | No existing crate imports or wraps `disruptor`; use `03-RESEARCH.md` docs-derived patterns and compile spike. |

## Metadata

**Analog search scope:** `Cargo.toml`, `crates/*/src/**/*.rs`, `crates/*/tests/**/*.rs`  
**Files scanned:** 31  
**Pattern extraction date:** 2026-04-17  
**Project instructions:** No repo-local `CLAUDE.md`; no project-local `.claude/skills` or `.agents/skills`; applied provided AGENTS/project instructions.  
**Primary analog crates:** `es-core`, `es-kernel`, `es-store-postgres`, `example-commerce`.

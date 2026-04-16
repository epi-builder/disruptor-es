# Phase 02: Durable Event Store Source of Truth - Pattern Map

**Mapped:** 2026-04-17
**Files analyzed:** 15
**Analogs found:** 10 / 15 local analogs, 5 / 15 research-only analogs

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `Cargo.toml` | config | build/workspace | `Cargo.toml` | exact |
| `crates/es-store-postgres/Cargo.toml` | config | build/crate | `crates/example-commerce/Cargo.toml` | role-match |
| `crates/es-store-postgres/src/lib.rs` | service/API facade | request-response | `crates/es-store-postgres/src/lib.rs` | exact boundary |
| `crates/es-store-postgres/src/error.rs` | utility/error | transform | `crates/es-core/src/lib.rs` | exact |
| `crates/es-store-postgres/src/models.rs` | model | transform | `crates/es-core/src/lib.rs` | exact |
| `crates/es-store-postgres/src/ids.rs` | utility | transform | `crates/es-core/src/lib.rs` | role-match |
| `crates/es-store-postgres/src/event_store.rs` | service | CRUD/request-response | `02-RESEARCH.md` lines 100-131 | research-only |
| `crates/es-store-postgres/src/sql.rs` | service/utility | CRUD | `02-RESEARCH.md` lines 157-238 | research-only |
| `crates/es-store-postgres/src/rehydrate.rs` | utility/service | read/transform | `02-RESEARCH.md` lines 240-259 | research-only |
| `crates/es-store-postgres/migrations/20260417000000_event_store.sql` | migration | schema | `02-RESEARCH.md` lines 211-235 | research-only |
| `crates/es-store-postgres/tests/common/mod.rs` | test utility | setup/file-I/O | `02-RESEARCH.md` lines 377-393 | research-only |
| `crates/es-store-postgres/tests/append_occ.rs` | test | CRUD/integration | `crates/example-commerce/src/lib.rs` | role-match |
| `crates/es-store-postgres/tests/dedupe.rs` | test | CRUD/integration | `crates/example-commerce/src/lib.rs` | role-match |
| `crates/es-store-postgres/tests/snapshots.rs` | test | read/transform/integration | `crates/es-kernel/src/lib.rs` | role-match |
| `crates/es-store-postgres/tests/global_reads.rs` | test | batch/read/integration | `crates/example-commerce/tests/dependency_boundaries.rs` | role-match |

## Pattern Assignments

### `Cargo.toml` (config, build/workspace)

**Analog:** `Cargo.toml`

**Workspace dependency catalog pattern** (lines 1-18):

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
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
thiserror = "2.0.18"
uuid = { version = "1.23.0", features = ["serde", "v7"] }
time = { version = "=0.3.44", features = ["serde", "formatting", "parsing"] }
```

**Apply:** Add workspace versions for storage dependencies here before inheriting them in `es-store-postgres`: `sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "postgres", "uuid", "time", "json", "migrate"] }`, `tokio = { version = "1.52.0", features = ["rt-multi-thread", "macros", "time"] }`, and dev-only candidates `anyhow`, `testcontainers = "0.25.0"`, `testcontainers-modules = { version = "0.13.0", features = ["postgres"] }`. Do not upgrade the Rust floor beyond `1.85`.

---

### `crates/es-store-postgres/Cargo.toml` (config, build/crate)

**Analog:** `crates/example-commerce/Cargo.toml`

**Member manifest pattern** (lines 1-19):

```toml
[package]
name = "example-commerce"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
es-core = { path = "../es-core" }
es-kernel = { path = "../es-kernel" }
thiserror.workspace = true

[dev-dependencies]
proptest.workspace = true
time.workspace = true
uuid.workspace = true

[lints]
workspace = true
```

**Apply:** Keep package inheritance and workspace lints. Add only storage dependencies: `es-core`, `serde`, `serde_json`, `sqlx`, `thiserror`, `time`, `uuid`; `tokio`, `anyhow`, `testcontainers`, and `testcontainers-modules` belong in dev dependencies unless production async helpers need `tokio` directly.

---

### `crates/es-store-postgres/src/lib.rs` (service/API facade, request-response)

**Analog:** `crates/es-store-postgres/src/lib.rs`

**Current boundary marker** (lines 1-4):

```rust
//! Durable event append and event-store transaction boundary.

/// Phase ownership marker for the durable event-store crate.
pub const PHASE_BOUNDARY: &str = "Phase 2 owns durable event append and transaction contracts.";
```

**Apply:** Replace the placeholder with a documented public facade that declares modules and re-exports storage API types. Keep the crate focused on durable event append, stream/global reads, snapshots, dedupe, and PostgreSQL implementation. Do not expose runtime command execution, disruptor, adapters, projectors, or broker publication.

**Facade shape to copy:**

```rust
//! PostgreSQL-backed durable event store.

mod error;
mod event_store;
mod ids;
mod models;
mod rehydrate;
mod sql;

pub use error::{StoreError, StoreResult};
pub use event_store::PostgresEventStore;
pub use models::{AppendRequest, AppendOutcome, CommittedAppend, NewEvent, StoredEvent};
```

---

### `crates/es-store-postgres/src/error.rs` (utility/error, transform)

**Analog:** `crates/es-core/src/lib.rs`

**Typed error pattern** (lines 7-16):

```rust
/// Errors returned by core value constructors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CoreError {
    /// A required string-backed value was empty.
    #[error("{type_name} cannot be empty")]
    EmptyValue {
        /// Name of the value type that rejected the empty input.
        type_name: &'static str,
    },
}
```

**Apply:** Use `thiserror` for public storage errors. Include variants for empty append, stream conflict, duplicate/invalid dedupe state, invalid stored revision/position conversion, and SQLx infrastructure errors. Keep domain errors out of this crate.

**Error handling shape:**

```rust
/// Result alias for PostgreSQL event-store operations.
pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("append request must contain at least one event")]
    EmptyAppend,
    #[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
    StreamConflict {
        stream_id: String,
        expected: String,
        actual: Option<u64>,
    },
    #[error("database error")]
    Database(#[from] sqlx::Error),
}
```

---

### `crates/es-store-postgres/src/models.rs` (model, transform)

**Analog:** `crates/es-core/src/lib.rs`

**Core type import and model style** (lines 3-5, 89-146):

```rust
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Ordered stream revision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StreamRevision(u64);

/// Optimistic concurrency expectation for appending to a stream.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ExpectedRevision {
    Any,
    NoStream,
    Exact(StreamRevision),
}

/// Metadata committed with a recorded event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventMetadata {
    pub event_id: Uuid,
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: TenantId,
    pub recorded_at: OffsetDateTime,
}
```

**Apply:** Reuse `es_core::{CommandMetadata, EventMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId}` directly. For storage-specific rows, mirror the `Clone, Debug, Serialize, Deserialize, Eq/PartialEq` style where possible. Use `serde_json::Value` only at the storage serialization boundary for payload/metadata, not in the domain/kernel API.

**Model shape:**

```rust
pub struct AppendRequest {
    pub stream_id: StreamId,
    pub expected_revision: ExpectedRevision,
    pub command_metadata: CommandMetadata,
    pub idempotency_key: String,
    pub events: Vec<NewEvent>,
}

pub struct NewEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

pub struct CommittedAppend {
    pub stream_id: StreamId,
    pub first_revision: StreamRevision,
    pub last_revision: StreamRevision,
    pub global_positions: Vec<i64>,
    pub event_ids: Vec<Uuid>,
}
```

---

### `crates/es-store-postgres/src/ids.rs` (utility, transform)

**Analog:** `crates/es-core/src/lib.rs`

**UUID-backed metadata pattern** (lines 116-146):

```rust
pub struct CommandMetadata {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: TenantId,
    pub requested_at: OffsetDateTime,
}

pub struct EventMetadata {
    pub event_id: Uuid,
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: TenantId,
    pub recorded_at: OffsetDateTime,
}
```

**Apply:** Add a tiny project-owned UUID helper so event IDs can be generated in Rust and tests can override deterministic IDs. Avoid DB-side `uuidv7()` as the primary path per `02-CONTEXT.md` lines 41-44.

**Core helper shape:**

```rust
/// Generates ordered UUIDs for event-store records.
pub trait IdGenerator {
    fn new_event_id(&self) -> uuid::Uuid;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UuidV7Generator;

impl IdGenerator for UuidV7Generator {
    fn new_event_id(&self) -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }
}
```

---

### `crates/es-store-postgres/src/event_store.rs` (service, CRUD/request-response)

**Analog:** `02-RESEARCH.md`

**Append transaction flow** (lines 100-131):

```text
AppendRequest { stream_id, expected_revision, metadata, events, idempotency_key }
  v
EventStore::append
  v
PostgreSQL transaction
  +--> Check command_dedup by (tenant_id, idempotency_key)
  +--> Lock/update stream revision according to ExpectedRevision
  +--> Insert events with global_position identity and full metadata
  +--> Insert command_dedup committed result
  +--> Optional: insert/update snapshot when caller requests snapshot write
  v
Commit transaction
  v
CommittedAppend { stream_id, first_revision, last_revision, global_positions, event_ids }
```

**Apply:** Implement `PostgresEventStore` as the public async storage service over `sqlx::PgPool`. Public methods should be storage-level only: `append`, `read_stream`, `read_global`, `save_snapshot`, `load_latest_snapshot`, and `load_rehydration`. Reject new empty appends before starting or before mutating a transaction.

**Boundary guard from context** (lines 29-31):

```text
- `es-store-postgres` exposes storage-level APIs: append events, read stream events, read events by global position, save/load snapshots, and return committed append results.
- `es-store-postgres` does not execute aggregate `decide`, own shard-local aggregate caches, run disruptor processors, implement adapter behavior, or publish to brokers.
- Rehydration support should provide the latest snapshot plus subsequent stream events. Applying those events to typed aggregate state remains a kernel/runtime responsibility.
```

---

### `crates/es-store-postgres/src/sql.rs` (service/utility, CRUD)

**Analog:** `02-RESEARCH.md`

**SQLx transaction pattern** (lines 165-179):

```rust
let mut tx = pool.begin().await?;

let current = sqlx::query_scalar!(
    "SELECT revision FROM streams WHERE tenant_id = $1 AND stream_id = $2 FOR UPDATE",
    tenant_id,
    stream_id,
)
.fetch_optional(&mut *tx)
.await?;

// Validate ExpectedRevision in Rust, then insert events and update streams.

tx.commit().await?;
```

**Dedupe pattern** (lines 190-198):

```sql
INSERT INTO command_dedup (
    tenant_id, idempotency_key, stream_id, first_revision, last_revision,
    first_global_position, last_global_position, response_payload
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
RETURNING stream_id, first_revision, last_revision, first_global_position, last_global_position;
```

**Apply:** Keep explicit SQL helper functions private to the crate. Use SQLx bind parameters/macros, not string interpolation. After `ON CONFLICT DO NOTHING` returns no row, issue a follow-up `SELECT` for the dedupe key inside the transaction or retry as research warns in lines 294-300.

---

### `crates/es-store-postgres/src/rehydrate.rs` (utility/service, read/transform)

**Analog:** `02-RESEARCH.md`

**Snapshot plus subsequent events query pattern** (lines 248-259):

```sql
SELECT stream_revision, state_payload, metadata
FROM snapshots
WHERE tenant_id = $1 AND stream_id = $2
ORDER BY stream_revision DESC
LIMIT 1;

SELECT stream_revision, event_type, schema_version, payload, metadata
FROM events
WHERE tenant_id = $1 AND stream_id = $2 AND stream_revision > $3
ORDER BY stream_revision ASC;
```

**Apply:** Return a storage DTO containing `Option<SnapshotRecord>` and ordered `Vec<StoredEvent>`. Do not call `es_kernel::replay` here; applying events to typed aggregate state belongs to kernel/runtime per `02-CONTEXT.md` lines 29-31.

---

### `crates/es-store-postgres/migrations/20260417000000_event_store.sql` (migration, schema)

**Analog:** `02-RESEARCH.md`

**Append-only schema pattern** (lines 211-235):

```sql
CREATE TABLE streams (
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    revision bigint NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, stream_id)
);

CREATE TABLE events (
    global_position bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    stream_revision bigint NOT NULL,
    command_id uuid NOT NULL,
    correlation_id uuid NOT NULL,
    causation_id uuid NULL,
    event_type text NOT NULL,
    schema_version integer NOT NULL,
    payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, stream_id, stream_revision)
);
```

**Apply:** Extend this schema with `command_dedup` and `snapshots`. Every tenant-owned table should include `tenant_id`, and all stream reads/writes should key by `(tenant_id, stream_id)`. Use `jsonb`, `timestamptz`, identity global positions, unique constraints, indexes, `ON CONFLICT`, and `RETURNING` per `02-CONTEXT.md` lines 41-44.

---

### `crates/es-store-postgres/tests/common/mod.rs` (test utility, setup/file-I/O)

**Analog:** `02-RESEARCH.md`

**Testcontainers harness pattern** (lines 381-390):

```rust
use testcontainers_modules::{postgres, testcontainers::runners::SyncRunner};

#[test]
fn starts_postgres_for_store_tests() {
    let container = postgres::Postgres::default().start().unwrap();
    let host_port = container.get_host_port_ipv4(5432).unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{host_port}/postgres");

    assert!(database_url.contains("postgres://"));
}
```

**Apply:** Create a reusable async test fixture that starts PostgreSQL, builds a `PgPool`, runs migrations, and returns the pool plus container handle. Keep container ownership alive for the full test. Use Rust-1.85-compatible `testcontainers` 0.25.0 and `testcontainers-modules` 0.13.0 per research lines 399-402.

---

### `crates/es-store-postgres/tests/append_occ.rs` (test, CRUD/integration)

**Analog:** `crates/example-commerce/src/lib.rs`

**Fixture construction pattern** (lines 141-157):

```rust
fn metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: TenantId::new("tenant-a").expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn create_command(sku: impl Into<String>, name: impl Into<String>) -> ProductCommand {
    ProductCommand::CreateProduct {
        stream_id: StreamId::new("product-1").expect("stream id"),
        sku: sku.into(),
        name: name.into(),
    }
}
```

**Assertion pattern** (lines 159-178):

```rust
#[test]
fn decide_valid_create_returns_event_and_reply() {
    let command = create_command("SKU-1", "Keyboard");
    let decision =
        ProductDraft::decide(&ProductState::default(), command, &metadata()).expect("decision");

    assert_eq!(
        vec![ProductEvent::ProductCreated {
            sku: "SKU-1".to_owned(),
            name: "Keyboard".to_owned(),
        }],
        decision.events
    );
}
```

**Apply:** Use small deterministic fixture builders for stream IDs, metadata, new events, and append requests. Tests must use real PostgreSQL. Cover first append success, multi-event revision assignment, wrong expected revision conflict, `ExpectedRevision::NoStream`, `ExpectedRevision::Exact`, full metadata columns, and rollback after failed append.

---

### `crates/es-store-postgres/tests/dedupe.rs` (test, CRUD/integration)

**Analog:** `crates/example-commerce/src/lib.rs`

**Error assertion pattern** (lines 180-200):

```rust
#[test]
fn decide_rejects_empty_sku_and_name() {
    assert_eq!(
        ProductError::EmptySku,
        ProductDraft::decide(
            &ProductState::default(),
            create_command("", "Keyboard"),
            &metadata()
        )
        .expect_err("empty sku")
    );
}
```

**Apply:** Assert duplicate tenant/idempotency append returns the original committed result and does not insert additional event rows. Add a cross-tenant case proving the same idempotency key is isolated by `tenant_id`. Include a concurrent duplicate case if the fixture supports it; if not, leave it as an explicit planner task.

---

### `crates/es-store-postgres/tests/snapshots.rs` (test, read/transform/integration)

**Analog:** `crates/es-kernel/src/lib.rs`

**Replay ordering pattern** (lines 52-59, 142-150):

```rust
/// Replays events from a default aggregate state.
pub fn replay<A: Aggregate>(events: impl IntoIterator<Item = A::Event>) -> A::State {
    let mut state = A::State::default();
    for event in events {
        A::apply(&mut state, &event);
    }
    state
}

#[test]
fn replay_applies_events_in_order() {
    let state = replay::<CounterAggregate>([
        CounterEvent::Added(2),
        CounterEvent::Added(3),
        CounterEvent::Added(-1),
    ]);

    assert_eq!(CounterState { value: 4 }, state);
}
```

**Apply:** Snapshot tests should verify storage returns the latest snapshot and only events after that snapshot revision, ordered by stream revision. Do not require the storage crate to apply events into aggregate state.

---

### `crates/es-store-postgres/tests/global_reads.rs` (test, batch/read/integration)

**Analog:** `crates/example-commerce/tests/dependency_boundaries.rs`

**Batch assertion pattern** (lines 67-85):

```rust
#[test]
fn required_workspace_members_exist() {
    let root = workspace_root();
    for member in [
        "es-core",
        "es-kernel",
        "es-runtime",
        "es-store-postgres",
        "es-projection",
        "es-outbox",
        "example-commerce",
        "adapter-http",
        "adapter-grpc",
        "app",
    ] {
        assert!(
            root.join("crates").join(member).is_dir(),
            "missing workspace member directory: {member}"
        );
    }
}
```

**Research query pattern** (`02-RESEARCH.md` lines 358-372):

```rust
let rows = sqlx::query!(
    r#"
    SELECT global_position, event_id, tenant_id, stream_id, stream_revision,
           command_id, correlation_id, causation_id, event_type, schema_version,
           payload, metadata, recorded_at
    FROM events
    WHERE global_position > $1
    ORDER BY global_position ASC
    LIMIT $2
    "#,
    after_position,
    batch_size
)
.fetch_all(pool)
.await?;
```

**Apply:** Append across multiple streams, then assert global reads return committed events ordered by `global_position`, respect `after_position`, respect `LIMIT`, and include tenant predicates where applicable.

## Shared Patterns

### Boundary Strictness

**Source:** `02-CONTEXT.md` lines 8-12 and 29-31
**Apply to:** All `crates/es-store-postgres/src/*` files

```text
Phase 02 implements the durable PostgreSQL event-store source of truth for command success.
This phase does not implement disruptor command execution, shard-local aggregate caches, HTTP/gRPC adapters, projector runtimes, outbox dispatchers, or commerce workflow behavior.
```

Do not add dependencies on `es-kernel`, `es-runtime`, adapters, projection, outbox publishers, broker clients, or disruptor to the storage crate unless a later phase explicitly changes the boundary.

### Core Type Reuse

**Source:** `02-CONTEXT.md` lines 88-92 and `crates/es-core/src/lib.rs` lines 18-146
**Apply to:** `models.rs`, `event_store.rs`, `sql.rs`, tests

```rust
pub struct StreamId(String);
pub struct TenantId(String);
pub struct StreamRevision(u64);
pub enum ExpectedRevision {
    Any,
    NoStream,
    Exact(StreamRevision),
}
pub struct CommandMetadata {
    pub command_id: Uuid,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: TenantId,
    pub requested_at: OffsetDateTime,
}
```

Storage APIs should accept and return these types rather than duplicating string/UUID structs.

### Transaction and Dedupe

**Source:** `02-RESEARCH.md` lines 157-201 and 294-300
**Apply to:** `event_store.rs`, `sql.rs`, `append_occ.rs`, `dedupe.rs`

```rust
let mut tx = pool.begin().await?;
// dedupe lookup/claim
// stream revision lock and ExpectedRevision validation
// event inserts
// command_dedup result insert
tx.commit().await?;
```

Use one PostgreSQL transaction for append success. Repeated tenant/idempotency keys must return the prior committed result without duplicate event rows. After `ON CONFLICT DO NOTHING` returns no row, select the existing dedupe row inside the transaction or retry.

### Validation

**Source:** `crates/es-core/src/lib.rs` lines 22-87 and `02-CONTEXT.md` lines 33-37
**Apply to:** `models.rs`, `event_store.rs`, tests

```rust
pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
    string_value(value, "StreamId").map(Self)
}

if value.is_empty() {
    return Err(CoreError::EmptyValue { type_name });
}
```

Use core constructors for IDs. Storage must reject new empty appends with a typed `StoreError::EmptyAppend`; no-op command replies are a future runtime concern.

### Real PostgreSQL Tests

**Source:** `02-CONTEXT.md` lines 46-51 and `02-RESEARCH.md` lines 377-393
**Apply to:** All `crates/es-store-postgres/tests/*.rs`

```rust
let container = postgres::Postgres::default().start().unwrap();
let host_port = container.get_host_port_ipv4(5432).unwrap();
let database_url = format!("postgres://postgres:postgres@127.0.0.1:{host_port}/postgres");
```

Do not replace STORE-01 through STORE-05 with SQLite, mocks, or in-memory substitutes. Unit tests are fine for pure validation and error mapping only.

## No Local Analog Found

These files have no close implementation analog in the current codebase. Planner should use `02-RESEARCH.md` patterns directly.

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/es-store-postgres/src/event_store.rs` | service | CRUD/request-response | No existing async SQLx service implementation exists. |
| `crates/es-store-postgres/src/sql.rs` | service/utility | CRUD | No existing SQL helper or PostgreSQL transaction code exists. |
| `crates/es-store-postgres/src/rehydrate.rs` | utility/service | read/transform | No existing snapshot/read-after-revision implementation exists. |
| `crates/es-store-postgres/migrations/20260417000000_event_store.sql` | migration | schema | No migrations directory or SQL schema files exist yet. |
| `crates/es-store-postgres/tests/common/mod.rs` | test utility | setup/file-I/O | No Testcontainers fixture exists yet. |

## Metadata

**Analog search scope:** `Cargo.toml`, `rust-toolchain.toml`, `crates/**/*.rs`, `crates/**/Cargo.toml`, prior Phase 01 pattern map, Phase 02 context/research
**Files scanned:** 20 source/config/test files plus Phase 02 planning artifacts
**Pattern extraction date:** 2026-04-17

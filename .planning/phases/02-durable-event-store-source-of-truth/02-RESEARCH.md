# Phase 2: Durable Event Store Source of Truth - Research

**Researched:** 2026-04-17 [VERIFIED: local environment date]
**Domain:** Rust/PostgreSQL event-store implementation for event sourcing [VERIFIED: .planning/ROADMAP.md]
**Confidence:** HIGH for architecture and schema patterns; MEDIUM for exact crate pins where latest crates exceed the workspace Rust floor [VERIFIED: cargo info]

## User Constraints

`02-CONTEXT.md` exists and supplies locked Phase 02 decisions D-01 through D-18. This research is aligned to those decisions: PostgreSQL is the only v1 durable event-store backend (D-01), SQLite/mocks/in-memory substitutes cannot replace PostgreSQL acceptance tests (D-02, D-04, D-15), storage rejects new empty appends (D-10), PostgreSQL 18 is the default development/integration-test target (D-11), and UUIDs are generated in Rust rather than through DB-side `uuidv7()` defaults (D-14). [VERIFIED: .planning/phases/02-durable-event-store-source-of-truth/02-CONTEXT.md]

Phase constraints still apply: Rust-first service template; event store is the source of truth; disruptor rings must not be durable state; external publication later flows through outbox rows committed with domain events; prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: .planning/PROJECT.md] [VERIFIED: user prompt]

## Summary

Phase 2 should implement a project-owned PostgreSQL event store in `es-store-postgres`, not adopt a generic CQRS framework. [VERIFIED: .planning/PROJECT.md] [VERIFIED: .planning/REQUIREMENTS.md] The event-store transaction should be the command success boundary and should atomically handle stream optimistic concurrency, event rows, command deduplication, snapshot writes when requested, and later outbox hooks. [VERIFIED: .planning/REQUIREMENTS.md] [CITED: https://docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html]

Use PostgreSQL tables and constraints to enforce correctness: `streams` owns the current stream revision, `events` owns append-only event records and global positions, `command_dedup` owns tenant/idempotency replay, and `snapshots` owns aggregate rehydration checkpoints. [VERIFIED: .planning/REQUIREMENTS.md] [CITED: https://www.postgresql.org/docs/18/ddl-constraints.html] Do not derive projector/outbox ordering from disruptor sequence numbers; persisted `events.global_position` is the only global catch-up cursor. [VERIFIED: .planning/REQUIREMENTS.md]

**Primary recommendation:** Use `sqlx` 0.8.6 with PostgreSQL, explicit SQL, migrations, and real PostgreSQL integration tests through Rust-1.85-compatible `testcontainers` 0.25.0 / `testcontainers-modules` 0.13.0. [VERIFIED: cargo info sqlx@0.8.6] [VERIFIED: cargo info testcontainers] [VERIFIED: rust-toolchain.toml]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Durable event append | Database / Storage | API / Backend | PostgreSQL owns atomic persistence; Rust storage code owns transaction orchestration and typed errors. [VERIFIED: .planning/REQUIREMENTS.md] |
| Stream optimistic concurrency | Database / Storage | API / Backend | Unique constraints and transaction updates enforce revision correctness; Rust maps conflicts into typed errors. [CITED: https://www.postgresql.org/docs/18/ddl-constraints.html] |
| Command deduplication | Database / Storage | API / Backend | `(tenant_id, idempotency_key)` uniqueness must be durable and cross-process, not shard-local memory. [VERIFIED: .planning/REQUIREMENTS.md] |
| Event metadata storage | Database / Storage | API / Backend | The event table must persist command/correlation/causation/tenant/type/schema/payload/metadata/timestamp for inspection and replay. [VERIFIED: .planning/REQUIREMENTS.md] |
| Snapshot storage and rehydration | Database / Storage | API / Backend | PostgreSQL stores snapshots; Rust storage loads the latest snapshot plus later events while aggregate replay remains kernel/runtime responsibility per D-07. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/phases/02-durable-event-store-source-of-truth/02-CONTEXT.md] |
| Global-position reads | Database / Storage | Projection / Outbox workers later | Committed global positions are read by future projectors/outbox workers independent of ring sequences. [VERIFIED: .planning/REQUIREMENTS.md] |

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STORE-01 | Command handling can append domain events to a durable event store with per-stream optimistic concurrency. | Use `streams` revision update plus `events(stream_id, stream_revision)` uniqueness in one SQLx transaction. [VERIFIED: .planning/REQUIREMENTS.md] [CITED: https://www.postgresql.org/docs/18/ddl-constraints.html] |
| STORE-02 | Event store records include event ID, stream ID, stream revision, global position, command ID, causation ID, correlation ID, tenant ID, event type, schema version, payload, metadata, and recorded timestamp. | Define `events` columns directly; store payload/metadata as `jsonb`; use PostgreSQL `timestamptz` and UUIDs. [VERIFIED: .planning/REQUIREMENTS.md] [CITED: https://www.postgresql.org/docs/18/datatype-json.html] |
| STORE-03 | Command deduplication returns the prior committed result for a repeated tenant/idempotency key. | Use a durable `command_dedup` table with unique `(tenant_id, idempotency_key)` and stored committed result. [VERIFIED: .planning/REQUIREMENTS.md] [CITED: https://www.postgresql.org/docs/18/sql-insert.html] |
| STORE-04 | Aggregate rehydration can load the latest snapshot and replay subsequent stream events. | Use `snapshots` keyed by stream and revision, then query `events WHERE stream_revision > snapshot_revision ORDER BY stream_revision`. [VERIFIED: .planning/REQUIREMENTS.md] |
| STORE-05 | Event store exposes global-position reads for projector and outbox catch-up. | Query `events WHERE global_position > $last ORDER BY global_position LIMIT $batch`. [VERIFIED: .planning/REQUIREMENTS.md] |

## Project Constraints

No `CLAUDE.md` exists in the project root. [VERIFIED: filesystem check]

Actionable project directives from supplied `AGENTS.md` and project docs: prefer `pnpm` for Node tooling; prefer `uv` for Python tooling; keep Rust as the core implementation language; never treat disruptor rings as durable state; avoid shared mutable hot business state in adapter handlers; commit external publication through later outbox rows in the same transaction as domain events; separate performance tests by layer. [VERIFIED: user-supplied AGENTS.md] [VERIFIED: .planning/PROJECT.md]

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| PostgreSQL | 18.x target; tests may run `postgres:18` or project-selected compatible image | Durable event store, stream revisions, command dedupe, snapshots, global-position reads | PostgreSQL provides ACID transactions, unique constraints, `ON CONFLICT`, `RETURNING`, JSONB, UUID functions, and transaction isolation semantics needed by this phase. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] [CITED: https://www.postgresql.org/docs/18/transaction-iso.html] |
| `sqlx` | 0.8.6 | Async PostgreSQL access, typed query mapping, migrations | 0.8.6 is the stable Rust-1.85-compatible target; 0.9.0-alpha.1 is newest but alpha and requires Rust 1.86. [VERIFIED: cargo info sqlx@0.8.6] [VERIFIED: cargo info sqlx] |
| `tokio` | 1.52.0 | Async runtime for storage I/O tests and future runtime integration | `sqlx` and the later adapter/runtime stack are async; `tokio` 1.52.0 is current and Rust-1.85-compatible. [VERIFIED: cargo info tokio] |
| `serde` / `serde_json` | `serde` 1.0.228; `serde_json` 1.0.149 | Serialize event payloads, metadata, and stored replies | Workspace already standardizes these crates; PostgreSQL `jsonb` is appropriate for inspectable event payload/metadata in this template. [VERIFIED: Cargo.toml] [CITED: https://www.postgresql.org/docs/18/datatype-json.html] |
| `uuid` | 1.23.1 available; workspace currently allows 1.23.x via `1.23.0` | Event IDs, command IDs, correlation IDs, causation IDs | `uuid` provides Rust UUIDv7 generation with the `v7` feature; PostgreSQL 18 also exposes `uuidv7()`. [VERIFIED: cargo info uuid] [CITED: https://www.postgresql.org/docs/18/functions-uuid.html] |
| `time` | Workspace pinned `=0.3.44`; latest 0.3.47 requires no phase need to upgrade | Rust timestamp representation for `timestamptz` mappings | Workspace already chose `time`; keep the exact pin unless a sqlx mapping issue requires change. [VERIFIED: Cargo.toml] [VERIFIED: cargo info time] |
| `thiserror` | 2.0.18 | Typed storage error enums | Workspace already uses `thiserror`; storage API should expose typed concurrency/dedupe/infrastructure errors. [VERIFIED: Cargo.toml] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `testcontainers` | 0.25.0 | Real PostgreSQL integration tests under Rust 1.85 | Use because latest 0.27.3 requires Rust 1.88 and this project is pinned to Rust 1.85. [VERIFIED: cargo info testcontainers] [VERIFIED: cargo info testcontainers@0.27.3] |
| `testcontainers-modules` | 0.13.0 | PostgreSQL container module under Rust 1.85 | Use with `features = ["postgres"]` if the module API is convenient; latest 0.15.0 requires Rust 1.88. [VERIFIED: cargo info testcontainers-modules] [VERIFIED: cargo info testcontainers-modules@0.15.0] |
| `sqlx-cli` | 0.8.6 | Migration authoring and optional query preparation | Pin to match `sqlx` 0.8.6; latest CLI is 0.9.0-alpha.1 and requires Rust 1.86. [VERIFIED: cargo info sqlx-cli@0.8.6] [VERIFIED: cargo info sqlx-cli] |
| `anyhow` | 1.0.102 | Test/bootstrap helper errors only | Use in integration test setup and CLI-like test harnesses, not in the public storage API. [VERIFIED: cargo info anyhow] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `sqlx` 0.8.6 | `sqlx` 0.9.0-alpha.1 | Do not use by default: it is alpha and requires Rust 1.86 while the workspace toolchain is Rust 1.85. [VERIFIED: cargo info sqlx] [VERIFIED: rust-toolchain.toml] |
| Real PostgreSQL tests | SQLite/in-memory store | Do not substitute for STORE tests: SQLite will not verify PostgreSQL `jsonb`, `ON CONFLICT`, locking, isolation, or `timestamptz` behavior. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html] |
| Generic event-sourcing framework | A Rust CQRS/event-sourcing crate | Do not use for Phase 2: project requirements need explicit transaction shape, outbox-ready append semantics, and global-position reads. [VERIFIED: .planning/PROJECT.md] |
| `cargo-nextest` latest | `cargo test` | Use `cargo test` now because latest `cargo-nextest` requires Rust 1.91 and no compatible CLI is installed. [VERIFIED: cargo info cargo-nextest@0.9.133] [VERIFIED: environment audit] |

**Installation:**

```bash
cargo add sqlx@0.8.6 --package es-store-postgres --features runtime-tokio-rustls,postgres,uuid,time,json,migrate
cargo add tokio@1.52.0 --workspace --features rt-multi-thread,macros,time
cargo add anyhow@1.0.102 --dev --package es-store-postgres
cargo add testcontainers@0.25.0 --dev --package es-store-postgres
cargo add testcontainers-modules@0.13.0 --dev --package es-store-postgres --features postgres
cargo install sqlx-cli --version 0.8.6 --no-default-features --features postgres,rustls
```

**Version verification:** Versions above were checked with `cargo info` on 2026-04-17. [VERIFIED: cargo info]

## Architecture Patterns

### System Architecture Diagram

```text
Command handler / future runtime
  |
  | AppendRequest { stream_id, expected_revision, metadata, events, idempotency_key }
  v
EventStore::append
  |
  v
PostgreSQL transaction
  |
  +--> Check command_dedup by (tenant_id, idempotency_key)
  |      |
  |      +--> Found: return stored committed result; append no events
  |      |
  |      +--> Missing: continue
  |
  +--> Lock/update stream revision according to ExpectedRevision
  |      |
  |      +--> mismatch: rollback and return StreamConflict
  |      |
  |      +--> match: compute next stream revisions
  |
  +--> Insert events with global_position identity and full metadata
  |
  +--> Insert command_dedup committed result
  |
  +--> Optional: insert/update snapshot when caller requests snapshot write
  |
  v
Commit transaction
  |
  v
CommittedAppend { stream_id, first_revision, last_revision, global_positions, event_ids }
  |
  +--> Future projectors/outbox read by events.global_position
```

This flow keeps durability and replay anchored to PostgreSQL, while later disruptor use remains an in-process execution detail. [VERIFIED: .planning/PROJECT.md] [VERIFIED: .planning/ROADMAP.md]

### Recommended Project Structure

```text
crates/es-store-postgres/
├── migrations/             # SQLx-managed schema migrations
├── src/
│   ├── lib.rs              # public storage API exports
│   ├── error.rs            # typed StoreError and conflict details
│   ├── event_store.rs      # EventStore trait and PostgresEventStore implementation
│   ├── models.rs           # StoredEvent, NewEvent, CommittedAppend, SnapshotRecord
│   ├── sql.rs              # explicit query functions
│   └── rehydrate.rs        # snapshot + event replay helpers
└── tests/
    ├── append_occ.rs       # real PostgreSQL append/conflict tests
    ├── dedupe.rs           # repeated command/idempotency behavior
    ├── snapshots.rs        # latest snapshot + subsequent events
    └── global_reads.rs     # projector/outbox catch-up reads
```

This structure keeps storage implementation in the storage crate and keeps deterministic aggregate logic in `es-kernel`. [VERIFIED: current workspace crates] [VERIFIED: .planning/PROJECT.md]

### Pattern 1: Explicit Append Transaction

**What:** Begin a SQLx transaction, perform dedupe lookup/claim, enforce stream revision, insert events, write dedupe result, then commit. [CITED: https://docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html]

**When to use:** Every command append path, including commands that emit multiple events for one stream. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: https://docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html
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

### Pattern 2: Database-Enforced Idempotency

**What:** Store a durable result keyed by `(tenant_id, idempotency_key)` and return that result on repeats. [VERIFIED: .planning/REQUIREMENTS.md]

**When to use:** Any command whose caller provides an idempotency key or command ID that must not append duplicate events. [VERIFIED: STORE-03]

**Example:**

```sql
-- Source: PostgreSQL INSERT / ON CONFLICT docs
INSERT INTO command_dedup (
    tenant_id, idempotency_key, stream_id, first_revision, last_revision,
    first_global_position, last_global_position, response_payload
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
RETURNING stream_id, first_revision, last_revision, first_global_position, last_global_position;
```

PostgreSQL documents `ON CONFLICT` as the supported alternative to raising a unique violation and supports `RETURNING` rows from successful insert/update paths. [CITED: https://www.postgresql.org/docs/18/sql-insert.html]

### Pattern 3: Append-Only Events with Separate Stream Head

**What:** Use `events` as immutable event history and `streams` as the current stream revision index. [VERIFIED: .planning/REQUIREMENTS.md]

**When to use:** Always; this gives fast OCC checks and ordered per-stream replay without rewriting event rows. [VERIFIED: .planning/REQUIREMENTS.md]

**Schema sketch:**

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

PostgreSQL unique constraints automatically create unique B-tree indexes and can cover multi-column keys. [CITED: https://www.postgresql.org/docs/18/ddl-constraints.html]

### Pattern 4: Rehydration from Snapshot plus Events

**What:** Load latest snapshot for a stream, then read events after that revision ordered by stream revision. [VERIFIED: STORE-04]

**When to use:** Before deciding a command when no shard-local aggregate cache exists or when rebuilding cache after restart. [VERIFIED: .planning/ROADMAP.md]

**Example:**

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

### Anti-Patterns to Avoid

- **Dedupe in memory only:** It fails across process restarts and multiple future service instances; use PostgreSQL uniqueness. [VERIFIED: .planning/REQUIREMENTS.md]
- **Global position from ring sequence:** Ring sequence is not durable or cross-process; use `events.global_position`. [VERIFIED: .planning/PROJECT.md]
- **Projector reads by timestamp:** Timestamps are not gap-free cursors; use monotonic committed global positions. [VERIFIED: STORE-05]
- **Snapshot as source of truth:** Snapshots are acceleration artifacts; events remain authoritative. [VERIFIED: .planning/PROJECT.md]
- **One JSON blob event table without typed columns:** STORE-02 requires inspectable first-class columns for IDs, stream, revisions, type, schema version, and timestamps. [VERIFIED: .planning/REQUIREMENTS.md]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQL migration runner | Custom migration table/runner | SQLx migrations / `sqlx-cli` 0.8.6 | SQLx already supports migrations and compile-time embedded migrations; custom migration state adds avoidable failure modes. [CITED: https://github.com/launchbadge/sqlx] |
| Transaction management | Manual connection state machine | `sqlx::Transaction` | SQLx transactions commit explicitly and roll back on drop when still in progress. [CITED: https://docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html] |
| Optimistic concurrency uniqueness | App-only pre-checks | PostgreSQL primary/unique constraints and row locks | Database constraints are authoritative under concurrency. [CITED: https://www.postgresql.org/docs/18/ddl-constraints.html] |
| Idempotency conflict handling | Local hash map or file cache | PostgreSQL unique `(tenant_id, idempotency_key)` | Command dedupe must survive restart and work across future runtime instances. [VERIFIED: STORE-03] |
| JSON parsing/storage format | Ad hoc string escaping | `serde_json` + PostgreSQL `jsonb` | JSONB validates and stores processed JSON; serde_json is already standardized in the workspace. [CITED: https://www.postgresql.org/docs/18/datatype-json.html] [VERIFIED: Cargo.toml] |
| PostgreSQL integration harness | Shell scripts with fixed ports | `testcontainers` 0.25.0 | Testcontainers starts and cleans container dependencies programmatically for integration tests. [CITED: https://rust.testcontainers.org/] |

**Key insight:** The hard parts in this phase are concurrency and durability boundaries, not serialization syntax; push correctness into PostgreSQL constraints/transactions and keep Rust responsible for typed API shape, validation, and error mapping. [VERIFIED: .planning/REQUIREMENTS.md] [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

## Common Pitfalls

### Pitfall 1: Using Latest Crates Without Checking Rust Floor

**What goes wrong:** `sqlx` 0.9 alpha, `testcontainers` 0.27.3, `testcontainers-modules` 0.15.0, and `cargo-nextest` 0.9.133 do not align with the workspace Rust 1.85 floor. [VERIFIED: cargo info] [VERIFIED: rust-toolchain.toml]

**Why it happens:** `cargo info <crate>` reports newest versions first, even when the project toolchain cannot compile them. [VERIFIED: cargo info]

**How to avoid:** Pin Rust-1.85-compatible versions in Phase 2 and use `cargo test` unless the project upgrades Rust. [VERIFIED: local environment audit]

**Warning signs:** Resolver errors mentioning `rust-version` greater than `1.85`. [VERIFIED: cargo info]

### Pitfall 2: Treating `ON CONFLICT DO NOTHING` as "Found Existing Row"

**What goes wrong:** A concurrent insert can prevent insertion under Read Committed even when that row is not visible to the command snapshot. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

**Why it happens:** PostgreSQL Read Committed takes snapshots per command, and `ON CONFLICT` has concurrency-specific behavior. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

**How to avoid:** After `ON CONFLICT DO NOTHING` returns no row, issue a follow-up `SELECT` for the dedupe key inside the transaction or retry the transaction where appropriate. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

**Warning signs:** Dedupe tests that intermittently return "not found" under concurrent duplicate commands. [ASSUMED]

### Pitfall 3: Serializable Isolation Without Retry Strategy

**What goes wrong:** Serializable transactions can abort with SQLSTATE `40001`; without retry handling the append path produces transient failures. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

**Why it happens:** PostgreSQL may roll back one transaction to preserve serializable behavior. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

**How to avoid:** Use Read Committed plus targeted row locks/constraints for this append path, or implement a bounded transaction retry loop if Serializable is chosen later. [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

**Warning signs:** Errors containing SQLSTATE `40001` or "could not serialize access". [CITED: https://www.postgresql.org/docs/18/transaction-iso.html]

### Pitfall 4: JSONB Changes Payload Semantics

**What goes wrong:** `jsonb` does not preserve insignificant whitespace, key order, or duplicate object keys. [CITED: https://www.postgresql.org/docs/18/datatype-json.html]

**Why it happens:** PostgreSQL stores `jsonb` in a processed binary representation. [CITED: https://www.postgresql.org/docs/18/datatype-json.html]

**How to avoid:** Treat payloads as semantic JSON, not byte-for-byte canonical event payloads; if exact byte preservation becomes a requirement, add a separate binary/text payload column. [CITED: https://www.postgresql.org/docs/18/datatype-json.html]

**Warning signs:** Tests comparing raw JSON strings instead of parsed JSON values. [ASSUMED]

### Pitfall 5: Confusing Empty Event Decisions with Appends

**What goes wrong:** Commands that produce no events can accidentally create stream revisions or dedupe results that imply a committed event. [ASSUMED]

**Why it happens:** Event-sourced command APIs often return replies even when no state change occurs. [VERIFIED: current `Decision<E, R>` supports any `Vec<E>` length]

**How to avoid:** Follow D-10: reject new empty appends in the low-level store; future runtime code can handle no-op command replies explicitly outside the Phase 02 append contract. [VERIFIED: .planning/phases/02-durable-event-store-source-of-truth/02-CONTEXT.md]

**Warning signs:** `CommittedAppend` with no event IDs but non-null revision/global position fields. [ASSUMED]

## Code Examples

### SQLx Transaction Boundary

```rust
// Source: https://docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html
pub async fn append(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("INSERT INTO streams (tenant_id, stream_id, revision) VALUES ($1, $2, $3)")
        .bind("tenant-a")
        .bind("order-1")
        .bind(1_i64)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}
```

### Global-Position Catch-Up Query

```rust
// Source: STORE-05 and SQLx query macro docs
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

SQLx query macros compile-check SQL against a database or prepared metadata, and SQLx documents that macros use regular SQL rather than an ORM DSL. [CITED: https://github.com/launchbadge/sqlx] [CITED: https://docs.rs/sqlx/0.8.6/sqlx/macro.query.html]

### Testcontainers PostgreSQL Harness

```rust
// Source: https://github.com/testcontainers/testcontainers-rs/blob/main/docs/quickstart/community_modules.md
use testcontainers_modules::{postgres, testcontainers::runners::SyncRunner};

#[test]
fn starts_postgres_for_store_tests() {
    let container = postgres::Postgres::default().start().unwrap();
    let host_port = container.get_host_port_ipv4(5432).unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{host_port}/postgres");

    assert!(database_url.contains("postgres://"));
}
```

Use this pattern only with the Rust-1.85-compatible module version, not latest 0.15.0. [VERIFIED: cargo info testcontainers-modules]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `sqlx` 0.8 latest stable | `sqlx` 0.9 alpha exists but 0.8.6 remains the stable fit here | Verified 2026-04-17 | Keep 0.8.6 because 0.9 alpha requires Rust 1.86 and raises stability risk. [VERIFIED: cargo info sqlx] |
| Treat `uuid-ossp` as required for UUIDs | PostgreSQL 18 includes `uuidv7()` | PostgreSQL 18 docs current on 2026-04-17 | DB-generated UUIDv7 is available if needed, but Rust-generated event IDs remain fine. [CITED: https://www.postgresql.org/docs/18/functions-uuid.html] |
| Latest Testcontainers by default | Pin Rust-floor-compatible Testcontainers | Verified 2026-04-17 | Use `testcontainers` 0.25.0 and modules 0.13.0 unless Rust floor is raised. [VERIFIED: cargo info testcontainers] |
| One E2E test for storage confidence | Real PostgreSQL integration tests per behavior | Project requirement current on 2026-04-17 | Plan separate tests for append/OCC, metadata, dedupe, snapshots, global reads. [VERIFIED: .planning/REQUIREMENTS.md] |

**Deprecated/outdated:**

- Using `sqlx` 0.9 alpha as the default is inappropriate for this workspace because it requires Rust 1.86 and is not stable. [VERIFIED: cargo info sqlx]
- Using latest `cargo-nextest` in the plan is inappropriate while the workspace is pinned to Rust 1.85 because latest requires Rust 1.91. [VERIFIED: cargo info cargo-nextest@0.9.133]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Concurrent dedupe warning signs will show as intermittent "not found" results. | Common Pitfalls | Test design might look for the wrong symptom; still test concurrent duplicates directly. |
| A2 | Payload tests should compare parsed JSON values rather than raw strings. | Common Pitfalls | If exact byte preservation is desired, schema must add a raw payload column. |
| A3 | Concurrent duplicate dedupe must be tested directly, not inferred from sequential duplicate behavior. | Common Pitfalls / Validation Architecture | Without an explicit concurrent duplicate test, an implementation might insert events before discovering a late dedupe conflict. |

## Open Questions

All previously open Phase 02 research questions are resolved by `02-CONTEXT.md`.

1. **Resolved by D-10:** Storage rejects new empty appends. No-op command replies are a later runtime concern, not a Phase 02 storage append behavior. [VERIFIED: .planning/phases/02-durable-event-store-source-of-truth/02-CONTEXT.md]

2. **Resolved by D-14:** Event IDs are generated in Rust through a dedicated module/helper, not through DB-side `uuidv7()` defaults. [VERIFIED: .planning/phases/02-durable-event-store-source-of-truth/02-CONTEXT.md]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain | Build and tests | yes | `rustc 1.85.1`, project channel `1.85` | None needed. [VERIFIED: local environment audit] |
| Cargo | Build and dependency checks | yes | `cargo 1.85.1` | None needed. [VERIFIED: local environment audit] |
| Docker CLI/daemon | Testcontainers PostgreSQL tests | yes | Docker 29.3.1 client; `docker info` exits 0 | None needed. [VERIFIED: local environment audit] |
| `psql` | Manual DB inspection | no | not installed | Use SQLx/testcontainers tests; optional install only for manual debugging. [VERIFIED: local environment audit] |
| `sqlx-cli` | Migration CLI | no | not installed | Add install step for `sqlx-cli` 0.8.6 or use embedded migrations/tests until CLI is installed. [VERIFIED: local environment audit] |
| `cargo-nextest` | Optional fast test runner | no | not installed | Use `cargo test --workspace --all-targets`. [VERIFIED: local environment audit] |

**Missing dependencies with no fallback:**

- None for planning; Docker is available for PostgreSQL integration tests. [VERIFIED: local environment audit]

**Missing dependencies with fallback:**

- `sqlx-cli` is missing; use `cargo install sqlx-cli --version 0.8.6 --no-default-features --features postgres,rustls` when migration CLI commands are needed. [VERIFIED: local environment audit]
- `psql` is missing; rely on automated SQLx tests unless manual inspection is required. [VERIFIED: local environment audit]
- `cargo-nextest` is missing and latest is incompatible with Rust 1.85; use `cargo test`. [VERIFIED: cargo info cargo-nextest@0.9.133]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness via `cargo test`; PostgreSQL integration tests through Testcontainers 0.25.0. [VERIFIED: local test run] [VERIFIED: cargo info testcontainers] |
| Config file | `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`; no nextest config detected. [VERIFIED: filesystem check] |
| Quick run command | `cargo test -p es-store-postgres --lib` [VERIFIED: cargo test available] |
| Full suite command | `cargo test --workspace --all-targets` [VERIFIED: local test run passed] |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| STORE-01 | Append events and reject wrong expected revision | PostgreSQL integration | `cargo test -p es-store-postgres --test append_occ -- --nocapture` | No, Wave 0. [VERIFIED: filesystem check] |
| STORE-02 | Persist and inspect full event metadata columns | PostgreSQL integration | `cargo test -p es-store-postgres --test append_occ metadata_columns -- --nocapture` | No, Wave 0. [VERIFIED: filesystem check] |
| STORE-03 | Repeated tenant/idempotency key returns prior result | PostgreSQL integration | `cargo test -p es-store-postgres --test dedupe -- --nocapture` | No, Wave 0. [VERIFIED: filesystem check] |
| STORE-04 | Latest snapshot plus subsequent events rehydrates state | Unit + PostgreSQL integration | `cargo test -p es-store-postgres --test snapshots -- --nocapture` | No, Wave 0. [VERIFIED: filesystem check] |
| STORE-05 | Read committed events by global position | PostgreSQL integration | `cargo test -p es-store-postgres --test global_reads -- --nocapture` | No, Wave 0. [VERIFIED: filesystem check] |

### Sampling Rate

- **Per task commit:** `cargo test -p es-store-postgres --lib` plus the relevant integration test for the touched behavior. [ASSUMED]
- **Per wave merge:** `cargo test --workspace --all-targets`. [VERIFIED: local test run passed]
- **Phase gate:** Full workspace test suite plus all Phase 2 PostgreSQL integration tests green before `/gsd-verify-work`. [VERIFIED: .planning/config.json nyquist_validation=true]

### Wave 0 Gaps

- [ ] `crates/es-store-postgres/migrations/` - event-store schema migrations for STORE-01 through STORE-05. [VERIFIED: filesystem check]
- [ ] `crates/es-store-postgres/tests/common/` - PostgreSQL container/pool/migration fixture. [VERIFIED: filesystem check]
- [ ] `crates/es-store-postgres/tests/append_occ.rs` - append and optimistic concurrency behavior. [VERIFIED: filesystem check]
- [ ] `crates/es-store-postgres/tests/dedupe.rs` - repeated command behavior. [VERIFIED: filesystem check]
- [ ] `crates/es-store-postgres/tests/snapshots.rs` - snapshot save/load and replay boundary. [VERIFIED: filesystem check]
- [ ] `crates/es-store-postgres/tests/global_reads.rs` - committed global-position reads. [VERIFIED: filesystem check]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 2 has no user authentication boundary. [VERIFIED: .planning/ROADMAP.md] |
| V3 Session Management | no | Phase 2 has no sessions. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | yes | Always include `tenant_id` in stream, event, snapshot, and dedupe keys/queries. [VERIFIED: STORE-02] |
| V5 Input Validation | yes | Validate non-empty IDs via `es-core` newtypes and validate schema versions/event types before append. [VERIFIED: crates/es-core/src/lib.rs] |
| V6 Cryptography | no | Phase 2 does not implement cryptography; UUID generation is identity/correlation, not crypto security. [VERIFIED: .planning/ROADMAP.md] |

### Known Threat Patterns for Rust/PostgreSQL Event Store

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant event leakage | Information Disclosure | Include `tenant_id` in primary keys, unique keys, and every read predicate. [VERIFIED: STORE-02] |
| SQL injection | Tampering | Use SQLx bind parameters and macros; do not string-concatenate SQL values. [CITED: https://github.com/launchbadge/sqlx] |
| Duplicate command replay | Tampering / Repudiation | Use durable `(tenant_id, idempotency_key)` uniqueness and return stored results. [VERIFIED: STORE-03] |
| Metadata spoofing across tenants | Spoofing | Storage should derive tenant from `CommandMetadata` and persist it consistently on events/dedupe/snapshots. [VERIFIED: crates/es-core/src/lib.rs] |
| Unbounded payload size | Denial of Service | Add application-level payload size limits before append; PostgreSQL storage alone is not admission control. [ASSUMED] |

## Sources

### Primary (HIGH confidence)

- Local project files: `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/PROJECT.md`, `.planning/STATE.md`, `Cargo.toml`, `rust-toolchain.toml`, `crates/es-core/src/lib.rs`, `crates/es-kernel/src/lib.rs`. [VERIFIED: filesystem reads]
- Context7 `/launchbadge/sqlx` docs: SQLx transactions, pools, query macros, JSON support. [VERIFIED: Context7 CLI]
- Context7 `/websites/postgresql_18` docs: unique constraints, `ON CONFLICT`, transaction isolation. [VERIFIED: Context7 CLI]
- PostgreSQL 18 docs: `INSERT`/`ON CONFLICT`/`RETURNING`, transaction isolation, JSON types, constraints, UUID functions. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] [CITED: https://www.postgresql.org/docs/18/transaction-iso.html] [CITED: https://www.postgresql.org/docs/18/datatype-json.html] [CITED: https://www.postgresql.org/docs/18/ddl-constraints.html] [CITED: https://www.postgresql.org/docs/18/functions-uuid.html]
- SQLx docs/GitHub: transaction API, query macros, features. [CITED: https://docs.rs/sqlx/0.8.6/sqlx/struct.Transaction.html] [CITED: https://docs.rs/sqlx/0.8.6/sqlx/macro.query.html] [CITED: https://github.com/launchbadge/sqlx]
- Crate metadata: `cargo info sqlx@0.8.6`, `sqlx`, `tokio`, `testcontainers`, `testcontainers@0.27.3`, `testcontainers-modules`, `testcontainers-modules@0.15.0`, `sqlx-cli@0.8.6`, `cargo-nextest@0.9.133`. [VERIFIED: cargo info]

### Secondary (MEDIUM confidence)

- Testcontainers Rust docs and Context7 snippets for PostgreSQL module usage. [CITED: https://rust.testcontainers.org/] [VERIFIED: Context7 CLI]

### Tertiary (LOW confidence)

- Assumptions about no-op command behavior, JSON test style, and payload size admission are flagged in the Assumptions Log. [ASSUMED]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH for `sqlx`/PostgreSQL choice; MEDIUM for testcontainers exact API because the Rust-1.85-compatible version is not the latest docs default. [VERIFIED: cargo info] [VERIFIED: Context7 CLI]
- Architecture: HIGH because requirements and project constraints explicitly define event store as source of truth and global-position reads. [VERIFIED: .planning/REQUIREMENTS.md]
- Pitfalls: HIGH for Rust-version and PostgreSQL isolation pitfalls; MEDIUM for no-op append behavior because it needs a project decision. [VERIFIED: cargo info] [CITED: https://www.postgresql.org/docs/18/transaction-iso.html] [ASSUMED]

**Research date:** 2026-04-17 [VERIFIED: local environment date]
**Valid until:** 2026-05-17 for crate pins and PostgreSQL docs; re-run `cargo info` before implementation if planning is delayed. [ASSUMED]

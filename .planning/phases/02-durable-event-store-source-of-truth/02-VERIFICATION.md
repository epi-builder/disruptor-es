---
phase: 02-durable-event-store-source-of-truth
verified: 2026-04-16T23:24:31Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 2: Durable Event Store Source of Truth Verification Report

**Phase Goal:** Command success is anchored to durable append-only event-store commits, with stream concurrency, metadata, dedupe, snapshots, replay, and global-position reads available before runtime behavior depends on them.
**Verified:** 2026-04-16T23:24:31Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Developer can append domain events to a durable event store with per-stream optimistic concurrency and a clear committed result. | VERIFIED | `PostgresEventStore::append` delegates to `sql::append`; the SQL transaction acquires dedupe and stream advisory locks, checks `SELECT revision ... FOR UPDATE`, validates `ExpectedRevision`, writes stream head and event rows, writes dedupe result, then commits. `append_occ.rs` covers first append, multi-event revisions, wrong revision conflicts, rollback, and concurrent first append races. |
| 2 | Developer can inspect stored events and find event ID, stream ID, revision, global position, command/correlation/causation IDs, tenant ID, type, schema version, payload, metadata, and timestamp. | VERIFIED | The migration defines all STORE-02 columns on `events`; `insert_event` binds the fields from `AppendRequest.command_metadata` and `NewEvent`; `metadata_columns_are_persisted` queries and asserts every required column. |
| 3 | Repeating a command with the same tenant/idempotency key returns the prior committed result instead of appending duplicate events. | VERIFIED | `command_dedup` has `PRIMARY KEY (tenant_id, idempotency_key)`. `sql::append` locks tenant/idempotency, reads `response_payload`, and returns `AppendOutcome::Duplicate` without event inserts. `dedupe.rs` covers duplicate replay, no extra events, tenant scoping, and concurrent duplicate calls. |
| 4 | Aggregate state can be rehydrated from the latest snapshot plus subsequent stream events. | VERIFIED | `save_snapshot`, `load_latest_snapshot`, and `rehydrate::load_rehydration` are wired. Latest snapshot reads use `ORDER BY stream_revision DESC LIMIT 1`; rehydration reads events after the snapshot revision ordered by stream revision. `snapshots.rs` covers latest snapshot replacement, nonexistent/future snapshot rejection, snapshot-plus-subsequent events, and no-snapshot full stream events. |
| 5 | Projectors and outbox workers can read committed events by global position, independent of any disruptor ring sequence. | VERIFIED | `read_global` queries `events` with `WHERE tenant_id = $1 AND global_position > $2 ORDER BY global_position ASC LIMIT $3`. `global_reads.rs` covers after-position reads, limit handling, and tenant scoping. No storage implementation depends on disruptor or runtime crates. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/es-store-postgres/migrations/20260417000000_event_store.sql` | PostgreSQL schema for streams, events, command dedupe, snapshots | VERIFIED | Defines all four tables, tenant columns, revision/global constraints, event metadata columns, indexes, and no DB-side UUID defaults. |
| `crates/es-store-postgres/tests/common/mod.rs` | PostgreSQL 18 migrated test fixture | VERIFIED | `start_postgres()` starts `postgres:18`, builds a `PgPool`, and runs `sqlx::migrate!("./migrations")`. |
| `crates/es-store-postgres/tests/harness_smoke.rs` | Smoke test for migrated schema | VERIFIED | Uses `to_regclass` to assert `events`, `streams`, `command_dedup`, and `snapshots` exist. |
| `crates/es-store-postgres/src/models.rs` | Storage DTOs and validation | VERIFIED | Defines `AppendRequest`, `NewEvent`, `StoredEvent`, `SnapshotRecord`, `RehydrationBatch`, `CommittedAppend`, `AppendOutcome`, and `MAX_JSON_PAYLOAD_BYTES`; unit tests cover constructor validation. |
| `crates/es-store-postgres/src/event_store.rs` | Public storage facade | VERIFIED | Exposes append, stream read, global read, snapshot save/load, and rehydration methods, all delegated to real SQL/helper paths. |
| `crates/es-store-postgres/src/sql.rs` | Append, dedupe, snapshot, and read SQL | VERIFIED | Contains bound SQLx queries for transaction append, dedupe replay, snapshot writes/reads, stream reads, and global reads. No dynamic SQL construction was found. |
| `crates/es-store-postgres/src/rehydrate.rs` | Latest snapshot plus event read helper | VERIFIED | Loads latest snapshot, then calls stream read after that revision. |
| `crates/es-store-postgres/tests/append_occ.rs` | Append/OCC/metadata/rollback tests | VERIFIED | Real PostgreSQL integration coverage including review-fix concurrency regression tests. |
| `crates/es-store-postgres/tests/dedupe.rs` | Durable idempotency tests | VERIFIED | Real PostgreSQL duplicate, no-extra-events, tenant isolation, and concurrent duplicate tests. |
| `crates/es-store-postgres/tests/snapshots.rs` | Snapshot and rehydration tests | VERIFIED | Real PostgreSQL snapshot latest/replacement, invalid snapshot, and rehydration tests. |
| `crates/es-store-postgres/tests/global_reads.rs` | Global-position catch-up tests | VERIFIED | Real PostgreSQL after-position, limit, and tenant-scope tests. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/es-store-postgres/Cargo.toml` | `Cargo.toml` | Workspace dependencies | WIRED | GSD key-link check passed; manifest uses `sqlx.workspace = true` and storage-only dependencies. |
| `crates/es-store-postgres/tests/common/mod.rs` | `crates/es-store-postgres/migrations` | SQLx migrator | WIRED | GSD key-link check passed; fixture uses `sqlx::migrate!("./migrations")`. |
| `crates/es-store-postgres/src/lib.rs` | `crates/es-store-postgres/src/models.rs` | Public re-exports | WIRED | GSD key-link check passed; public storage DTOs are re-exported. |
| `crates/es-store-postgres/src/models.rs` | `crates/es-core/src/lib.rs` | Typed metadata imports | WIRED | Uses `es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId}` rather than duplicate storage newtypes. |
| `crates/es-store-postgres/src/event_store.rs` | `crates/es-store-postgres/src/sql.rs` | Method delegation | WIRED | `append`, `read_stream`, `read_global`, `save_snapshot`, and `load_latest_snapshot` delegate to SQL helpers. |
| `crates/es-store-postgres/src/event_store.rs` | `crates/es-store-postgres/src/rehydrate.rs` | Rehydration delegation | WIRED | `load_rehydration` delegates to `rehydrate::load_rehydration`. |
| `crates/es-store-postgres/src/sql.rs` | `events.global_position` | Durable catch-up query | WIRED | `read_global` orders by `global_position ASC` and filters by tenant and cursor. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `PostgresEventStore::append` | `AppendOutcome::Committed/Duplicate` | `AppendRequest` plus PostgreSQL `streams`, `events`, and `command_dedup` rows | Yes | VERIFIED - transaction writes durable rows and returns committed revisions, event IDs, and global positions; duplicate path decodes stored `response_payload`. |
| `StoredEvent` reads | event metadata and payload fields | PostgreSQL `events` rows mapped by `EventRow::try_into` | Yes | VERIFIED - read methods map real DB rows into typed `StoredEvent`, including IDs, tenant, revisions, JSON payload, metadata, and timestamp. |
| `load_rehydration` | `RehydrationBatch.snapshot` and `.events` | `snapshots` latest row plus `events` after snapshot revision | Yes | VERIFIED - helper composes two storage reads and does not fake aggregate replay. |
| `read_global` | ordered catch-up batch | PostgreSQL `events.global_position` | Yes | VERIFIED - uses durable identity global positions, not in-process ring sequence state. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Workspace metadata resolves storage dependencies | `cargo metadata --format-version 1 --no-deps` | Exit 0; `es-store-postgres` reports `es-core`, `sqlx`, `serde`, `serde_json`, `thiserror`, `time`, `uuid`, and test dependencies | PASS |
| Storage unit tests pass | `cargo test -p es-store-postgres --lib` | Exit 0; 7 tests passed | PASS |
| Artifact existence/substance checks | `gsd-tools verify artifacts` for plans 02-01 through 02-04 | 11/11 artifacts passed | PASS |
| Key-link checks | `gsd-tools verify key-links` for plans 02-01 through 02-04 | 8/8 links verified | PASS |
| PostgreSQL append/OCC and snapshot regression tests | Orchestrator ran `cargo test -p es-store-postgres --test append_occ --test snapshots -- --nocapture` after review fixes | Reported pass | PASS |
| Full workspace tests | Orchestrator ran `cargo test --workspace --all-targets` after code review fixes | Reported pass | PASS |
| Schema drift | Orchestrator drift check | `drift_detected=false` | PASS |
| Code review rerun | Orchestrator reran code review | `status: clean`, 0 findings | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| STORE-01 | 02-01, 02-02, 02-03 | Command handling can append domain events to a durable event store with per-stream optimistic concurrency. | SATISFIED | `append` transaction writes PostgreSQL streams/events and enforces `ExpectedRevision`; append/OCC tests pass per orchestrator and code inspection. |
| STORE-02 | 02-01, 02-02, 02-03 | Event store records include event ID, stream ID, stream revision, global position, command ID, causation ID, correlation ID, tenant ID, event type, schema version, payload, metadata, and recorded timestamp. | SATISFIED | Migration and `insert_event` include all columns; `metadata_columns_are_persisted` asserts all required fields. |
| STORE-03 | 02-01, 02-02, 02-03 | Command deduplication returns the prior committed result for a repeated tenant/idempotency key. | SATISFIED | Durable `command_dedup` primary key, stored `response_payload`, duplicate return path, and real PostgreSQL dedupe tests. |
| STORE-04 | 02-01, 02-02, 02-04 | Aggregate rehydration can load the latest snapshot and replay subsequent stream events. | SATISFIED | Storage returns latest snapshot plus ordered subsequent events; aggregate-state application is intentionally left to runtime/kernel per D-07. |
| STORE-05 | 02-01, 02-02, 02-04 | Event store exposes global-position reads for projector and outbox catch-up. | SATISFIED | `read_global` exposes tenant-scoped durable global-position reads; integration tests cover order, after-position, limit, and tenant isolation. |

No orphaned Phase 2 requirements were found in `.planning/REQUIREMENTS.md`; STORE-01 through STORE-05 are all declared in phase plans and accounted for above.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/es-store-postgres/tests/common/mod.rs` | 14 | `format!` for local PostgreSQL URL | Info | Test harness constructs a container connection string, not SQL. Caller values are not interpolated into SQL queries. |
| `crates/es-store-postgres/src/lib.rs` | 6 | `disruptor` in crate docs | Info | Documentation states the storage crate is not a disruptor execution crate. No disruptor dependency or runtime usage is present. |

No blocker anti-patterns were found. Grep found no `todo!`, `unimplemented!`, SQLite/mocks/in-memory stores, forbidden runtime/adapter/projection/outbox dependencies, or dynamic SQL construction in storage SQL.

### Human Verification Required

None. This phase delivers storage code and automated PostgreSQL behavior tests; no visual, UX, or external manual workflow remains.

### Gaps Summary

No gaps found. The Phase 02 goal is achieved: command success can be anchored to durable PostgreSQL append commits with optimistic concurrency, required metadata, tenant-scoped dedupe, snapshot rehydration inputs, and global-position reads available before runtime behavior depends on them.

---

_Verified: 2026-04-16T23:24:31Z_
_Verifier: Claude (gsd-verifier)_

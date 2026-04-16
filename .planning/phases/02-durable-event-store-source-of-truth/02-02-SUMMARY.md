---
phase: 02-durable-event-store-source-of-truth
plan: 02
subsystem: database
tags: [rust, postgres, event-store, validation, uuid]
requires:
  - phase: 02-durable-event-store-source-of-truth
    provides: "PostgreSQL schema, SQLx dependency wiring, and migrated integration-test harness from Plan 01"
provides:
  - "Typed PostgreSQL event-store API contracts for append, reads, snapshots, rehydration, and dedupe outcomes"
  - "Storage validation errors for empty appends, event type, idempotency key, schema version, and payload size"
  - "Rust UUIDv7 event ID generator facade"
affects: [phase-02, phase-03-command-runtime, phase-05-cqrs-projections, phase-06-outbox]
tech-stack:
  added: []
  patterns: [typed-storage-dtos, rust-uuidv7-generation, storage-api-shell]
key-files:
  created:
    - crates/es-store-postgres/src/error.rs
    - crates/es-store-postgres/src/models.rs
    - crates/es-store-postgres/src/ids.rs
    - crates/es-store-postgres/src/event_store.rs
  modified:
    - crates/es-store-postgres/src/lib.rs
key-decisions:
  - "AppendRequest derives tenant ownership from CommandMetadata rather than accepting a separate append-level tenant field."
  - "PostgresEventStore exposes the storage method surface now while append/read SQL remains owned by Plans 03 and 04."
  - "Event IDs are generated in Rust through a small IdGenerator trait using UUIDv7."
patterns-established:
  - "Storage DTO constructors validate correctness and denial-of-service guards before persistence."
  - "The storage facade re-exports public contracts while keeping runtime, adapter, projection, outbox, broker, and execution concerns out."
requirements-completed: [STORE-01, STORE-02, STORE-03, STORE-04, STORE-05]
duration: 4m24s
completed: 2026-04-16
---

# Phase 02 Plan 02: Storage API Contracts Summary

**Typed PostgreSQL event-store DTOs, validation errors, UUIDv7 generation, and storage API shell**

## Performance

- **Duration:** 4m24s
- **Started:** 2026-04-16T22:45:23Z
- **Completed:** 2026-04-16T22:49:47Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `StoreError` and `StoreResult` with typed validation, stream conflict, dedupe conflict, conversion, and database error variants.
- Added storage DTOs for new events, append requests, committed append results, stored events, snapshots, snapshot saves, and rehydration batches.
- Enforced empty append, empty event type, empty idempotency key, non-positive schema version, and 1 MiB serialized JSON payload validation before SQL persistence.
- Added `IdGenerator` and `UuidV7Generator` for Rust-side UUIDv7 event IDs.
- Replaced the placeholder crate facade with storage-focused module exports and a `PostgresEventStore` API shell.

## Task Commits

1. **Task 1 RED: Add failing model validation tests** - `2d51fe8` (test)
2. **Task 1 GREEN: Add storage models with payload validation** - `953dd24` (feat)
3. **Task 2: Add Rust UUID helper and public facade** - `d48887d` (feat)
4. **Task 3: Add PostgresEventStore API shell** - `9f6a55d` (feat)

**Plan metadata:** pending final docs commit

## Files Created/Modified

- `crates/es-store-postgres/src/error.rs` - Defines `StoreError`, `StoreResult`, and model validation error contracts.
- `crates/es-store-postgres/src/models.rs` - Defines append, event, stored row, snapshot, and rehydration DTOs with constructor validation tests.
- `crates/es-store-postgres/src/ids.rs` - Defines `IdGenerator` and UUIDv7 implementation.
- `crates/es-store-postgres/src/event_store.rs` - Defines `PostgresEventStore`, pool access, and the public async storage API shell.
- `crates/es-store-postgres/src/lib.rs` - Replaces the phase marker with storage-only crate docs, modules, and public re-exports.

## Decisions Made

- Used `CommandMetadata.tenant_id` as append tenant truth to avoid divergent tenant inputs at the storage API boundary.
- Kept SQL behavior out of this plan; methods return `StoreError::Database(sqlx::Error::RowNotFound)` until Plans 03 and 04 implement append/read/snapshot SQL.
- Added a tiny ID generator trait rather than coupling event ID creation to PostgreSQL defaults.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a minimal `event_store.rs` during the facade task**
- **Found during:** Task 2 (Add Rust UUID helper and public facade)
- **Issue:** The plan required `lib.rs` to declare and re-export `PostgresEventStore`, but the module did not exist yet, so the facade could not compile for `cargo test -p es-store-postgres --lib ids`.
- **Fix:** Added a minimal `PostgresEventStore` shell in Task 2, then expanded it with the planned API methods in Task 3.
- **Files modified:** `crates/es-store-postgres/src/event_store.rs`, `crates/es-store-postgres/src/lib.rs`
- **Verification:** `cargo test -p es-store-postgres --lib ids`, `cargo check -p es-store-postgres`
- **Committed in:** `d48887d`, completed by `9f6a55d`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The extra file creation was required to satisfy the planned facade and did not add runtime or SQL behavior beyond the planned API shell.

## Issues Encountered

- `cargo fmt --package es-store-postgres` formatted `tests/common/mod.rs`, which was outside this plan. That formatting-only change was reverted before task commits.

## Verification

- `cargo test -p es-store-postgres --lib models`
- `cargo test -p es-store-postgres --lib ids`
- `cargo check -p es-store-postgres`

## Known Stubs

The following stubs are intentional API-shell placeholders from this plan. Plans 03 and 04 own the SQL implementations.

| File | Line | Reason |
|------|------|--------|
| `crates/es-store-postgres/src/event_store.rs` | 29 | `append` validates empty events, then returns the planned pending SQL error until append/OCC/dedupe SQL is implemented. |
| `crates/es-store-postgres/src/event_store.rs` | 40 | `read_stream` exposes the tenant-scoped signature pending read SQL. |
| `crates/es-store-postgres/src/event_store.rs` | 50 | `read_global` exposes the tenant-scoped signature pending global-position SQL. |
| `crates/es-store-postgres/src/event_store.rs` | 58 | `save_snapshot` exposes the snapshot save signature pending snapshot SQL. |
| `crates/es-store-postgres/src/event_store.rs` | 67 | `load_latest_snapshot` exposes the tenant-scoped signature pending snapshot SQL. |
| `crates/es-store-postgres/src/event_store.rs` | 76 | `load_rehydration` exposes the tenant-scoped signature pending rehydration SQL. |

## Threat Flags

None. This plan adds validation DTOs and a PostgreSQL pool wrapper but no new network endpoints, file access, auth paths, or schema trust boundaries beyond the plan threat model.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 03 can implement append, optimistic concurrency, metadata persistence, and durable command dedupe against the explicit `AppendRequest`, `AppendOutcome`, `StoredEvent`, and error contracts. Plan 04 can fill the snapshot, rehydration, and global read methods without changing the public API shape.

## Self-Check: PASSED

- Verified created/modified files exist.
- Verified task commits exist: `2d51fe8`, `953dd24`, `d48887d`, `9f6a55d`.

---
*Phase: 02-durable-event-store-source-of-truth*
*Completed: 2026-04-16*

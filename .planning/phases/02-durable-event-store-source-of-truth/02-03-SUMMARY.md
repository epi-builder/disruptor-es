---
phase: 02-durable-event-store-source-of-truth
plan: 03
subsystem: database
tags: [rust, postgres, sqlx, event-store, optimistic-concurrency, idempotency]
requires:
  - phase: 02-durable-event-store-source-of-truth
    provides: "PostgreSQL schema, migrated Testcontainers harness, and typed storage API contracts from Plans 01 and 02"
provides:
  - "SQLx append transaction with optimistic concurrency and full metadata persistence"
  - "Tenant-scoped durable command dedupe replay backed by command_dedup"
  - "PostgreSQL integration coverage for append, OCC conflicts, metadata columns, rollback, and concurrent dedupe"
affects: [phase-03-command-runtime, phase-05-cqrs-projections, phase-06-outbox]
tech-stack:
  added: []
  patterns: [transactional-append, advisory-lock-dedupe, tenant-scoped-sql, postgres-integration-tests]
key-files:
  created:
    - crates/es-store-postgres/src/sql.rs
    - crates/es-store-postgres/tests/append_occ.rs
    - crates/es-store-postgres/tests/dedupe.rs
  modified:
    - crates/es-store-postgres/src/error.rs
    - crates/es-store-postgres/src/event_store.rs
    - crates/es-store-postgres/src/lib.rs
key-decisions:
  - "Use a transaction-scoped PostgreSQL advisory lock derived from tenant/idempotency key before stream or event writes."
  - "Store the full CommittedAppend JSON in command_dedup.response_payload so duplicate replies preserve exact event IDs and global positions."
  - "Keep append SQL in a private sql.rs helper while PostgresEventStore remains the public storage facade."
patterns-established:
  - "Append flow: dedupe lock, dedupe lookup, stream FOR UPDATE, OCC validation, stream head write, event inserts, dedupe result write, commit."
  - "Every append and dedupe query binds tenant_id explicitly and uses SQLx parameters rather than dynamic SQL construction."
requirements-completed: [STORE-01, STORE-02, STORE-03]
duration: 6m55s
completed: 2026-04-16
---

# Phase 02 Plan 03: Durable Append/OCC/Dedupe Summary

**PostgreSQL append transaction with optimistic concurrency, metadata-rich events, rollback-safe conflicts, and durable tenant-scoped command dedupe**

## Performance

- **Duration:** 6m55s
- **Started:** 2026-04-16T22:52:41Z
- **Completed:** 2026-04-16T22:59:36Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Implemented `PostgresEventStore::append` through a SQLx transaction that commits stream head updates, event rows, and dedupe results atomically.
- Persisted full STORE-02 metadata columns for every event: IDs, tenant, command/correlation/causation, type, schema, payload, metadata, revisions, global position, and timestamp.
- Added real PostgreSQL integration tests for no-stream appends, multi-event revision assignment, OCC conflicts, rollback, duplicate replay, tenant isolation, and concurrent same-key dedupe.

## Task Commits

1. **Task 1 RED: Add failing append/OCC coverage** - `b41fafe` (test)
2. **Task 1 GREEN: Implement append/OCC transaction** - `999df91` (feat)
3. **Task 2: Cover durable command dedupe** - `20e57fa` (test)

**Plan metadata:** pending final docs commit

## Files Created/Modified

- `crates/es-store-postgres/src/sql.rs` - Adds private SQLx append helpers for advisory dedupe locking, stream `FOR UPDATE`, OCC validation, event inserts, command dedupe replay, and transaction commit.
- `crates/es-store-postgres/src/event_store.rs` - Wires `PostgresEventStore::append` to `sql::append` while leaving Plan 04 read/snapshot methods as existing API-shell placeholders.
- `crates/es-store-postgres/src/error.rs` - Adds a typed decode error for stored dedupe response payloads.
- `crates/es-store-postgres/src/lib.rs` - Registers the private SQL module.
- `crates/es-store-postgres/tests/append_occ.rs` - Verifies append success, sequential revisions, metadata persistence, OCC conflict mapping, and rollback without extra event rows.
- `crates/es-store-postgres/tests/dedupe.rs` - Verifies duplicate replay, no duplicate events, tenant scoping, and concurrent duplicate serialization.

## Decisions Made

- Used `pg_advisory_xact_lock(hashtextextended($1 || ':' || $2, 0))` to serialize duplicate tenant/idempotency keys before any stream or event mutation.
- Stored the serialized `CommittedAppend` in `command_dedup.response_payload`; the existing first/last columns remain queryable, but duplicate replies do not have to infer exact global positions from a range.
- Kept the implementation storage-only: no runtime, disruptor, adapter, projection, outbox, broker, or domain decision logic was introduced.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Cast dedupe insert sentinel to bigint**
- **Found during:** Task 1 (Implement append/OCC tests and transaction)
- **Issue:** `INSERT INTO command_dedup ... RETURNING 1` returned PostgreSQL `INT4`, but the SQLx helper decoded it as `i64`, causing all append tests to fail after successful writes.
- **Fix:** Changed the sentinel to `RETURNING 1::bigint`.
- **Files modified:** `crates/es-store-postgres/src/sql.rs`
- **Verification:** `cargo check -p es-store-postgres`; `cargo test -p es-store-postgres --test append_occ -- --nocapture`
- **Committed in:** `999df91`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** The fix was required for the planned dedupe result insert path to decode correctly. No extra scope was added.

## Issues Encountered

- Task 2's dedupe tests passed on the first run because Task 1's planned append flow already included durable dedupe replay and advisory-lock serialization. The tests were committed as coverage rather than forcing a redundant implementation change.
- `cargo fmt --package es-store-postgres` reformatted `tests/common/mod.rs`; that unrelated formatting change was restored before commits so task diffs stayed scoped.

## Verification

- `cargo check -p es-store-postgres`
- `cargo test -p es-store-postgres --lib`
- `cargo test -p es-store-postgres --test append_occ -- --nocapture`
- `cargo test -p es-store-postgres --test dedupe -- --nocapture`

## Known Stubs

These are pre-existing API-shell placeholders from Plan 02 and remain intentional for Plan 04.

| File | Line | Reason |
|------|------|--------|
| `crates/es-store-postgres/src/event_store.rs` | 40 | `read_stream` remains pending read SQL for Plan 04. |
| `crates/es-store-postgres/src/event_store.rs` | 50 | `read_global` remains pending global-position SQL for Plan 04. |
| `crates/es-store-postgres/src/event_store.rs` | 58 | `save_snapshot` remains pending snapshot SQL for Plan 04. |
| `crates/es-store-postgres/src/event_store.rs` | 67 | `load_latest_snapshot` remains pending snapshot SQL for Plan 04. |
| `crates/es-store-postgres/src/event_store.rs` | 76 | `load_rehydration` remains pending snapshot-plus-events SQL for Plan 04. |

## Threat Flags

None. This plan implemented the planned storage API to PostgreSQL trust boundary. The SQL uses bind parameters, tenant-scoped predicates, durable dedupe rows, and metadata persistence as described in the plan threat model.

## User Setup Required

None - no external service configuration required. Docker or a compatible container runtime is required to run the PostgreSQL-backed integration tests locally.

## Next Phase Readiness

Plan 04 can build stream/global read and snapshot rehydration SQL on top of committed event rows and global positions. Phase 03 can depend on append returning only after durable PostgreSQL commit succeeds.

## Self-Check: PASSED

- Verified the summary file exists.
- Verified task commits exist: `b41fafe`, `999df91`, `20e57fa`.

---
*Phase: 02-durable-event-store-source-of-truth*
*Completed: 2026-04-16*

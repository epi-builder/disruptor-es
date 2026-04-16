---
phase: 02-durable-event-store-source-of-truth
plan: 04
subsystem: database
tags: [rust, postgres, sqlx, event-store, snapshots, rehydration, global-position]
requires:
  - phase: 02-durable-event-store-source-of-truth
    provides: "PostgreSQL schema, migrated integration harness, typed storage API contracts, append/OCC SQL, and durable command dedupe from Plans 01-03"
provides:
  - "Snapshot save and latest-snapshot load backed by PostgreSQL snapshots"
  - "Rehydration batches containing the latest snapshot plus subsequent stream events"
  - "Tenant-scoped stream reads and durable global-position catch-up reads"
affects: [phase-03-command-runtime, phase-05-cqrs-projections, phase-06-outbox]
tech-stack:
  added: []
  patterns: [snapshot-acceleration, durable-global-cursor, tenant-scoped-read-sql, tdd-red-green]
key-files:
  created:
    - crates/es-store-postgres/src/rehydrate.rs
    - crates/es-store-postgres/tests/snapshots.rs
    - crates/es-store-postgres/tests/global_reads.rs
  modified:
    - crates/es-store-postgres/src/error.rs
    - crates/es-store-postgres/src/event_store.rs
    - crates/es-store-postgres/src/lib.rs
    - crates/es-store-postgres/src/models.rs
    - crates/es-store-postgres/src/sql.rs
key-decisions:
  - "Represent snapshot records with state_payload and metadata to match the PostgreSQL snapshots table and plan contract."
  - "Keep rehydration in storage as latest snapshot plus ordered StoredEvent rows; aggregate replay remains kernel/runtime responsibility."
  - "Validate negative global cursors and read limits before SQL execution instead of casting them into unsigned values."
patterns-established:
  - "Read APIs bind tenant_id in every stream, snapshot, and global-position predicate."
  - "Global catch-up uses events.global_position as the durable projector/outbox cursor, ordered ascending with explicit LIMIT."
requirements-completed: [STORE-04, STORE-05]
duration: 6m45s
completed: 2026-04-16
---

# Phase 02 Plan 04: Snapshot Rehydration and Global Reads Summary

**PostgreSQL snapshot acceleration, rehydration batches, and tenant-scoped durable global-position catch-up reads**

## Performance

- **Duration:** 6m45s
- **Started:** 2026-04-16T23:03:01Z
- **Completed:** 2026-04-16T23:09:46Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments

- Added PostgreSQL-backed snapshot saving with `ON CONFLICT` replacement for `(tenant_id, stream_id, stream_revision)`.
- Added latest snapshot loading and rehydration batches that return only events after the snapshot revision, or all stream events when no snapshot exists.
- Added tenant-scoped durable global-position reads for future projector and outbox catch-up workers.
- Implemented stream read SQL with explicit limit validation and ordered per-stream revision results.

## Task Commits

1. **Task 1 RED: Add failing snapshot and rehydration tests** - `db7dc1c` (test)
2. **Task 1 GREEN: Implement snapshot and rehydration reads** - `1e19ae2` (feat)
3. **Task 2 RED: Add failing global read tests** - `7677c73` (test)
4. **Task 2 GREEN: Implement tenant global reads** - `790e7c2` (feat)

**Plan metadata:** pending final docs commit

## Files Created/Modified

- `crates/es-store-postgres/src/rehydrate.rs` - Loads latest snapshot plus subsequent stream events without applying aggregate replay.
- `crates/es-store-postgres/src/sql.rs` - Adds snapshot insert/load SQL, stream read SQL, global-position read SQL, row mapping, and read validation.
- `crates/es-store-postgres/src/event_store.rs` - Wires public read, snapshot, and rehydration methods to private SQL/helper modules.
- `crates/es-store-postgres/src/models.rs` - Aligns snapshot DTOs with `state_payload` and `metadata` stored in PostgreSQL.
- `crates/es-store-postgres/src/error.rs` - Adds typed errors for invalid stored tenant/stream IDs and invalid read limits.
- `crates/es-store-postgres/src/lib.rs` - Registers the private rehydration module.
- `crates/es-store-postgres/tests/snapshots.rs` - Verifies latest snapshot selection, replacement, tenant filtering, and rehydration behavior.
- `crates/es-store-postgres/tests/global_reads.rs` - Verifies after-position reads, ascending global ordering, limits, and tenant scoping.

## Decisions Made

- Snapshot DTOs now use `state_payload` and `metadata` because the migration already stores those JSONB columns and the plan requires that public request shape.
- Rehydration deliberately returns storage rows only; no `es_kernel::replay` or aggregate-state application was introduced.
- Negative `after_global_position`, negative stream read cursors, and negative limits are rejected with typed storage errors before the query is executed.

## TDD Gate Compliance

- RED gate present for Task 1: `db7dc1c`.
- GREEN gate present for Task 1: `1e19ae2`.
- RED gate present for Task 2: `7677c73`.
- GREEN gate present for Task 2: `790e7c2`.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo fmt --package es-store-postgres` reformatted `tests/common/mod.rs`, which was outside this plan. That formatting-only change was restored before commits so task diffs stayed scoped.

## Verification

- `cargo check -p es-store-postgres`
- `cargo test -p es-store-postgres --lib`
- `cargo test -p es-store-postgres --test snapshots -- --nocapture`
- `cargo test -p es-store-postgres --test global_reads -- --nocapture`
- `cargo test --workspace --all-targets`

## Known Stubs

None.

## User Setup Required

None - no external service configuration required. Docker or a compatible container runtime is required to run the PostgreSQL-backed integration tests locally.

## Next Phase Readiness

Phase 03 can use the storage crate to rebuild aggregate inputs from latest snapshots plus committed events. Phase 05 projectors and Phase 06 outbox workers can depend on tenant-scoped `events.global_position` reads rather than disruptor ring sequences.

## Self-Check: PASSED

- Verified created/modified files exist.
- Verified task commits exist: `db7dc1c`, `1e19ae2`, `7677c73`, `790e7c2`.

---
*Phase: 02-durable-event-store-source-of-truth*
*Completed: 2026-04-16*

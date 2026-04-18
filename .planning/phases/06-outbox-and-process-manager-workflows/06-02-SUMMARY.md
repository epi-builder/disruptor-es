---
phase: 06-outbox-and-process-manager-workflows
plan: 02
subsystem: integration
tags: [rust, postgres, outbox, process-manager, sqlx, testcontainers]

requires:
  - phase: 06-outbox-and-process-manager-workflows
    provides: "Storage-neutral outbox contracts, typed DTOs, retry outcomes, publisher idempotency keys, and process-manager names."
provides:
  - "Tenant-scoped PostgreSQL outbox schema with source-event/topic idempotency and dispatcher status fields."
  - "PostgresOutboxStore repository for insert, skip-locked claim, publish, retry, failure, and process-manager offsets."
  - "Container-backed integration tests for idempotency, tenant isolation, claim locking, retry bounds, and monotonic offsets."
affects: [phase-06, es-store-postgres, es-outbox, dispatcher, process-manager]

tech-stack:
  added: [es-outbox]
  patterns: [postgres-skip-locked-queue, typed-row-mapping, tenant-scoped-offset-upsert]

key-files:
  created:
    - crates/es-store-postgres/migrations/20260418010000_outbox.sql
    - crates/es-store-postgres/src/outbox.rs
    - crates/es-store-postgres/tests/outbox.rs
  modified:
    - Cargo.lock
    - crates/es-store-postgres/Cargo.toml
    - crates/es-store-postgres/src/error.rs
    - crates/es-store-postgres/src/lib.rs

key-decisions:
  - "Use PostgreSQL row locking with FOR UPDATE SKIP LOCKED for concurrent dispatcher claims instead of in-memory locks."
  - "Keep process-manager progress as tenant-scoped monotonic offsets using GREATEST on upsert."
  - "Validate inserted outbox rows against the source event's tenant and global position before storing them."

patterns-established:
  - "PostgreSQL outbox repository maps rows back through es-outbox typed constructors and reports mapping failures as StoreError::Outbox."
  - "Outbox status transitions clear worker locks when marking published, retrying, or failing rows."
  - "Container-backed outbox tests serialize PostgreSQL containers through a file-local Tokio mutex."

requirements-completed: [INT-01, INT-02, INT-03]

duration: 5min 43s
completed: 2026-04-18
---

# Phase 06 Plan 02: PostgreSQL Outbox Storage Summary

**Durable PostgreSQL outbox rows with skip-locked dispatcher claims, bounded retry status transitions, and tenant-scoped process-manager offsets.**

## Performance

- **Duration:** 5min 43s
- **Started:** 2026-04-18T08:10:28Z
- **Completed:** 2026-04-18T08:16:11Z
- **Tasks:** 1
- **Files modified:** 7

## Accomplishments

- Added `outbox_messages` with tenant/source-event/topic uniqueness, status/attempt/lock fields, pending-row index, and a foreign key to committed events.
- Added `process_manager_offsets` with a tenant/name primary key and monotonic offset upsert behavior.
- Implemented `PostgresOutboxStore` for message insert, bounded skip-locked claim, publish marking, retry/failure transitions, and process-manager offsets.
- Added Testcontainers-backed integration coverage for idempotency, queue claiming, retry bounds, tenant filters, and monotonic offsets.

## Task Commits

This TDD task was committed atomically through the required gates:

1. **RED: Add failing outbox repository tests** - `83f619c` (test)
2. **GREEN: Implement PostgreSQL outbox store** - `2ceaf1a` (feat)

**Plan metadata:** committed after summary creation.

## Files Created/Modified

- `Cargo.lock` - Records `es-store-postgres` depending on `es-outbox`.
- `crates/es-store-postgres/Cargo.toml` - Adds the local `es-outbox` dependency.
- `crates/es-store-postgres/migrations/20260418010000_outbox.sql` - Defines durable outbox and process-manager offset tables.
- `crates/es-store-postgres/src/error.rs` - Adds `StoreError::Outbox` for typed outbox row mapping failures.
- `crates/es-store-postgres/src/lib.rs` - Exports `PostgresOutboxStore`.
- `crates/es-store-postgres/src/outbox.rs` - Implements the PostgreSQL repository and row mapping.
- `crates/es-store-postgres/tests/outbox.rs` - Verifies schema and repository behavior against PostgreSQL 18 containers.

## Decisions Made

- Used `FOR UPDATE SKIP LOCKED` in the claim query so multiple workers can claim due rows without central coordination.
- Incremented `attempts` when a row is claimed for publication; `schedule_retry` uses the current attempt count to decide between `RetryScheduled` and `Failed`.
- Kept retry availability immediate for this repository plan because no backoff policy is part of the current contract.
- Required `insert_outbox_message` to match `tenant_id`, `source_event_id`, and `source_global_position` against the committed `events` row before insertion.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Enforced tenant/source-event consistency on insert**
- **Found during:** Task 06-02-01
- **Issue:** The migration foreign key proves the source event exists, but a bare UUID foreign key does not by itself prove the event belongs to the tenant inserting the outbox row.
- **Fix:** `insert_outbox_message` inserts through a `SELECT ... WHERE EXISTS` predicate matching `events.tenant_id`, `events.event_id`, and `events.global_position`.
- **Files modified:** `crates/es-store-postgres/src/outbox.rs`
- **Verification:** `outbox_repository_filters_by_tenant` passed, and all outbox repository acceptance checks passed.
- **Committed in:** `2ceaf1a`

---

**Total deviations:** 1 auto-fixed (1 missing critical).
**Impact on plan:** Strengthened the planned tenant isolation threat mitigation without changing the public API.

## Issues Encountered

None.

## Known Stubs

None.

## TDD Gate Compliance

- RED commit present: `83f619c`
- GREEN commit present after RED: `2ceaf1a`
- Refactor commit not needed.

## Verification

- `cargo test -p es-store-postgres --test outbox outbox_ -- --test-threads=1 --nocapture` - PASS
- Schema acceptance grep for `outbox_messages`, source-event/topic uniqueness, event foreign key, and `process_manager_offsets` - PASS
- Repository acceptance grep for `FOR UPDATE SKIP LOCKED`, publishing locks, retry outcomes, tenant binds, exports, and test names - PASS

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 06-03 can wire append-time outbox row creation into the event-store transaction using the new schema and repository contracts. The repository already provides the process-manager offset primitive needed by Plan 06-05.

## Self-Check: PASSED

- Created files exist: summary, outbox migration, `src/outbox.rs`, and integration tests.
- Task commits exist in git history: `83f619c`, `2ceaf1a`.
- Stub scan found no TODO/FIXME/placeholder or empty mock-data patterns in files created or modified by this plan.

---
*Phase: 06-outbox-and-process-manager-workflows*
*Completed: 2026-04-18*

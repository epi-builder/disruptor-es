---
phase: 06-outbox-and-process-manager-workflows
plan: 04
subsystem: integration
tags: [rust, outbox, dispatcher, postgres, idempotency, tdd]

requires:
  - phase: 06-outbox-and-process-manager-workflows
    provides: "Storage-neutral outbox contracts, PostgreSQL outbox repository, and append transaction outbox creation from Plans 06-01 through 06-03."
provides:
  - "Storage-neutral OutboxStore dispatcher port and dispatch_once orchestration."
  - "At-least-once dispatcher behavior that marks rows published only after Publisher::publish returns Ok(())."
  - "Retry and final failed outcome reporting through RetryScheduleOutcome."
  - "PostgresOutboxStore implementation of the OutboxStore dispatcher port."
affects: [phase-06, es-outbox, es-store-postgres, dispatcher, process-manager]

tech-stack:
  added: [futures dependency for es-store-postgres]
  patterns: [boxed-future-storage-port, tdd-red-green, at-least-once-outbox-dispatch]

key-files:
  created:
    - crates/es-outbox/src/dispatcher.rs
  modified:
    - Cargo.lock
    - crates/es-outbox/src/lib.rs
    - crates/es-store-postgres/Cargo.toml
    - crates/es-store-postgres/src/outbox.rs
    - crates/es-store-postgres/tests/outbox.rs

key-decisions:
  - "Keep dispatch orchestration in es-outbox storage-neutral; PostgreSQL implements the OutboxStore port instead of leaking SQLx into dispatcher code."
  - "Count publisher failures from the storage retry outcome so rows exhausted at max attempts are reported as failed, not retried."
  - "Use a fixed 30-second PostgreSQL claim lock in the OutboxStore adapter while preserving the repository's explicit lock-duration API."

patterns-established:
  - "Dispatcher ports follow the existing futures::BoxFuture trait style used by other storage-neutral boundaries."
  - "Dispatcher outcomes distinguish idle, all-published, and partial publish/retry/failed batches."
  - "Integration dispatcher tests verify durable row status transitions through the public dispatch_once API."

requirements-completed: [INT-02, INT-03]

duration: 4min 31s
completed: 2026-04-18
---

# Phase 06 Plan 04: Outbox Dispatcher Summary

**Storage-neutral outbox dispatcher with PostgreSQL row claiming, idempotent publishing, and bounded retry/failure reporting.**

## Performance

- **Duration:** 4min 31s
- **Started:** 2026-04-18T08:25:28Z
- **Completed:** 2026-04-18T08:29:59Z
- **Tasks:** 1
- **Files modified:** 6

## Accomplishments

- Added `crates/es-outbox/src/dispatcher.rs` with `OutboxStore` and `dispatch_once`.
- Wired `dispatch_once` to claim pending rows, publish envelopes with deterministic idempotency keys, mark successful rows only after publish success, and schedule retry/failure transitions on publisher errors.
- Implemented `OutboxStore` for `PostgresOutboxStore` using the existing repository methods and a 30-second lock duration.
- Added unit coverage for idle, success, retry, failed-at-max-attempts, and idempotency key preservation.
- Added PostgreSQL integration coverage for successful publish marking, retry scheduling, and max-attempt failure reporting.

## Task Commits

This TDD task was committed through the required gates:

1. **RED: Add failing dispatcher tests** - `d85bc18` (test)
2. **GREEN: Implement outbox dispatcher** - `0e79e21` (feat)

**Plan metadata:** committed after summary creation.

## Files Created/Modified

- `Cargo.lock` - Records the `es-store-postgres` futures dependency edge already present in the workspace lock.
- `crates/es-outbox/src/lib.rs` - Exports the dispatcher module, `dispatch_once`, and `OutboxStore`.
- `crates/es-outbox/src/dispatcher.rs` - Defines the storage-neutral dispatcher port, dispatch loop, and unit tests.
- `crates/es-store-postgres/Cargo.toml` - Adds the existing workspace `futures` dependency required for the `OutboxStore` BoxFuture implementation.
- `crates/es-store-postgres/src/outbox.rs` - Implements `OutboxStore` for `PostgresOutboxStore` and maps `StoreError` to `OutboxError::Store`.
- `crates/es-store-postgres/tests/outbox.rs` - Adds dispatcher integration tests for published, retried, and failed durable rows.

## Decisions Made

- Kept the dispatcher storage-neutral and broker-neutral; it depends on `OutboxStore` and `Publisher`, not SQLx or broker clients.
- Used `error.to_string()` when scheduling retry so publisher failures preserve the public `OutboxError` context in `last_error`.
- Preserved the existing explicit repository methods while adding the trait adapter, so lower-level storage tests can still choose lock durations directly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added missing futures dependency to es-store-postgres**
- **Found during:** Task 06-04-01 GREEN verification.
- **Issue:** Implementing `OutboxStore` for `PostgresOutboxStore` required `futures::future::BoxFuture`, but `es-store-postgres` did not declare `futures` as a direct dependency.
- **Fix:** Added `futures.workspace = true` to `crates/es-store-postgres/Cargo.toml`.
- **Files modified:** `crates/es-store-postgres/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo test -p es-outbox dispatcher -- --nocapture && cargo test -p es-store-postgres --test outbox dispatcher_ -- --test-threads=1 --nocapture` passed.
- **Committed in:** `0e79e21`

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** Required to satisfy the planned storage-neutral trait implementation; no public behavior or architecture changed.

## Issues Encountered

- The RED gate failed as intended: 1 idle test passed and 4 dispatcher behavior tests failed against the initial stub.
- The first GREEN verification exposed the missing direct `futures` dependency in `es-store-postgres`; it was fixed before the final verification run.

## Known Stubs

None.

## TDD Gate Compliance

- RED commit present: `d85bc18`
- GREEN commit present after RED: `0e79e21`
- Refactor commit not needed.

## Threat Flags

None - no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes beyond the planned dispatcher-to-publisher and PostgreSQL outbox surfaces.

## Verification

- `cargo test -p es-outbox dispatcher -- --nocapture && cargo test -p es-store-postgres --test outbox dispatcher_ -- --test-threads=1 --nocapture` - PASS
- Acceptance grep for dispatcher trait/function, publish call, mark/retry calls, retry/failed outcomes, public re-exports, PostgreSQL trait wiring, and all unit/integration test names - PASS
- Stub scan for TODO/FIXME/placeholder or hardcoded empty UI data patterns in touched files - PASS

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 06-05 can build process-manager workflows on top of the durable outbox and process-manager offset primitives. Dispatcher behavior now preserves at-least-once publishing semantics and reports retry exhaustion explicitly.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/06-outbox-and-process-manager-workflows/06-04-SUMMARY.md`.
- Task commits exist in git history: `d85bc18`, `0e79e21`.
- Key files exist on disk: dispatcher module, PostgreSQL outbox adapter, and dispatcher integration tests.
- Required verification command passed after the GREEN commit.

---
*Phase: 06-outbox-and-process-manager-workflows*
*Completed: 2026-04-18*

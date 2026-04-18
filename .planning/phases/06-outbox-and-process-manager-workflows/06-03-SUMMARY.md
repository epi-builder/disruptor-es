---
phase: 06-outbox-and-process-manager-workflows
plan: 03
subsystem: integration
tags: [rust, postgres, outbox, event-sourcing, sqlx, tdd]

requires:
  - phase: 06-outbox-and-process-manager-workflows
    provides: "Storage-neutral outbox contracts and PostgreSQL outbox schema/repository with tenant/source-event/topic idempotency."
provides:
  - "AppendRequest support for derived pre-append outbox messages with source-event validation."
  - "Atomic PostgreSQL append transaction insertion of pending outbox rows using committed source global positions."
  - "Integration tests proving append/outbox atomicity, duplicate replay idempotency, and conflict rollback behavior."
affects: [phase-06, es-store-postgres, es-outbox, dispatcher, process-manager]

tech-stack:
  added: []
  patterns: [append-transaction-outbox-insert, source-event-validation, tdd-red-green]

key-files:
  created: []
  modified:
    - crates/es-store-postgres/src/error.rs
    - crates/es-store-postgres/src/models.rs
    - crates/es-store-postgres/src/sql.rs
    - crates/es-store-postgres/tests/outbox.rs

key-decisions:
  - "Keep AppendRequest::new source-compatible by delegating to new_with_outbox with an empty outbox message list."
  - "Validate pre-append outbox source event IDs against the events in the same append request before storage."
  - "Insert outbox rows after event rows have committed transaction-local global positions and before command dedupe is recorded."
  - "Use tenant_id from CommandMetadata for append-created outbox rows and ON CONFLICT (tenant_id, source_event_id, topic) DO NOTHING for late duplicate idempotency."

patterns-established:
  - "Append-time outbox messages reference PendingSourceEventRef before storage; sql.rs upgrades them to committed source_global_position from InsertedEvent."
  - "Append transaction ordering is event inserts -> derived outbox inserts -> command dedupe result -> commit."
  - "Duplicate command replay exits from command_dedup before stream/event/outbox writes."

requirements-completed: [INT-01, INT-03]

duration: 4min 5s
completed: 2026-04-18
---

# Phase 06 Plan 03: Append Transaction Outbox Summary

**Append transactions now atomically persist derived pending outbox rows alongside committed events and command dedupe records.**

## Performance

- **Duration:** 4min 5s
- **Started:** 2026-04-18T08:18:47Z
- **Completed:** 2026-04-18T08:22:52Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments

- Extended `AppendRequest` with `outbox_messages: Vec<NewOutboxMessage>` while preserving the existing `AppendRequest::new(...)` call shape.
- Added `AppendRequest::new_with_outbox(...)` and validation that every outbox source event ID belongs to the same append request.
- Wired `sql::append` to insert pending outbox rows inside the append transaction after `insert_event` returns committed global positions and before `insert_dedupe_result`.
- Added integration coverage for append-created outbox rows, duplicate idempotency replay, and stream-conflict rollback.

## Task Commits

This TDD task was committed through the required gates:

1. **RED: Add failing append outbox atomicity tests** - `ec3b630` (test)
2. **GREEN: Implement append transaction outbox inserts** - `dd998cc` (feat)

**Plan metadata:** committed after summary creation.

## Files Created/Modified

- `crates/es-store-postgres/src/error.rs` - Adds `StoreError::InvalidOutboxSourceEvent`.
- `crates/es-store-postgres/src/models.rs` - Adds `AppendRequest::outbox_messages`, `new_with_outbox`, source-event validation, and model tests.
- `crates/es-store-postgres/src/sql.rs` - Inserts pending outbox rows with Rust-generated UUIDv7 IDs in the append transaction.
- `crates/es-store-postgres/tests/outbox.rs` - Adds append atomicity, duplicate replay, and conflict rollback integration tests.

## Decisions Made

- Kept append-created outbox rows tenant-scoped through `request.command_metadata.tenant_id`; callers cannot supply an independent outbox tenant.
- Matched outbox messages to inserted events by `source_event_id`, then used the inserted event's transaction-local `global_position` for `source_global_position`.
- Preserved duplicate command replay behavior by returning from `command_dedup` before any new event or outbox insertion work.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope expansion; implementation stayed within append transaction atomicity.

## Issues Encountered

None.

## Known Stubs

None.

## TDD Gate Compliance

- RED commit present: `ec3b630`
- GREEN commit present after RED: `dd998cc`
- Refactor commit not needed.

## Threat Flags

None - no new network endpoints, auth paths, file access patterns, or trust-boundary schema changes beyond the planned append-to-outbox persistence surface.

## Verification

- `cargo test -p es-store-postgres --test outbox append_ -- --test-threads=1 --nocapture` - PASS
- Acceptance grep for `outbox_messages`, `new_with_outbox`, `InvalidOutboxSourceEvent`, transactional insert SQL, absence of publisher calls, and append test names - PASS
- `cargo test -p es-store-postgres --lib models::append_request_rejects_unknown_outbox_source_event -- --nocapture` - PASS

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 06-04 can dispatch pending outbox rows knowing append-created messages are committed atomically with their source events and are idempotent by tenant/source-event/topic.

## Self-Check: PASSED

- Summary file exists at `.planning/phases/06-outbox-and-process-manager-workflows/06-03-SUMMARY.md`.
- Task commits exist in git history: `ec3b630`, `dd998cc`.
- Key modified files exist on disk: `error.rs`, `models.rs`, `sql.rs`, and `tests/outbox.rs`.
- Stub scan found no TODO/FIXME/placeholder or empty mock-data patterns in files created or modified by this plan.

---
*Phase: 06-outbox-and-process-manager-workflows*
*Completed: 2026-04-18*

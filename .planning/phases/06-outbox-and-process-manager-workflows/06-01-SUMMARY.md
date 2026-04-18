---
phase: 06-outbox-and-process-manager-workflows
plan: 01
subsystem: integration
tags: [rust, outbox, publisher, idempotency, event-sourcing]

requires:
  - phase: 05-cqrs-projection-and-query-catch-up
    provides: "Committed-event projection patterns, validated newtypes, and storage-neutral contract style."
provides:
  - "Storage-neutral outbox contract crate with validated DTOs and typed errors."
  - "Deterministic publisher idempotency key contract built from tenant, topic, and source event ID."
  - "In-memory idempotent publisher for dispatcher and future integration tests."
affects: [phase-06, es-outbox, es-store-postgres, dispatcher, process-manager]

tech-stack:
  added: [es-core, futures, serde, serde_json, thiserror, time, uuid]
  patterns: [validated-newtypes, boxed-future-port, idempotent-test-publisher]

key-files:
  created:
    - crates/es-outbox/src/error.rs
    - crates/es-outbox/src/models.rs
    - crates/es-outbox/src/publisher.rs
    - crates/es-outbox/tests/contracts.rs
  modified:
    - Cargo.lock
    - crates/es-outbox/Cargo.toml
    - crates/es-outbox/src/lib.rs

key-decisions:
  - "Keep es-outbox storage-neutral by depending on typed contracts, futures::BoxFuture, and no SQLx, broker, adapter, or disruptor runtime APIs."
  - "Use deterministic external idempotency keys in the form tenant_id:topic:source_event_id."
  - "Separate pre-append PendingSourceEventRef from persisted SourceEventRef so global positions are only required after storage commit."

patterns-established:
  - "Outbox contract modules follow the es-projection facade pattern with private modules and explicit public re-exports."
  - "Publisher ports use the existing local boxed-future trait pattern instead of async-trait."
  - "InMemoryPublisher records one external effect per idempotency key while accepting duplicate publish attempts."

requirements-completed: [INT-01, INT-02, INT-03]

duration: 4min 41s
completed: 2026-04-18
---

# Phase 06 Plan 01: Storage-Neutral Outbox Contracts Summary

**Validated outbox DTOs, deterministic publisher idempotency keys, and a storage-neutral publisher contract for future append, dispatcher, and workflow plans.**

## Performance

- **Duration:** 4min 41s
- **Started:** 2026-04-18T08:02:38Z
- **Completed:** 2026-04-18T08:07:19Z
- **Tasks:** 1
- **Files modified:** 7

## Accomplishments

- Added `OutboxError` and `OutboxResult` with typed validation, publisher, store, and process-manager error variants.
- Added validated outbox value objects, source references, retry policy, statuses, dispatch outcomes, and persisted/new message DTOs.
- Added `Publisher`, `PublishEnvelope`, and `InMemoryPublisher` with idempotent duplicate-publish behavior.
- Added contract tests for malformed values, pre-append versus persisted source references, deterministic envelopes, retry bounds, and publisher failures.

## Task Commits

Each task was committed atomically through the TDD gate:

1. **RED: Add failing outbox contract tests** - `569e844` (test)
2. **GREEN: Implement outbox contracts and publisher** - `7e78ae2` (feat)

**Plan metadata:** committed after summary creation.

## Files Created/Modified

- `Cargo.lock` - Records `es-outbox` dependency wiring.
- `crates/es-outbox/Cargo.toml` - Adds workspace dependencies needed by storage-neutral contracts and tests.
- `crates/es-outbox/src/lib.rs` - Replaces the placeholder facade with private modules and public re-exports.
- `crates/es-outbox/src/error.rs` - Defines typed outbox errors and result alias.
- `crates/es-outbox/src/models.rs` - Defines validated DTOs, source references, statuses, retry policy, and publish envelope construction.
- `crates/es-outbox/src/publisher.rs` - Defines the boxed-future publisher port and idempotent in-memory publisher.
- `crates/es-outbox/tests/contracts.rs` - Verifies the plan behavior through focused contract tests.

## Decisions Made

- Kept `es-outbox` free of SQLx, broker, adapter, and disruptor dependencies so PostgreSQL storage and dispatcher plans can build against stable contracts.
- Modeled `PendingSourceEventRef` without `global_position`; only `SourceEventRef` for persisted rows requires a positive committed global position.
- Used `{tenant_id}:{topic}:{source_event_id}` as the external idempotency key and verified duplicate `InMemoryPublisher::publish` calls do not create duplicate recorded effects.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope expansion; implementation stayed within the storage-neutral contract boundary.

## Issues Encountered

None.

## Known Stubs

None.

## TDD Gate Compliance

- RED commit present: `569e844`
- GREEN commit present after RED: `7e78ae2`
- Refactor commit not needed.

## Verification

- `cargo test -p es-outbox -- --nocapture` - PASS
- Acceptance checks for module declarations, error variants, DTO types, idempotency key surface, publisher trait, in-memory publisher APIs - PASS

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 06-02 can add PostgreSQL outbox schema and repository code against these contracts. The next plan should preserve the source-reference split and enforce tenant/source-event/topic idempotency at the database layer.

## Self-Check: PASSED

- Created files exist: summary, `error.rs`, `models.rs`, `publisher.rs`, and contract tests.
- Task commits exist in git history: `569e844`, `7e78ae2`.
- Stub scan found no TODO/FIXME/placeholder or empty mock-data patterns in files created or modified by this plan.

---
*Phase: 06-outbox-and-process-manager-workflows*
*Completed: 2026-04-18*

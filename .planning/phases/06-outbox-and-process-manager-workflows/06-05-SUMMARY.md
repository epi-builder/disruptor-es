---
phase: 06-outbox-and-process-manager-workflows
plan: 05
subsystem: integration
tags: [rust, process-manager, command-gateway, postgres, event-sourcing, tdd]

requires:
  - phase: 06-outbox-and-process-manager-workflows
    provides: "Outbox contracts, PostgreSQL outbox storage, append-time outbox rows, dispatcher orchestration, and durable process-manager offset primitives."
provides:
  - "Storage-neutral process-manager contracts, committed-event reader port, offset port, and batch helpers."
  - "PostgreSQL committed-event reader and process-manager offset adapters over PostgresEventStore and PostgresOutboxStore."
  - "Commerce OrderPlaced workflow that reserves inventory and confirms or rejects orders through CommandGateway."
  - "Unit and integration tests proving deterministic idempotency keys, reply waiting, metadata preservation, and durable offset timing."
affects: [phase-06, phase-07, es-outbox, es-store-postgres, es-runtime, app, process-manager]

tech-stack:
  added: [app dependencies on es-core, es-outbox, es-runtime, example-commerce, serde_json, time, tokio, uuid]
  patterns: [boxed-future-process-manager-port, committed-event-consumer, command-gateway-workflow, tdd-red-green]

key-files:
  created:
    - crates/es-outbox/src/process_manager.rs
    - crates/es-outbox/tests/process_manager.rs
    - crates/app/src/lib.rs
    - crates/app/src/commerce_process_manager.rs
  modified:
    - Cargo.lock
    - crates/app/Cargo.toml
    - crates/es-outbox/Cargo.toml
    - crates/es-outbox/src/lib.rs
    - crates/es-store-postgres/src/outbox.rs
    - crates/es-store-postgres/tests/outbox.rs

key-decisions:
  - "Keep process-manager contracts in es-outbox storage-neutral; app composes es-outbox, es-runtime, and example-commerce to avoid crate dependency cycles."
  - "Read process-manager batches from PostgresEventStore::read_global using the saved tenant-scoped offset before delegating to process_batch."
  - "Use deterministic follow-up idempotency keys in the form pm:{process_manager}:{source_event_id}:{action}:{target_id}."
  - "Advance durable process-manager offsets only after follow-up command replies complete or an event is intentionally skipped."

patterns-established:
  - "ProcessManager, CommittedEventReader, and ProcessManagerOffsetStore use futures::BoxFuture ports like other storage-neutral boundaries."
  - "ProcessEvent preserves committed event identity, tenant, command/correlation/causation IDs, payload, and metadata across PostgreSQL mapping."
  - "Commerce workflow follow-up commands copy tenant and correlation from the source event and set causation to the source event ID."

requirements-completed: [INT-04]

duration: 8min 48s
completed: 2026-04-18
---

# Phase 06 Plan 05: Commerce Process Manager Workflow Summary

**Committed-event process manager contracts with a commerce OrderPlaced workflow that reserves inventory, confirms or rejects orders, and advances PostgreSQL offsets after replies.**

## Performance

- **Duration:** 8min 48s
- **Started:** 2026-04-18T08:32:18Z
- **Completed:** 2026-04-18T08:41:06Z
- **Tasks:** 1
- **Files modified:** 10

## Accomplishments

- Added `ProcessEvent`, `ProcessOutcome`, `ProcessManager`, `CommittedEventReader`, `ProcessManagerOffsetStore`, `process_batch`, and `process_committed_batch` to `es-outbox`.
- Implemented PostgreSQL adapters that map `StoredEvent` into `ProcessEvent` and persist tenant-scoped process-manager offsets through `PostgresOutboxStore`.
- Added `CommerceOrderProcessManager` in the app composition crate so workflow code can depend on `es-outbox`, `es-runtime`, and `example-commerce` without adding those dependencies to `es-outbox`.
- Proved `OrderPlaced -> ReserveInventory -> ConfirmOrder/RejectOrder` through real `CommandGateway` receivers, deterministic idempotency keys, and reply-gated completion.
- Added PostgreSQL integration coverage proving committed events are read by global position and offsets advance only after processing returns, with source event metadata preserved.

## Task Commits

This TDD task was committed through the required gates:

1. **RED: Add failing process manager workflow tests** - `b563832` (test)
2. **GREEN: Implement commerce process manager workflow** - `39b4458` (feat)

**Plan metadata:** committed after summary creation.

## Files Created/Modified

- `Cargo.lock` - Records app test dependency resolution from the RED gate.
- `crates/app/Cargo.toml` - Adds app composition dependencies and a dev dependency on `es-store-postgres` for test fixtures.
- `crates/app/src/lib.rs` - Exports the app composition workflow module.
- `crates/app/src/commerce_process_manager.rs` - Implements `CommerceOrderProcessManager` and gateway-driven workflow tests.
- `crates/es-outbox/Cargo.toml` - Adds `tokio` dev dependency for async process-manager contract tests.
- `crates/es-outbox/src/lib.rs` - Exports process-manager contracts and helpers.
- `crates/es-outbox/src/process_manager.rs` - Defines storage-neutral process-manager contracts and batch helpers.
- `crates/es-outbox/tests/process_manager.rs` - Verifies offset filtering, skipped event advancement, and committed-event reader offset use.
- `crates/es-store-postgres/src/outbox.rs` - Implements `ProcessManagerOffsetStore` and `CommittedEventReader` adapters and `StoredEvent` mapping.
- `crates/es-store-postgres/tests/outbox.rs` - Adds the PostgreSQL INT-04 process-manager offset and metadata preservation test.

## Decisions Made

- Kept `es-outbox` free of `es-runtime` and `example-commerce`; the concrete workflow belongs in `app` to preserve the workspace dependency graph.
- Treated source event decode failures as `OutboxError::PayloadDecode`, which stops processing without advancing the offset.
- Treated product reservation command errors as workflow data that drive `RejectOrder`; final order command errors still surface as process-manager command submission failures.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed async workflow tests to poll process futures**
- **Found during:** Task 06-05-01 GREEN verification.
- **Issue:** The initial app tests pinned `manager.process(&event)` but waited on gateway receivers without polling the process future, so no follow-up commands were submitted.
- **Fix:** Drove process-manager execution with Tokio tasks in the tests and waited on the task only after command replies were sent.
- **Files modified:** `crates/app/src/commerce_process_manager.rs`
- **Verification:** `cargo test -p app commerce_process_manager -- --nocapture` passed.
- **Committed in:** `39b4458`

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** Test harness correction only; planned behavior and architecture were unchanged.

## Issues Encountered

- The RED gate initially also exposed a missing direct `tokio` dev dependency in `es-outbox`; it was added with the RED test commit so async contract tests compile and fail on the intended missing process-manager API.

## Known Stubs

None.

## TDD Gate Compliance

- RED commit present: `b563832`
- GREEN commit present after RED: `39b4458`
- Refactor commit not needed.

## Threat Flags

None - no new network endpoints, auth paths, file access patterns, or unplanned trust-boundary schema changes were introduced. The planned committed-event-to-workflow-command and process-manager-offset trust boundaries were implemented with tenant/correlation propagation and deterministic idempotency keys.

## Verification

- `cargo test -p es-outbox process_manager -- --nocapture && cargo test -p app commerce_process_manager -- --nocapture && cargo test -p es-store-postgres --test outbox process_manager_advances_postgres_offset_after_gateway_replies -- --test-threads=1 --nocapture` - PASS
- `cargo test -p es-outbox process_manager -- --nocapture && cargo test -p app commerce_process_manager -- --nocapture` - PASS
- Acceptance greps for exports, process-manager contracts, PostgreSQL adapters, dependency-cycle avoidance, app workflow structure, deterministic idempotency keys, and test names - PASS
- Stub scan for TODO/FIXME/placeholder or hardcoded empty UI data patterns in touched files - PASS

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 06 now satisfies INT-04. Phase 07 can compose adapters and observability over command gateways, projections, outbox dispatch, and the process-manager workflow without treating disruptor rings as durable state.

## Self-Check: PASSED

- Created files exist: `crates/es-outbox/src/process_manager.rs`, `crates/es-outbox/tests/process_manager.rs`, `crates/app/src/lib.rs`, `crates/app/src/commerce_process_manager.rs`, and this summary.
- Task commits exist in git history: `b563832`, `39b4458`.
- Required verification commands passed after the GREEN commit.
- Stub scan found no TODO/FIXME/placeholder or empty mock-data patterns in files created or modified by this plan.

---
*Phase: 06-outbox-and-process-manager-workflows*
*Completed: 2026-04-18*

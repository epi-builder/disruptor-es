---
phase: 07-adapters-observability-stress-and-template-guidance
plan: 07
subsystem: observability
tags: [rust, postgres, sqlx, metrics, stress, cqrs]

requires:
  - phase: 07-adapters-observability-stress-and-template-guidance
    provides: Phase 07 adapter, observability, integration, stress, and documentation baseline
provides:
  - Tenant durable max-position projection lag computation
  - PostgreSQL backlog regression coverage for es_projection_lag
  - Runtime shard depth sampling for stress reports
  - Measured append latency and projection lag in single-service stress reports
affects: [phase-07, observability, stress, projection, runtime]

tech-stack:
  added: []
  patterns:
    - Tenant-scoped max(global_position) is the source for projection freshness metrics.
    - Stress append latency is measured inside a RuntimeEventStore wrapper around append.
    - Runtime depth reporting is read-only and derived from shard-local queues.

key-files:
  created:
    - .planning/phases/07-adapters-observability-stress-and-template-guidance/07-07-SUMMARY.md
  modified:
    - crates/es-store-postgres/src/projection.rs
    - crates/es-store-postgres/tests/phase7_integration.rs
    - crates/es-runtime/src/engine.rs
    - crates/app/src/stress.rs

key-decisions:
  - "Projection lag is computed from tenant-scoped durable event-store max global position rather than fetched batch size."
  - "Single-service stress append latency is recorded around RuntimeEventStore::append instead of command round-trip latency."
  - "Stress shard depth samples read-only runtime shard state without exposing mutable shard internals."

patterns-established:
  - "Metrics regression tests can install metrics::set_default_local_recorder on current-thread Tokio tests to capture exact gauge values."
  - "Stress report fields must be sourced from the component named by the field, not synthetic proxies."

requirements-completed: [OBS-02, TEST-02, TEST-04]

duration: 10min 42s
completed: 2026-04-19T02:10:54Z
---

# Phase 07 Plan 07: Projection and Stress Signal Gap Closure Summary

**Durable projection lag and single-service stress signals now come from tenant event-store positions, runtime shard queues, and measured append calls.**

## Performance

- **Duration:** 10min 42s
- **Started:** 2026-04-19T02:00:13Z
- **Completed:** 2026-04-19T02:10:54Z
- **Tasks:** 3
- **Files modified:** 4 source/test files plus this summary

## Accomplishments

- Fixed `es_projection_lag` so idle and applied catch-up paths use the tenant's durable latest `events.global_position`.
- Added a PostgreSQL integration test that proves backlog lag is not batch-local and ignores other tenants.
- Added `CommandEngine::shard_depths()` as a read-only queue-depth sample for stress reporting.
- Replaced stress append-latency and shard-depth proxies with measured append durations and runtime shard depth samples.
- Fixed stress projection lag to return remaining tenant backlog after bounded catch-up, with controlled backlog regression coverage.

## Task Commits

1. **Task 07-07-01: Compute projection lag from tenant durable max global position**
   - `30e8af0` test: add projection lag backlog regression
   - `2da8dd8` fix: compute projection lag from durable tenant position
2. **Task 07-07-02: Expose measured runtime shard depth and append latency to stress**
   - `86e6e8a` feat: measure stress append latency and shard depth
3. **Task 07-07-03: Return measured stress projection lag and add backlog regression coverage**
   - `ba77b8e` test: add stress projection lag backlog regression
   - `fa3fb28` fix: return measured stress projection lag

## Files Created/Modified

- `crates/es-store-postgres/src/projection.rs` - Adds tenant durable latest-position helper and emits lag from durable backlog in idle and applied paths.
- `crates/es-store-postgres/tests/phase7_integration.rs` - Adds `ProjectionLagRecorder`, gauge capture, tenant backlog fixture, and backlog-sized lag regression.
- `crates/es-runtime/src/engine.rs` - Adds read-only `shard_depths()` derived from pending accepted commands and processable handoffs.
- `crates/app/src/stress.rs` - Adds measured runtime store wrapper, measured append latency histogram input, runtime shard-depth sampling, tenant max-position projection lag, and controlled backlog regression test.

## Decisions Made

- Projection freshness uses `SELECT COALESCE(max(global_position), 0) FROM events WHERE tenant_id = $1` so lag is tenant scoped and durable.
- The projection gauge is emitted after the projection transaction commits for applied batches, preserving committed-offset semantics.
- Stress append latency is measured inside a local `RuntimeEventStore` wrapper instead of changing runtime/store public contracts.
- Stress projection lag remains outside the command success gate; command success is still based only on command replies.

## Deviations from Plan

None - plan executed as written.

## Known Stubs

None.

## Issues Encountered

- The Task 1 RED test failed as intended with observed lag `25.0` and expected durable backlog `225.0`.
- The Task 3 RED test failed as intended because `sample_projection_lag` returned zero while a controlled backlog remained.
- `cargo test --workspace --no-run` emitted an existing missing-docs warning in `crates/es-runtime/tests/shard_disruptor.rs`; it did not fail and was outside this plan's scope.

## Verification

- `cargo test -p es-store-postgres phase7_projection_lag_uses_tenant_durable_backlog_not_batch_size -- --nocapture` passed.
- `cargo test -p app stress_projection_lag_reports_controlled_backlog -- --nocapture` passed.
- `cargo test -p app single_service_stress_smoke -- --nocapture` passed.
- `cargo test --workspace --no-run` passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 07 verifier gaps for OBS-02, TEST-02, and TEST-04 are closed at the implementation and regression-test level. Remaining advisory review items WR-01 through WR-03 are still outside this gap-closure plan and should remain separate follow-up work.

## Self-Check: PASSED

- Found summary and all key modified files.
- Found task commits `30e8af0`, `2da8dd8`, `86e6e8a`, `ba77b8e`, and `fa3fb28`.

---
*Phase: 07-adapters-observability-stress-and-template-guidance*
*Completed: 2026-04-19T02:10:54Z*

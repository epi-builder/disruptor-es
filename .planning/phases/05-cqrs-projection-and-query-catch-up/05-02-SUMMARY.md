---
phase: 05-cqrs-projection-and-query-catch-up
plan: 02
subsystem: domain
tags: [rust, serde, serde_json, cqrs, projection, example-commerce]

requires:
  - phase: 04-commerce-fixture-domain
    provides: typed commerce user, product, and order events
provides:
  - serde-enabled commerce ID/value/event payload types
  - JSON round-trip tests for order and product projection payloads
affects: [05-cqrs-projection-and-query-catch-up, es-store-postgres-projections]

tech-stack:
  added: [serde, serde_json]
  patterns:
    - serde derives on closed domain event DTOs for storage-side projection decode
    - serde_json::Value round-trip tests for committed event payload shapes

key-files:
  created:
    - .planning/phases/05-cqrs-projection-and-query-catch-up/05-02-SUMMARY.md
  modified:
    - crates/example-commerce/Cargo.toml
    - crates/example-commerce/src/ids.rs
    - crates/example-commerce/src/order.rs
    - crates/example-commerce/src/product.rs

key-decisions:
  - "Keep serde support inside example-commerce limited to event payload DTOs and value objects."
  - "Use serde_json only as a dev dependency for projection payload round-trip tests."

patterns-established:
  - "Projection payload contract tests serialize commerce events to serde_json::Value and decode back into typed events."

requirements-completed: [PROJ-02]

duration: 3min
completed: 2026-04-18
---

# Phase 05 Plan 02: Serde-Backed Commerce Projection Payloads Summary

**Commerce order and product event payloads now round-trip through serde_json::Value for typed projector decoding.**

## Performance

- **Duration:** 3min
- **Started:** 2026-04-18T00:24:28Z
- **Completed:** 2026-04-18T00:27:18Z
- **Tasks:** 2 completed
- **Files modified:** 5

## Accomplishments

- Added `serde` support to commerce IDs, quantities, order payload types, and product payload types without introducing runtime, storage, adapter, SQLx, or Tokio dependencies.
- Added four projection payload round-trip tests for `OrderPlaced`, `OrderRejected`, `ProductCreated`, and `InventoryReserved`.
- Verified targeted example-commerce checks and the dependency boundary integration test.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add serde derives to commerce projection payload types** - `88d5ac4` (feat)
2. **Task 2: Add commerce event JSON round-trip tests** - `265096c` (test)

**Plan metadata:** this summary commit (docs)

## Files Created/Modified

- `crates/example-commerce/Cargo.toml` - Adds `serde` dependency and `serde_json` test dependency.
- `crates/example-commerce/src/ids.rs` - Derives `Deserialize` and `Serialize` for commerce ID and quantity value objects.
- `crates/example-commerce/src/order.rs` - Derives serde for order projection payload types and adds order event JSON round-trip tests.
- `crates/example-commerce/src/product.rs` - Derives serde for product events and adds product event JSON round-trip tests.
- `.planning/phases/05-cqrs-projection-and-query-catch-up/05-02-SUMMARY.md` - Captures execution result.

## Verification

- `cargo check -p example-commerce` - PASS
- `cargo test -p example-commerce projection_payload -- --nocapture` - PASS, 4 projection payload tests passed
- `cargo test -p example-commerce dependency_boundaries -- --nocapture` - PASS, command exited 0
- `cargo test -p example-commerce --test dependency_boundaries -- --nocapture` - PASS, 3 dependency boundary tests passed
- `cargo test --workspace` - FAIL outside this plan's ownership; `crates/es-projection/tests/minimum_position.rs` has 3 failing Plan 05-01 tests (`minimum_position_rejects_empty_projector_name`, `minimum_position_rejects_negative_required_position`, `minimum_position_rejects_invalid_batch_limits`)

## Decisions Made

- Kept serde derives on closed Rust event/value types rather than introducing projection DTOs in the domain crate.
- Added `serde_json` only under `dev-dependencies` because production projection decoding belongs to storage/projection crates.

## Deviations from Plan

None - plan executed as written for files owned by 05-02.

## Issues Encountered

- Task 2 was marked `tdd="true"`, but its tests were already expected to pass after Task 1 added serde support. The coverage was committed as a test-only task commit.
- The workspace wave gate failed in Plan 05-01-owned `es-projection` tests. This executor did not modify those files.
- `Cargo.lock` was modified by Cargo while Plan 05-01 changes were present in the workspace; it was left unstaged because it is outside this executor's ownership list.

## Known Stubs

None found in files modified by this plan.

## Threat Flags

None - this plan adds serialization derives and tests only; it does not introduce endpoints, file access, auth paths, schema changes, or new trust boundaries beyond the plan's stored JSON payload decode boundary.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

05-02 targeted outputs are ready for Plan 05-03 projection decoding work. Wave 2 should wait for Plan 05-01's `es-projection` constructor validation failures and shared `Cargo.lock` state to be resolved by the owning executor/orchestrator.

## Self-Check: PASSED

- Found summary file: `.planning/phases/05-cqrs-projection-and-query-catch-up/05-02-SUMMARY.md`
- Found Task 1 commit: `88d5ac4`
- Found Task 2 commit: `265096c`
- Confirmed no tracked file deletions in task commits.

---
*Phase: 05-cqrs-projection-and-query-catch-up*
*Completed: 2026-04-18*

---
phase: 05-cqrs-projection-and-query-catch-up
plan: 01
subsystem: projection
tags: [rust, cqrs, projection, query, tokio]

requires:
  - phase: 04-commerce-fixture-domain
    provides: committed commerce events for later projection payload decoding
provides:
  - storage-neutral projection event and projector contracts
  - validated projector names, offsets, minimum positions, and batch limits
  - bounded minimum-position query wait helper with typed lag errors
affects: [05-cqrs-projection-and-query-catch-up, es-store-postgres-projections, query-catch-up]

tech-stack:
  added: [es-core, serde, serde_json, thiserror, tokio, time, anyhow]
  patterns:
    - boxed-future projector trait instead of async-trait
    - tenant-scoped projector offsets
    - bounded query freshness waits returning ProjectionLag

key-files:
  created:
    - crates/es-projection/src/error.rs
    - crates/es-projection/src/checkpoint.rs
    - crates/es-projection/src/projector.rs
    - crates/es-projection/src/query.rs
    - crates/es-projection/tests/minimum_position.rs
    - .planning/phases/05-cqrs-projection-and-query-catch-up/05-01-SUMMARY.md
  modified:
    - crates/es-projection/Cargo.toml
    - crates/es-projection/src/lib.rs

key-decisions:
  - "Keep es-projection storage-neutral; PostgreSQL StoredEvent conversion remains in es-store-postgres."
  - "Use typed constructors to reject invalid projector names, positions, and batch limits before storage calls."
  - "Minimum-position query waits are bounded by timeout and return ProjectionLag instead of blocking indefinitely."

patterns-established:
  - "Projection contracts expose storage-neutral DTOs and traits from es-projection, with storage implementations mapping into them later."
  - "Query freshness uses MinimumGlobalPosition plus WaitPolicy, preserving eventual consistency without gating command success."

requirements-completed: [PROJ-01, PROJ-03, PROJ-04]

duration: 5min 30s
completed: 2026-04-18
---

# Phase 05 Plan 01: Projection Contracts and Minimum-Position Wait Summary

**Storage-neutral projection contracts with validated checkpoints and bounded read-your-own-write query waits.**

## Performance

- **Duration:** 5min 30s
- **Started:** 2026-04-18T00:24:28Z
- **Completed:** 2026-04-18T00:29:58Z
- **Tasks:** 3 completed
- **Files modified:** 8

## Accomplishments

- Added public `es-projection` contracts for projector names, offsets, minimum positions, batch limits, projection events, projector handlers, catch-up outcomes, and query freshness checks.
- Added typed validation for empty projector names, negative global positions, invalid batch limits, and invalid wait policies.
- Implemented `wait_for_minimum_position` with Tokio sleep, explicit deadline, and `ProjectionError::ProjectionLag` on freshness timeout.
- Added minimum-position unit coverage for validation, freshness comparison, lag timeout, wait-policy rejection, and catch-up outcome fields.

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire projection crate modules and dependencies** - `0fea0bc` (feat)
2. **Task 2 RED: Define validated checkpoint and projector contracts tests** - `9bdf45c` (test)
3. **Task 2 GREEN: Define validated checkpoint and projector contracts** - `34541f1` (feat)
4. **Task 3 RED: Implement bounded minimum-position wait policy tests** - `ed0b6ff` (test)
5. **Task 3 GREEN: Implement bounded minimum-position wait policy** - `73f2ced` (feat)
6. **Formatting cleanup** - `8b9679e` (refactor)

**Plan metadata:** pending final docs commit

## Files Created/Modified

- `crates/es-projection/Cargo.toml` - Adds projection contract dependencies and test dependency.
- `crates/es-projection/src/lib.rs` - Declares private modules, re-exports public projection contracts, and updates the phase boundary marker.
- `crates/es-projection/src/error.rs` - Defines `ProjectionError` variants and `ProjectionResult`.
- `crates/es-projection/src/checkpoint.rs` - Defines validated `ProjectorName`, `MinimumGlobalPosition`, `ProjectionBatchLimit`, and tenant-scoped `ProjectorOffset`.
- `crates/es-projection/src/projector.rs` - Defines storage-neutral `ProjectionEvent`, `CatchUpOutcome`, and boxed-future `Projector` trait.
- `crates/es-projection/src/query.rs` - Defines `WaitPolicy`, `FreshnessCheck`, and `wait_for_minimum_position`.
- `crates/es-projection/tests/minimum_position.rs` - Covers PROJ-04 minimum-position and validation behavior.
- `.planning/phases/05-cqrs-projection-and-query-catch-up/05-01-SUMMARY.md` - Captures execution result.

## Verification

- `cargo fmt -p es-projection --check` - PASS
- `cargo check -p es-projection` - PASS
- `cargo test -p es-projection minimum_position -- --nocapture` - PASS, 8 tests passed
- `cargo test --workspace` - PASS

## Decisions Made

- Kept `es-projection` free of `es-store-postgres` so Plan 03 can own PostgreSQL-specific conversion and storage errors.
- Used `ProjectionError::InvalidBatchLimit` for invalid wait-policy durations, matching the plan's allowed typed validation path.
- Kept command success independent from projection catch-up; `wait_for_minimum_position` is query-side only.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added compile-ready module skeletons during Task 1**
- **Found during:** Task 1 (Wire projection crate modules and dependencies)
- **Issue:** `lib.rs` had to declare and re-export four new modules, but the module files did not exist yet, so `cargo check -p es-projection` could not pass after Task 1 with only `Cargo.toml` and `lib.rs` edits.
- **Fix:** Added minimal storage-neutral module files with the planned public types, then completed validation and wait behavior in later TDD commits.
- **Files modified:** `crates/es-projection/src/error.rs`, `crates/es-projection/src/checkpoint.rs`, `crates/es-projection/src/projector.rs`, `crates/es-projection/src/query.rs`
- **Verification:** `cargo check -p es-projection` passed after Task 1; all final plan checks passed.
- **Committed in:** `0fea0bc`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The auto-fix was required to satisfy the Task 1 compile gate. No storage dependency or scope creep was introduced.

## Issues Encountered

- Cargo commands briefly waited on package/build locks while a parallel executor was active. Commands were allowed to complete normally.
- A parallel Plan 05-02 executor created unrelated workspace changes; this plan only staged and committed `es-projection` and `05-01-SUMMARY.md` files.

## Known Stubs

None - stub scan found no TODO/FIXME/placeholder or hardcoded empty UI data patterns in files created or modified by this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 03 can map PostgreSQL stored events into `ProjectionEvent`, persist tenant-scoped projector offsets, and use `wait_for_minimum_position` from query repositories. Plan 02 ran concurrently and produced commerce payload serde support for read-model decoding.

## Self-Check: PASSED

- Verified created files exist: `error.rs`, `checkpoint.rs`, `projector.rs`, `query.rs`, `minimum_position.rs`, and this summary.
- Verified commits exist: `0fea0bc`, `9bdf45c`, `34541f1`, `ed0b6ff`, `73f2ced`, `8b9679e`.

---
*Phase: 05-cqrs-projection-and-query-catch-up*
*Completed: 2026-04-18*

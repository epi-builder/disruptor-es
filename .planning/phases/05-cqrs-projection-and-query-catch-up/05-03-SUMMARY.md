---
phase: 05-cqrs-projection-and-query-catch-up
plan: 03
subsystem: projection
tags: [rust, cqrs, postgres, sqlx, testcontainers]

requires:
  - phase: 05-cqrs-projection-and-query-catch-up
    provides: projection contracts and commerce serde payload support from plans 05-01 and 05-02
provides:
  - PostgreSQL projection offsets and commerce read-model schema
  - PostgresProjectionStore catch-up and freshness-aware query methods
  - containerized projection integration coverage for PROJ-01 through PROJ-04
affects: [05-cqrs-projection-and-query-catch-up, es-store-postgres-projections, query-catch-up]

tech-stack:
  added: [es-projection, example-commerce]
  patterns:
    - tenant-scoped projector offsets committed with read-model writes
    - bounded minimum-position query waits against read-model freshness
    - Testcontainers PostgreSQL projection tests serialized within one test file

key-files:
  created:
    - crates/es-store-postgres/migrations/20260418000000_projection_read_models.sql
    - crates/es-store-postgres/src/projection.rs
    - crates/es-store-postgres/tests/projections.rs
    - .planning/phases/05-cqrs-projection-and-query-catch-up/05-03-SUMMARY.md
  modified:
    - Cargo.lock
    - crates/es-store-postgres/Cargo.toml
    - crates/es-store-postgres/src/lib.rs

key-decisions:
  - "Keep PostgreSQL StoredEvent to ProjectionEvent conversion inside es-store-postgres."
  - "Query freshness waits use read-model last_applied_global_position because query methods are row-specific and do not accept a projector name."
  - "Serialize the projection integration tests to avoid Testcontainers PostgreSQL startup races under default Rust test parallelism."

patterns-established:
  - "Projection catch-up reads committed events with PostgresEventStore::read_global and commits read-model upserts plus projector_offsets in one SQLx transaction."
  - "Handled malformed commerce payloads return ProjectionError::PayloadDecode and leave projector_offsets unchanged."
  - "Read-model queries accept MinimumGlobalPosition plus WaitPolicy and return ProjectionLag after bounded polling."

requirements-completed: [PROJ-01, PROJ-02, PROJ-03, PROJ-04]

duration: 10min 49s
completed: 2026-04-18
---

# Phase 05 Plan 03: PostgreSQL Projection Store Summary

**PostgreSQL-backed commerce projections now catch up from committed events, atomically checkpoint offsets, and serve bounded freshness-aware read-model queries.**

## Performance

- **Duration:** 10min 49s
- **Started:** 2026-04-18T00:33:39Z
- **Completed:** 2026-04-18T00:44:28Z
- **Tasks:** 3 completed
- **Files modified:** 7

## Accomplishments

- Added `projector_offsets`, `order_summary_read_models`, and `product_inventory_read_models` with tenant-scoped primary keys.
- Implemented `PostgresProjectionStore` with atomic catch-up, commerce payload decoding, projector offset persistence, and row-specific minimum-position query waits.
- Added PostgreSQL integration tests for offset/read-model atomicity, commerce read models, restart resume, bounded lag errors, tenant isolation, and malformed payload handling.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add tenant-scoped projection schema and store exports** - `d753260` (feat)
2. **Task 2 RED: Add failing projection decode tests** - `73504db` (test)
3. **Task 2 GREEN: Implement atomic catch-up and read-model queries** - `ffb2bca` (feat)
4. **Task 3: Prove projection catch-up, restart, idempotence, and tenant isolation** - `3818672` (test)

**Plan metadata:** this docs commit

## Files Created/Modified

- `Cargo.lock` - Records new `es-store-postgres` path dependencies.
- `crates/es-store-postgres/Cargo.toml` - Adds `es-projection` and `example-commerce`.
- `crates/es-store-postgres/migrations/20260418000000_projection_read_models.sql` - Creates tenant-scoped projector offset and read-model tables.
- `crates/es-store-postgres/src/lib.rs` - Exports projection repository and read-model DTOs.
- `crates/es-store-postgres/src/projection.rs` - Implements catch-up, SQLx transaction writes, payload decoding, bounded query waits, and projection unit tests.
- `crates/es-store-postgres/tests/projections.rs` - Adds Testcontainers PostgreSQL integration coverage for PROJ-01 through PROJ-04.
- `.planning/phases/05-cqrs-projection-and-query-catch-up/05-03-SUMMARY.md` - Captures execution result.

## Verification

- `cargo check -p es-store-postgres` - PASS
- `cargo test -p es-store-postgres projection::tests -- --nocapture` - PASS, 2 tests passed
- `cargo test -p es-store-postgres projections_offset_commits_with_read_models -- --nocapture` - PASS, 1 test passed
- `cargo test -p es-store-postgres projections_build_commerce_read_models -- --nocapture` - PASS on rerun after one transient PostgreSQL container EOF
- `cargo test -p es-store-postgres projections_resume_without_duplicate_effects -- --nocapture` - PASS, 1 test passed
- `cargo test -p es-projection minimum_position -- --nocapture` - PASS, 8 tests passed
- `cargo test -p es-store-postgres projections -- --nocapture` - PASS, 6 tests passed
- `cargo test --workspace` - PASS

## Decisions Made

- Kept `es-projection` storage-neutral; PostgreSQL error and stored-event conversion stay local to `es-store-postgres`.
- Used read-model row freshness for `order_summary` and `product_inventory` minimum-position waits because those query methods do not take a projector name.
- Serialized `tests/projections.rs` with a Tokio mutex after parallel Testcontainers starts produced intermittent PostgreSQL wire EOF errors.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added compile-ready projection module during Task 1**
- **Found during:** Task 1 (Add tenant-scoped projection schema and store exports)
- **Issue:** `mod projection;` and public exports required the module file to exist for `cargo check -p es-store-postgres`.
- **Fix:** Added initial DTO/store skeleton in `crates/es-store-postgres/src/projection.rs`, then replaced it with the full implementation in Task 2.
- **Files modified:** `crates/es-store-postgres/src/projection.rs`
- **Verification:** `cargo check -p es-store-postgres` passed.
- **Committed in:** `d753260`

**2. [Rule 1 - Bug] Prevented duplicate delta effects for repeated product update positions**
- **Found during:** Task 3 (Prove projection catch-up, restart, idempotence, and tenant isolation)
- **Issue:** Product inventory delta updates used `last_applied_global_position <= $5`, which would reapply additive deltas if the same event position were processed twice.
- **Fix:** Changed additive update guards to require `last_applied_global_position < $5`.
- **Files modified:** `crates/es-store-postgres/src/projection.rs`
- **Verification:** `cargo test -p es-store-postgres projections -- --nocapture` and `cargo test --workspace` passed.
- **Committed in:** `3818672`

**3. [Rule 3 - Blocking] Serialized PostgreSQL projection integration tests**
- **Found during:** Task 3 (Prove projection catch-up, restart, idempotence, and tenant isolation)
- **Issue:** Default parallel test execution intermittently produced PostgreSQL wire EOF errors while multiple Testcontainers instances started simultaneously.
- **Fix:** Added a file-local Tokio mutex so `tests/projections.rs` runs container-backed tests serially while preserving the plan's exact verification command.
- **Files modified:** `crates/es-store-postgres/tests/projections.rs`
- **Verification:** `cargo test -p es-store-postgres projections -- --nocapture` passed with 6 tests.
- **Committed in:** `3818672`

---

**Total deviations:** 3 auto-fixed (1 bug, 2 blocking)
**Impact on plan:** All deviations preserved the planned architecture and were required for compile correctness, idempotent projection behavior, or stable automated verification.

## Issues Encountered

- `cargo test -p es-store-postgres projections_build_commerce_read_models -- --nocapture` hit one transient PostgreSQL container EOF and passed immediately on rerun.
- `cargo test -p es-store-postgres projections -- --nocapture` reproduced the same EOF under parallel test execution; serializing the projection integration tests fixed it.
- Task 3 tests passed on first run because Task 2 had already implemented the behavior under test. The TDD RED gate was satisfied for Task 2; Task 3 was committed as test coverage over the completed implementation.

## Known Stubs

None - stub scan found no TODO/FIXME/placeholder or hardcoded empty UI data patterns in files created or modified by this plan.

## Threat Flags

None - this plan's new PostgreSQL schema, query methods, payload decode boundary, and tenant predicates are the threat surfaces explicitly covered by the plan threat model.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 5 projection storage is ready for verification: committed commerce events can drive PostgreSQL read models, projector restart resumes from saved tenant-scoped offsets, and query callers can request bounded read-your-own-write freshness.

## Self-Check: PASSED

- Verified created files exist: projection read-model migration, `projection.rs`, `tests/projections.rs`, and this summary.
- Verified commits exist: `d753260`, `73504db`, `ffb2bca`, and `3818672`.

---
*Phase: 05-cqrs-projection-and-query-catch-up*
*Completed: 2026-04-18*

---
phase: 02-durable-event-store-source-of-truth
plan: 01
subsystem: database
tags: [rust, postgres, sqlx, testcontainers, migrations]
requires:
  - phase: 01-workspace-and-typed-kernel-contracts
    provides: "Rust 2024 workspace, es-core metadata types, and es-store-postgres crate shell"
provides:
  - "Rust-1.85-compatible SQLx/PostgreSQL dependency catalog"
  - "Initial PostgreSQL event-store schema for streams, events, command deduplication, and snapshots"
  - "Reusable PostgreSQL 18 Testcontainers harness with SQLx migration smoke coverage"
affects: [phase-02, phase-03-command-runtime, phase-05-cqrs-projections, phase-06-outbox]
tech-stack:
  added: [sqlx-0.8.6, tokio-1.52.0, anyhow-1.0.102, testcontainers-0.25.0, testcontainers-modules-0.13.0]
  patterns: [tenant-scoped-storage, sqlx-migrations, postgres18-integration-harness]
key-files:
  created:
    - crates/es-store-postgres/migrations/20260417000000_event_store.sql
    - crates/es-store-postgres/tests/common/mod.rs
    - crates/es-store-postgres/tests/harness_smoke.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/es-store-postgres/Cargo.toml
    - .planning/phases/02-durable-event-store-source-of-truth/02-VALIDATION.md
key-decisions:
  - "Use SQLx 0.8.6 and Testcontainers 0.25.0/0.13.0 to stay compatible with the Rust 1.85 workspace floor."
  - "Use PostgreSQL identity global positions and Rust-supplied UUIDs; the migration does not use DB-side uuidv7 defaults."
  - "Connect to the local Testcontainers PostgreSQL instance with SSL disabled while preserving the postgres:18 test target."
patterns-established:
  - "Storage crate inherits workspace dependencies but remains isolated from runtime, adapter, projection, outbox, broker, and disruptor crates."
  - "Integration tests call a shared start_postgres helper that owns the container, exposes PgPool, and runs sqlx::migrate!(\"./migrations\")."
requirements-completed: [STORE-01, STORE-02, STORE-03, STORE-04, STORE-05]
duration: 7m23s
completed: 2026-04-16
---

# Phase 02 Plan 01: PostgreSQL Storage Foundation Summary

**PostgreSQL event-store schema with SQLx workspace wiring and a PostgreSQL 18 migration smoke-test harness**

## Performance

- **Duration:** 7m23s
- **Started:** 2026-04-16T22:35:09Z
- **Completed:** 2026-04-16T22:42:31Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added the Rust-1.85-compatible SQLx, Tokio, anyhow, and Testcontainers dependency set and wired `es-store-postgres` without crossing storage boundaries.
- Created the first SQLx migration for tenant-scoped `streams`, `events`, `command_dedup`, and `snapshots` tables with revision, uniqueness, JSONB, foreign-key, and global-position constraints.
- Added a reusable PostgreSQL 18 integration-test harness that starts `postgres:18`, builds a `PgPool`, runs migrations, and verifies the four migrated tables exist.
- Updated Phase 02 validation to mark only this plan's schema/harness smoke coverage green while leaving `nyquist_compliant: false` and `wave_0_complete: false`.

## Task Commits

1. **Task 1: Add Rust-1.85-compatible storage dependencies** - `0343ec0` (feat)
2. **Task 2: Create the event-store migration** - `c1bd531` (feat)
3. **Task 3: Add PostgreSQL integration-test support and update validation** - `75d75f9` (feat)

**Plan metadata:** pending final docs commit

## Files Created/Modified

- `Cargo.toml` - Added workspace dependency pins for SQLx, Tokio, anyhow, Testcontainers, and Testcontainers PostgreSQL modules.
- `Cargo.lock` - Locked the new Rust-1.85-compatible dependency graph.
- `crates/es-store-postgres/Cargo.toml` - Added storage-only production dependencies plus test harness dev-dependencies.
- `crates/es-store-postgres/migrations/20260417000000_event_store.sql` - Defined event-store tables, tenant-scoped keys, constraints, and indexes.
- `crates/es-store-postgres/tests/common/mod.rs` - Added `PostgresHarness` and `start_postgres()` with PostgreSQL 18 and SQLx migrations.
- `crates/es-store-postgres/tests/harness_smoke.rs` - Added migrated-table smoke test using `to_regclass`.
- `.planning/phases/02-durable-event-store-source-of-truth/02-VALIDATION.md` - Reflected schema/harness coverage without marking Wave 0 complete.

## Decisions Made

- Kept `es-store-postgres` storage-only: no kernel, runtime, adapter, projection, outbox, broker, or disruptor dependencies were introduced.
- Used `Postgres::default().with_tag("18")` from `testcontainers-modules` instead of `GenericImage`, because the Rust-1.85-compatible module API supports the required image tag override.
- Disabled SSL in the local container database URL because SQLx's default SSL negotiation received an invalid response from the Testcontainers PostgreSQL endpoint.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Disabled SSL for the local PostgreSQL container connection**
- **Found during:** Task 3 (Add PostgreSQL integration-test support and update validation)
- **Issue:** The smoke test compiled, but SQLx failed to connect with `unexpected response from SSLRequest: 0x00`.
- **Fix:** Added `?sslmode=disable` to the local `postgres://postgres:postgres@127.0.0.1:{port}/postgres` test URL.
- **Files modified:** `crates/es-store-postgres/tests/common/mod.rs`
- **Verification:** `cargo test -p es-store-postgres --test harness_smoke -- --nocapture`
- **Committed in:** `75d75f9`

**2. [Rule 1 - Bug] Relaxed table-existence assertion to avoid PostgreSQL regclass display differences**
- **Found during:** Task 3 (Add PostgreSQL integration-test support and update validation)
- **Issue:** `to_regclass('public.events')::text` returned `events` because `public` is on the search path, causing an exact-string assertion failure despite the table existing.
- **Fix:** Asserted that `to_regclass` returned `Some(_)` for each required table.
- **Files modified:** `crates/es-store-postgres/tests/harness_smoke.rs`
- **Verification:** `cargo test -p es-store-postgres --test harness_smoke -- --nocapture`
- **Committed in:** `75d75f9`

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes were necessary for the planned harness smoke test to verify the migrated PostgreSQL 18 schema. No scope was added.

## Issues Encountered

- The first compile generated `Cargo.lock` after the Task 1 commit; it was amended into `0343ec0` so dependency and lockfile changes remain atomic.
- Docker/Testcontainers was available and successfully ran `postgres:18`; no authentication or manual setup gate was encountered.

## User Setup Required

None - no external service configuration required. Docker or a compatible container runtime is required to run the PostgreSQL-backed integration test locally.

## Known Stubs

None.

## Next Phase Readiness

Plans 02 through 04 can now build storage APIs and behavior tests against a real migrated PostgreSQL schema. The Wave 0 behavior files for append/OCC, dedupe, snapshots, and global reads remain pending by design.

## Self-Check: PASSED

- Verified all created/modified files exist.
- Verified task commits exist: `0343ec0`, `c1bd531`, `75d75f9`.

---
*Phase: 02-durable-event-store-source-of-truth*
*Completed: 2026-04-16*

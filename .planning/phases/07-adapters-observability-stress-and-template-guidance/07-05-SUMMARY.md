---
phase: 07-adapters-observability-stress-and-template-guidance
plan: 05
subsystem: testing
tags: [rust, stress, postgres, testcontainers, cqrs, outbox, hdrhistogram]

requires:
  - phase: 07-adapters-observability-stress-and-template-guidance
    provides: "HTTP/runtime boundaries, observability instrumentation, and PostgreSQL integration coverage from Plans 07-01 through 07-03"
provides:
  - Single-service integrated stress runner with bounded ingress and runtime execution
  - Stress report fields for throughput, latency percentiles, queue depths, append latency, projection lag, outbox lag, rejects, CPU, and cores
  - Thin `app stress-smoke` bootstrap that prints the stress report as JSON
affects: [TEST-03, TEST-04, OBS-02, phase-07-docs]

tech-stack:
  added: [hdrhistogram, sysinfo, sqlx, testcontainers, testcontainers-modules]
  patterns: [production-shaped in-process stress composition, command-success-before-projection-outbox-sampling, thin app bootstrap]

key-files:
  created:
    - crates/app/src/stress.rs
    - crates/app/migrations/20260417000000_event_store.sql
    - crates/app/migrations/20260418000000_projection_read_models.sql
    - crates/app/migrations/20260418010000_outbox.sql
  modified:
    - Cargo.lock
    - crates/app/Cargo.toml
    - crates/app/src/lib.rs
    - crates/app/src/main.rs

key-decisions:
  - "Stress command success is counted from durable command replies only; projection and outbox lag are sampled after command replies and never gate command success."
  - "The app crate owns a local Testcontainers PostgreSQL 18 harness and app-local migrations for `sqlx::migrate!(\"./migrations\")`."
  - "The binary remains a thin bootstrap that selects `stress-smoke`, invokes `run_single_service_stress`, and prints JSON."

patterns-established:
  - "Stress runners compose `PostgresEventStore`, `PostgresProjectionStore`, `PostgresOutboxStore`, `CommandGateway`, and `CommandEngine` in one process."
  - "Rejected stress commands are counted in `commands_rejected` and `reject_rate` instead of panicking."
  - "CLI bootstraps call app library composition code and do not import runtime/storage/projector/outbox internals."

requirements-completed: [TEST-04, TEST-03, OBS-02]

duration: 8min
completed: 2026-04-18
---

# Phase 07 Plan 05: Single-Service Stress Runner Summary

**Production-shaped single-process stress smoke now drives bounded runtime commands through PostgreSQL event storage, then samples projection and outbox lag outside the command success gate.**

## Performance

- **Duration:** 8min
- **Started:** 2026-04-18T14:58:05Z
- **Completed:** 2026-04-18T15:06:04Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Added `StressConfig`, `StressScenario`, `StressReport`, and `run_single_service_stress` in `crates/app/src/stress.rs`.
- Implemented five smoke scenarios: single-service integrated, full E2E in-process, hot-key, burst, and degraded dependency.
- Composed a Testcontainers PostgreSQL 18 harness with `PostgresEventStore`, `PostgresProjectionStore`, `PostgresOutboxStore`, bounded `CommandGateway`, and `CommandEngine`.
- Added `app stress-smoke` as a thin Tokio bootstrap that prints the required JSON report keys.

## Task Commits

Each task was committed atomically:

1. **Task 07-05-01 RED: Add failing stress runner smoke tests** - `4798442` (test)
2. **Task 07-05-01 GREEN: Implement single-service stress runner** - `61809bd` (feat)
3. **Task 07-05-02: Add thin stress bootstrap mode** - `fa979af` (feat)

**Plan metadata:** this docs commit

## Files Created/Modified

- `Cargo.lock` - Locked app stress dependencies.
- `crates/app/Cargo.toml` - Added app stress dependencies and adapter dev dependency.
- `crates/app/src/lib.rs` - Exported the `stress` module.
- `crates/app/src/stress.rs` - Added stress contracts, Testcontainers harness, command runtime drive loop, lag sampling, and smoke tests.
- `crates/app/src/main.rs` - Added thin `stress-smoke` CLI bootstrap and JSON output.
- `crates/app/migrations/20260417000000_event_store.sql` - App-local event-store migration for the stress harness.
- `crates/app/migrations/20260418000000_projection_read_models.sql` - App-local projection migration for the stress harness.
- `crates/app/migrations/20260418010000_outbox.sql` - App-local outbox migration for the stress harness.
- `.planning/phases/07-adapters-observability-stress-and-template-guidance/07-05-SUMMARY.md` - Execution summary.

## Decisions Made

- Command success is defined by successful runtime replies after durable append. Projection catch-up and outbox dispatch are sampled afterward, so CQRS/outbox lag is visible without redefining command success.
- The app crate carries local migrations because `sqlx::migrate!("./migrations")` resolves from the app crate manifest directory.
- Stress smoke uses intentionally small defaults so Testcontainers-backed verification remains practical during normal plan execution.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added app-local migrations for stress harness compilation**
- **Found during:** Task 07-05-01 (GREEN implementation)
- **Issue:** The plan required `sqlx::migrate!("./migrations")` in `crates/app/src/stress.rs`, but the app crate had no `migrations/` directory.
- **Fix:** Added app-local copies of the event-store, projection, and outbox migrations used by the stress Testcontainers harness.
- **Files modified:** `crates/app/migrations/20260417000000_event_store.sql`, `crates/app/migrations/20260418000000_projection_read_models.sql`, `crates/app/migrations/20260418010000_outbox.sql`
- **Verification:** `cargo test -p app single_service_stress_smoke -- --nocapture`; `cargo run -p app -- stress-smoke`; `cargo test --workspace --no-run`
- **Committed in:** `61809bd`

---

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** The deviation was required to run the specified app-owned migration macro and did not change the stress architecture.

## Issues Encountered

- `cargo test --workspace --no-run` emitted a pre-existing missing-docs warning for `crates/es-runtime/tests/shard_disruptor.rs`; compilation still passed and this plan did not modify that file.

## TDD Gate Compliance

- RED commit present: `4798442`
- GREEN commit present after RED: `61809bd`
- REFACTOR commit: not needed

## Known Stubs

None.

## Threat Flags

None - the CLI stress selection and bounded stress ingress trust boundaries were covered by the plan threat model.

## Verification

- `cargo test -p app single_service_stress_smoke -- --nocapture` - PASS
- `cargo run -p app -- stress-smoke` - PASS, printed `p95_micros`
- `cargo test --workspace --no-run` - PASS
- `rg 'StressConfig|StressScenario|SingleServiceIntegrated|FullE2eInProcess|HotKey|Burst|DegradedDependency|StressReport|run_single_service_stress|hdrhistogram|sysinfo|throughput_per_second|p50_micros|p95_micros|p99_micros|projection_lag|outbox_lag|reject_rate|cpu_utilization_percent|core_count' crates/app/src/stress.rs` - PASS
- `rg 'CommandGateway|try_submit|CommandEngine|process_one|oneshot' crates/app/src/stress.rs` - PASS
- `rg 'PostgresEventStore|PostgresProjectionStore|PostgresOutboxStore|testcontainers|testcontainers_modules::postgres::Postgres|with_tag\("18"\)|sqlx::migrate!\("./migrations"\)|catch_up|dispatch_once' crates/app/src/stress.rs` - PASS
- `rg 'stress-smoke|run_single_service_stress|usage: app stress-smoke' crates/app/src/main.rs` - PASS
- `! rg 'CommandEngine|PostgresEventStore|dispatch_once|catch_up' crates/app/src/main.rs` - PASS

## User Setup Required

None - no external service configuration required. Docker/Testcontainers availability is required for the stress smoke command.

## Next Phase Readiness

Plan 07-06 can document how to interpret integrated stress results separately from ring-only microbenchmarks, and can point template users at `run_single_service_stress` and `app stress-smoke` as the production-shaped local smoke path.

## Self-Check: PASSED

- Verified key created/modified files exist.
- Verified task commits exist: `4798442`, `61809bd`, `fa979af`.
- Verified plan-level commands passed.
- Verified `.planning/STATE.md` and `.planning/ROADMAP.md` were not modified by this executor.

---
*Phase: 07-adapters-observability-stress-and-template-guidance*
*Completed: 2026-04-18*

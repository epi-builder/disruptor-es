---
phase: 12-external-process-http-e2e-stress-and-benchmark-closure
plan: 02
subsystem: testing
tags: [rust, criterion, stress, benchmark, external-process, http]
requires:
  - phase: 12-external-process-http-e2e-stress-and-benchmark-closure
    provides: reusable external-process app harness and canonical HTTP fixtures
provides:
  - explicit external-process HTTP stress runner
  - dedicated Criterion benchmark lane for real-process HTTP traffic
  - in-process stress lane renamed away from full-E2E wording
  - updated operator/reporting guidance
affects: [testing, benchmark, docs, observability]
tech-stack:
  added: [reqwest]
  patterns: [external-process stress report parity, explicit workload-layer labels]
key-files:
  created:
    - crates/app/src/http_stress.rs
    - benches/external_process_http.rs
  modified:
    - Cargo.toml
    - crates/app/Cargo.toml
    - crates/app/src/lib.rs
    - crates/app/src/main.rs
    - crates/app/src/stress.rs
    - docs/stress-results.md
    - docs/template-guide.md
key-decisions:
  - "Rename the old full-E2E in-process scenario to `InProcessIntegrated` and reserve external-process wording for the real HTTP process boundary."
  - "Expose `app http-stress` as the reusable external-process HTTP stress entrypoint."
  - "Make the root benchmark depend on the app crate and resolve/build the app binary when Criterion runs outside `cargo test -p app`."
patterns-established:
  - "Archive-facing workload labels must distinguish ring-only, in-process integrated, and external-process HTTP measurements."
  - "External-process HTTP reports preserve throughput, percentile, depth, append-latency, lag, reject-rate, CPU, and core-count fields."
requirements-completed: [TEST-03, TEST-04, OBS-02]
duration: session
completed: 2026-04-25
---

# Phase 12 Plan 02: External-Process HTTP Stress And Benchmark Summary

**External-process HTTP stress and benchmark lane with report-field parity and explicit in-process vs real-process labels**

## Performance

- **Duration:** session
- **Started:** 2026-04-25
- **Completed:** 2026-04-25
- **Tasks:** 4
- **Files modified:** 10

## Accomplishments

- Renamed the misleading in-process `FullE2eInProcess` scenario to `InProcessIntegrated` and changed its stable label from `full-e2e` to `in-process-integrated`.
- Added `crates/app/src/http_stress.rs`, a reusable external-process HTTP runner that launches `app serve`, drives canonical requests, scrapes Prometheus metrics, and emits the required stress report fields.
- Added `app http-stress` and `benches/external_process_http.rs` for a dedicated external-process HTTP workload lane.
- Updated stress and template docs so report consumers can distinguish ring-only, in-process integrated, and external-process HTTP measurements.

## Task Commits

No commits created. The workspace had pre-existing dirty changes in files touched by this plan, so committing would have mixed this work with earlier uncommitted edits.

## Files Created/Modified

- `crates/app/src/http_stress.rs` - external-process HTTP runner, canonical fixtures, metric scraping, and smoke test.
- `benches/external_process_http.rs` - Criterion benchmark lane for real HTTP client/service-process overhead.
- `crates/app/src/main.rs` and `crates/app/src/lib.rs` - `http-stress` CLI and module export.
- `crates/app/src/stress.rs` - renamed in-process scenario and added external-process scenario label.
- `docs/stress-results.md` and `docs/template-guide.md` - workload-layer guidance and run commands.
- `Cargo.toml` and `crates/app/Cargo.toml` - benchmark registration, root app dev-dependency, and `reqwest` dependency.

## Decisions Made

- Keep external-process stress in app library code so tests, CLI, and Criterion call one implementation.
- Scrape the service Prometheus endpoint for external-process depth, append latency, projection lag, and outbox lag instead of reaching into in-process runtime state.
- Resolve the app binary through `CARGO_BIN_EXE_app` when present and otherwise build/use `target/{profile}/app` for root benchmark execution.

## Deviations from Plan

### Auto-fixed Issues

**1. Root benchmark could not find the app crate or binary**
- **Found during:** Task 12-02-03 verification.
- **Issue:** The root bench target did not depend on the `app` crate, and Criterion execution does not provide `CARGO_BIN_EXE_app`.
- **Fix:** Added `app = { path = "crates/app" }` as a root dev-dependency and taught the external-process runner to build/resolve the app binary when needed.
- **Files modified:** `Cargo.toml`, `crates/app/src/http_stress.rs`
- **Verification:** `cargo bench --bench external_process_http -- --sample-size 10` completed.

**Total deviations:** 1 auto-fixed.
**Impact on plan:** Required for the planned benchmark lane to be runnable from the documented command.

## Issues Encountered

Criterion initially projected a long run because the first warmup included release binary compilation. After the binary was built, the benchmark completed and reported `external_process_http_smoke` around 3.16s to 3.44s.

## User Setup Required

None for local verification beyond working Docker/Testcontainers support.

## Next Phase Readiness

Phase 12 coverage is ready for phase verification. Phase 13 can add steady-state live HTTP stress evidence before Phase 14 consumes the full evidence chain for archive sign-off.

---
*Phase: 12-external-process-http-e2e-stress-and-benchmark-closure*
*Completed: 2026-04-25*

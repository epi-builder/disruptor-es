---
phase: 12-external-process-http-e2e-stress-and-benchmark-closure
plan: 01
subsystem: testing
tags: [rust, axum, reqwest, testcontainers, external-process, http]
requires:
  - phase: 11-evidence-recovery-and-runnable-http-service
    provides: official `app serve` process path
provides:
  - reusable external-process app test harness
  - canonical real-process HTTP success and metadata contract tests
  - real-process HTTP error contract coverage
affects: [api, testing, stress, benchmark]
tech-stack:
  added: [reqwest]
  patterns: [shared process-test support, canonical HTTP request fixtures]
key-files:
  created:
    - crates/app/tests/support/mod.rs
    - crates/app/tests/support/http_process.rs
    - crates/app/tests/external_process_http.rs
  modified:
    - Cargo.toml
    - crates/app/Cargo.toml
    - crates/app/tests/serve_smoke.rs
key-decisions:
  - "Use reqwest for reusable external-process HTTP tests instead of raw TCP request strings."
  - "Keep canonical place-order fixtures in app library code so E2E, stress, and benchmarks share request shape."
patterns-established:
  - "External-process tests start the real `app serve` binary, wait on `/healthz`, and fail with child stdout/stderr context."
  - "HTTP contract assertions parse JSON and check durable metadata fields rather than matching only raw substrings."
requirements-completed: [API-01, API-03]
duration: session
completed: 2026-04-25
---

# Phase 12 Plan 01: External-Process HTTP E2E Harness Summary

**Reusable real-process HTTP harness with canonical success, metadata, and error contract coverage through `app serve`**

## Performance

- **Duration:** session
- **Started:** 2026-04-25
- **Completed:** 2026-04-25
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Extracted `crates/app/tests/support/http_process.rs` for app binary resolution, PostgreSQL container startup, readiness probing, HTTP requests, child logs, and shutdown.
- Slimmed `serve_smoke` so it reuses shared support and focuses on readiness plus one real command-path probe.
- Added `external_process_http_success_path`, `external_process_http_metadata_contract`, and `external_process_http_error_contracts` against the real `app serve` process.

## Task Commits

No commits created. The workspace had pre-existing dirty changes in files touched by this plan, so committing would have mixed this work with earlier uncommitted edits.

## Files Created/Modified

- `crates/app/tests/support/mod.rs` - test support module registration.
- `crates/app/tests/support/http_process.rs` - reusable external-process harness.
- `crates/app/tests/external_process_http.rs` - canonical real-process HTTP E2E tests.
- `crates/app/tests/serve_smoke.rs` - smoke test refactored onto shared support.
- `Cargo.toml` and `crates/app/Cargo.toml` - direct `reqwest` dependency for external-process HTTP clients.

## Decisions Made

- Use `reqwest` for external-process test clients because it provides stable status/header/body assertions and avoids duplicating raw HTTP framing.
- Put canonical place-order fixtures in `app::http_stress` so Plan 12-02 can reuse the same request shape for stress and benchmark traffic.

## Deviations from Plan

None - scope stayed within the planned harness and contract-test work.

## Issues Encountered

None for Plan 12-01 verification.

## User Setup Required

None - tests manage PostgreSQL through Testcontainers and spawn the local app binary.

## Next Phase Readiness

Ready for Plan 12-02. The real-process harness and canonical request fixture are available for stress and benchmark work.

---
*Phase: 12-external-process-http-e2e-stress-and-benchmark-closure*
*Completed: 2026-04-25*

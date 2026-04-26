---
phase: 13-live-external-process-http-steady-state-stress-testing
plan: 01
subsystem: testing
tags: [rust, http, stress, steady-state, external-process]
requires:
  - phase: 12-external-process-http-e2e-stress-and-benchmark-closure
    provides: reusable app serve harness, canonical HTTP fixtures, and Prometheus-backed external-process stress lane
provides:
  - bounded external HTTP stress profiles with upfront validation
  - warmup-to-measurement reset semantics on one long-lived app serve child
  - measured-window report metadata for deadline policy and host sampling
affects: [testing, benchmark, docs, observability]
tech-stack:
  added: []
  patterns: [validated steady-state stress presets, measured-only report accumulation, bounded drain-after-deadline policy]
key-files:
  created: []
  modified:
    - crates/app/src/http_stress.rs
    - crates/app/src/stress.rs
key-decisions:
  - "Keep target selection pinned to ExternalProcessHarness::spawn and validate all steady-state knobs before starting PostgreSQL or app serve."
  - "Measure live HTTP stress as warmup plus measured window plus bounded drain, and report only the measured interval."
patterns-established:
  - "External-process stress uses named presets plus exact numeric bounds before harness startup."
  - "Measured-window reports include deadline-policy and host-sampling metadata without retaining DATABASE_URL or environment maps."
requirements-completed: [TEST-04, OBS-02]
duration: 6 min
completed: 2026-04-26
---

# Phase 13 Plan 01: Live External-Process HTTP Steady-State Runner Summary

**Validated external HTTP steady-state profiles with warmup reset, bounded deadline drain, and measured-window host metadata**

## Performance

- **Duration:** 6 min
- **Started:** 2026-04-26T06:10:52Z
- **Completed:** 2026-04-26T06:16:50Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `HttpStressProfile` presets for `smoke`, `baseline`, `burst`, and `hot-key`, with exact bounds validation on warmup, measurement, concurrency, command count, shard count, ingress capacity, and ring size.
- Reworked the external-process runner into explicit readiness, warmup, and measured windows against one spawned `app serve` child, with stop-submit and 5-second drain semantics at the measured deadline.
- Extended `StressReport` and smoke coverage so measured-only outputs now include `commands_failed`, run-duration metadata, deadline policy, host OS/arch, CPU brand, and CPU usage samples without leaking `DATABASE_URL`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add bounded steady-state profiles and config validation** - `a41d44f` (feat)
2. **Task 2: Separate warmup from the measured window and enrich measured-only reporting** - `7af0208` (feat)

TDD red phase:

1. **Failing tests for Phase 13 runner behavior** - `1dac714` (test)

## Files Created/Modified

- `crates/app/src/http_stress.rs` - validated steady-state config model, duration-window runner, measured-only sampling, and Phase 13 regression coverage.
- `crates/app/src/stress.rs` - report schema extended for measured-window metadata and secret-safe serialization.

## Decisions Made

- Kept the runner deriving its target addresses only from `ExternalProcessHarness::spawn`, which preserves the localhost-only threat model and blocks arbitrary external retargeting.
- Used duration-driven submission with a bounded drain timeout instead of a startup-inclusive finite batch so the report represents steady-state behavior only.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - the task uses the existing local Testcontainers and spawned `app serve` harness.

## Next Phase Readiness

Plan 13-02 can now layer CLI controls, Criterion positioning, and documentation on top of a measured-window runner that already exposes the required metadata and bounded safety checks.

## Self-Check

PASSED - summary file exists and task commits `1dac714`, `a41d44f`, and `7af0208` are present in git history.

---
*Phase: 13-live-external-process-http-steady-state-stress-testing*
*Completed: 2026-04-26*

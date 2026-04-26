---
phase: 13-live-external-process-http-steady-state-stress-testing
plan: 02
subsystem: testing
tags: [rust, http, stress, criterion, cli, docs]
requires:
  - phase: 13-live-external-process-http-steady-state-stress-testing
    provides: validated steady-state runner profiles, measured-window metadata, and localhost-only harness behavior
provides:
  - configurable `app http-stress` CLI with bounded steady-state overrides
  - binary JSON output that exposes measured-window metadata for live HTTP runs
  - Criterion smoke wrapper explicitly demoted behind the authoritative live HTTP lane
  - operator docs that separate Phase 13 steady-state evidence from Phase 12 and Criterion output
affects: [testing, benchmark, docs, observability]
tech-stack:
  added: []
  patterns: [local CLI parsing without new dependencies, shared smoke-profile routing across CLI and Criterion, measured-window reporting guidance]
key-files:
  created: []
  modified:
    - crates/app/src/main.rs
    - crates/app/src/http_stress.rs
    - benches/external_process_http.rs
    - docs/stress-results.md
    - docs/template-guide.md
key-decisions:
  - "Keep `app http-stress` on a small local parser instead of adding a new CLI dependency, while still enforcing the exact bounded flag surface from the plan."
  - "Treat the live `app http-stress` JSON output as the authoritative Phase 13 evidence source and force Criterion to reuse the unmodified smoke preset."
patterns-established:
  - "Binary entrypoints must serialize the same measured-window metadata that library stress reports already expose."
  - "External-process smoke benches reuse named stress presets instead of maintaining bench-only workload variants."
requirements-completed: [TEST-03, TEST-04, OBS-02]
duration: 10 min
completed: 2026-04-26
---

# Phase 13 Plan 02: Live External-Process HTTP Steady-State Stress Testing Summary

**Configurable live HTTP steady-state CLI with measured-window JSON output, smoke-only Criterion reuse, and operator guidance for interpreting Phase 13 evidence**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-26T06:17:15Z
- **Completed:** 2026-04-26T06:27:23Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added bounded `app http-stress` flag parsing for `--profile`, warmup, measurement, concurrency, command count, shard count, ingress capacity, and ring size without introducing a new CLI dependency.
- Extended binary JSON output so live HTTP runs now print measured-window metadata such as `profile_name`, `run_duration_seconds`, `deadline_policy`, `drain_timeout_seconds`, host identity, and CPU samples.
- Reworked the Criterion wrapper to reuse the shared smoke preset and documented that Phase 13 steady-state JSON, not Criterion iteration output, is the archive-facing evidence path.
- Updated operator docs with exact Phase 13 commands, measured-window exclusions, and the distinction between Phase 12 benchmark output, Criterion smoke, and Phase 13 steady-state live HTTP reports.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add a configurable `app http-stress` CLI and measured-window JSON output** - `1ec5d15` (test), `240ac29` (feat)
2. **Task 2: Keep Criterion clearly secondary to the steady-state live lane** - `a4b8ab9` (test), `25389ef` (feat)
3. **Task 3: Update operator docs for steady-state live HTTP runs and report interpretation** - `1073d7e` (docs)

## Files Created/Modified

- `crates/app/src/main.rs` - local CLI parser, usage enforcement, and JSON serialization for steady-state report metadata.
- `crates/app/src/http_stress.rs` - shared binary-resolution fix for `cargo run -p app -- http-stress` and smoke-profile parity coverage for the Criterion lane.
- `benches/external_process_http.rs` - smoke-only Criterion wrapper routed through `HttpStressProfile::Smoke` with an explicit non-authoritative Phase 13 note.
- `docs/stress-results.md` - steady-state live HTTP interpretation rules, required output keys, and measured-window deadline semantics.
- `docs/template-guide.md` - exact Phase 13 run commands and operator guidance separating live HTTP steady-state output from Phase 12 and Criterion results.

## Decisions Made

- Kept the CLI surface bounded to local stress controls only; there is still no arbitrary target URL or remote host override.
- Reused the same smoke preset for Criterion and the live runner instead of preserving a smaller bench-only workload that could drift from the documented Phase 13 lane.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed `app http-stress` binary resolution under `cargo run`**
- **Found during:** Task 1 (Add a configurable `app http-stress` CLI and measured-window JSON output)
- **Issue:** The shared external-process harness only resolved the `app` binary correctly from test binaries under `target/*/deps`, so the plan-mandated `cargo run -p app -- http-stress ...` command failed with `No such file or directory`.
- **Fix:** Updated `app_binary()` to return the current executable when already running as `target/debug/app`, preserving the existing fallback build path for test and bench contexts.
- **Files modified:** `crates/app/src/http_stress.rs`
- **Verification:** `cargo run -p app -- http-stress --profile smoke --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 16 --shard-count 2 --ingress-capacity 8 --ring-size 16`
- **Committed in:** `240ac29` (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Required to make the documented live HTTP CLI runnable from the binary entrypoint. No scope creep beyond the planned external-process lane.

## Issues Encountered

- The first Task 1 red command used an invalid `cargo test` argument pattern and had to be rerun with a valid filter before the expected compile failure surfaced.

## User Setup Required

None - the verified commands use the existing local Testcontainers and spawned `app serve` harness.

## Next Phase Readiness

- Phase 13 now has one documented, bounded live HTTP steady-state command for archive-facing evidence plus a clearly secondary Criterion smoke wrapper.
- The verifier can use the documented `app http-stress` profiles and required JSON keys to compare sustained runs consistently across environments.

## Self-Check

PASSED - summary file exists and task commits `1ec5d15`, `240ac29`, `a4b8ab9`, `25389ef`, and `1073d7e` are present in git history.

---
*Phase: 13-live-external-process-http-steady-state-stress-testing*
*Completed: 2026-04-26*

---
phase: 12
slug: external-process-http-e2e-stress-and-benchmark-closure
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-25
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for external-process HTTP E2E, stress, and benchmark closure.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust integration tests, targeted CLI/workload runs, `rg` doc checks, and Criterion benches |
| **Quick run command** | `cargo test -p app serve_smoke -- --nocapture && cargo test -p app external_process_http -- --nocapture` |
| **Full suite command** | `cargo test --workspace && cargo bench --bench external_process_http -- --sample-size 10` |
| **Estimated runtime** | targeted app checks should stay within a few minutes; the external-process bench depends on local Docker/Testcontainers availability |

## Per-Task Verification Map

| Task ID | Plan | Requirement | Automated Command | Status |
|---------|------|-------------|-------------------|--------|
| 12-01-01 | 01 | API-01, API-03 | `cargo test -p app serve_smoke -- --nocapture` | planned |
| 12-01-02 | 01 | API-01, API-03 | `cargo test -p app external_process_http_success_path -- --nocapture && cargo test -p app external_process_http_metadata_contract -- --nocapture` | planned |
| 12-01-03 | 01 | API-01, API-03 | `cargo test -p app external_process_http_error_contracts -- --nocapture` | planned |
| 12-02-01 | 02 | TEST-03, TEST-04 | `cargo test -p app stress_smoke -- --nocapture && rg -n "FullE2eInProcess|full-e2e" crates/app/src/stress.rs docs/stress-results.md docs/template-guide.md` | planned |
| 12-02-02 | 02 | TEST-04, OBS-02 | `cargo test -p app external_process_http_stress_smoke -- --nocapture` | planned |
| 12-02-03 | 02 | TEST-03 | `cargo bench --bench external_process_http -- --sample-size 10` | planned |
| 12-02-04 | 02 | TEST-03, OBS-02 | `rg -n "external-process|app serve|throughput_per_second|p95|projection_lag|outbox_lag|reject_rate|cpu_utilization_percent" docs/stress-results.md docs/template-guide.md crates/app/src` | planned |

## Wave 0 Requirements

- [x] `.planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-RESEARCH.md`
- [x] `.planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-PATTERNS.md`
- [x] `.planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-01-PLAN.md`
- [x] `.planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-02-PLAN.md`
- [ ] `crates/app/tests/support/http_process.rs`
- [ ] `crates/app/tests/external_process_http.rs`
- [ ] `crates/app/src/http_stress.rs`
- [ ] `benches/external_process_http.rs`

## Sampling Rules

- **After harness extraction:** rerun `serve_smoke` immediately before building additional external-process scenarios.
- **After each new canonical HTTP scenario:** run only the relevant targeted test first, then the grouped `external_process_http` filter.
- **After renaming the in-process lane:** run the in-process stress smoke plus `rg` checks so no stale `FullE2eInProcess` / `full-e2e` strings remain in archive-facing locations.
- **After adding external-process stress reporting:** run the targeted stress smoke and check that the report still includes throughput, latency percentiles, queue depth, append latency, projection lag, outbox lag, reject rate, CPU, and core fields.
- **Before phase sign-off:** run the external-process bench target and record command/environment details next to the resulting numbers.

## Validation Audit Template

When execution completes, append a dated `Validation Audit YYYY-MM-DD` section with:

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

### Audit Evidence
- `cargo test -p app serve_smoke -- --nocapture`
- `cargo test -p app external_process_http -- --nocapture`
- `cargo test -p app external_process_http_stress_smoke -- --nocapture`
- `cargo test -p app stress_smoke -- --nocapture`
- `cargo bench --bench external_process_http -- --sample-size 10`

## Validation Sign-Off Criteria

- [ ] All plan tasks have at least one executable verification command.
- [ ] The external-process lane is clearly distinct from in-process and microbenchmark lanes.
- [ ] No archive-facing `FullE2eInProcess` / `full-e2e` label remains unless explicitly demoted as historical-only text.
- [ ] External-process reports preserve the required stress fields from `TEST-04` / `OBS-02`.
- [ ] Docs explain how to run and interpret the new lane.

**Approval:** pending

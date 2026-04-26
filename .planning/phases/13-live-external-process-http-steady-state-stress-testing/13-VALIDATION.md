---
phase: 13
slug: live-external-process-http-steady-state-stress-testing
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-26
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust integration tests + CLI smoke commands + docs grep |
| **Config file** | `Cargo.toml`, `crates/app/Cargo.toml` |
| **Quick run command** | `cargo test -p app external_process_http_stress_smoke -- --nocapture` |
| **Full suite command** | `cargo test --workspace && cargo run -p app -- http-stress` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p app external_process_http_stress_smoke -- --nocapture` or the task-specific grep command listed below.
- **After every plan wave:** Run `cargo test --workspace && cargo run -p app -- http-stress`.
- **Before `/gsd-verify-work`:** Full workspace tests and one documented live HTTP stress command must be green.
- **Max feedback latency:** 180 seconds for targeted checks; full suite may exceed this when Docker/Testcontainers startup is required.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 13-01-01 | 01 | 1 | TEST-04 | T-13-01 | Harness targets only its spawned localhost service | integration | `cargo test -p app external_process_http_stress_smoke -- --nocapture` | ✅ | ⬜ pending |
| 13-01-02 | 01 | 1 | TEST-03 | T-13-02 | Profile inputs are bounded and validated before load starts | unit/CLI | `cargo run -p app -- http-stress --profile smoke` | ❌ W0 | ⬜ pending |
| 13-02-01 | 02 | 2 | OBS-02 | T-13-03 | Reports exclude secret env values and expose bounded metric names only | integration/grep | `cargo test -p app external_process_http_stress_smoke -- --nocapture && rg -n "append_latency|projection_lag|outbox_lag|reject_rate|ingress_depth_max|shard_depth_max" crates/app/src/http_stress.rs docs/stress-results.md` | Partial | ⬜ pending |
| 13-03-01 | 03 | 3 | TEST-03, TEST-04 | — | Documentation separates smoke, live steady-state, and Criterion evidence | docs grep | `rg -n "steady-state|baseline|burst|hot-key|Criterion" docs/stress-results.md docs/template-guide.md crates/app/src` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/app/src/http_stress.rs` — explicit warmup duration and measurement duration configuration.
- [ ] `crates/app/src/http_stress.rs` — profile presets for `smoke`, `baseline`, `burst`, and `hot-key`.
- [ ] `crates/app/src/http_stress.rs` — histogram/counter reset after warmup and before measured window.
- [ ] `crates/app/src/http_stress.rs` — measured-window CPU sampling rather than one trailing sample.
- [ ] `docs/stress-results.md` and `docs/template-guide.md` — interpretation guidance separating Phase 13 steady-state live HTTP results from Phase 12 smoke benchmarks and Criterion microbenchmarks.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Measured interval excludes service boot, migrations, readiness probing, harness compilation, and PostgreSQL container startup | TEST-04 | Requires reading emitted timestamps/logs around the live run boundaries | Run the documented Phase 13 command, confirm the report has distinct warmup and measured-window fields, and confirm throughput/latency counters start after warmup completes. |
| Resource metadata matches the local run environment | OBS-02 | CPU/core count and host metadata are environment-dependent | Compare the report's CPU/core count and run duration fields with the local machine and command-line profile settings. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands or Wave 0 dependencies.
- [x] Sampling continuity: no 3 consecutive tasks without automated verify.
- [x] Wave 0 covers all missing references from research.
- [x] No watch-mode flags.
- [x] Feedback latency target documented.
- [x] `nyquist_compliant: true` set in frontmatter.

**Approval:** pending

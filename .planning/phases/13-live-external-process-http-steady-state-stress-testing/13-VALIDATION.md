---
phase: 13
slug: live-external-process-http-steady-state-stress-testing
status: draft
nyquist_compliant: true
wave_0_complete: true
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
| **Full suite command** | `cargo test --workspace && cargo run -p app -- http-stress --profile smoke` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run the task-specific automated command listed below; prefer the fastest task-local loop before the longer live-run or bench gates.
- **After every plan wave:** Run `cargo test --workspace && cargo run -p app -- http-stress --profile smoke`.
- **Before `/gsd-verify-work`:** Full workspace tests and one documented live HTTP stress command must be green.
- **Max feedback latency:** 180 seconds for targeted checks; full suite may exceed this when Docker/Testcontainers startup is required.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 13-01-01 | 01 | 1 | TEST-04, OBS-02 | T-13-01 | Profile inputs are bounded and validated before any child process or container starts | unit | `cargo test -p app http_stress_profile_presets_cover_phase13_profiles -- --nocapture && cargo test -p app http_stress_config_validation_rejects_unbounded_inputs -- --nocapture` | ✅ | ⬜ pending |
| 13-01-02 | 01 | 1 | TEST-04, OBS-02 | T-13-02, T-13-03 | One spawned localhost service survives warmup and measurement, uses `drain_with_timeout` semantics at the deadline, and emits run/report metadata without secret leakage | integration | `cargo test -p app external_process_http_stress_smoke -- --nocapture && cargo test -p app stress_report_omits_sensitive_environment_fields -- --nocapture` | ✅ | ⬜ pending |
| 13-02-01 | 02 | 2 | TEST-04, OBS-02 | T-13-05, T-13-06 | CLI exposes only bounded local stress controls and emits JSON with run duration, concurrency, and report metadata | CLI/integration | `cargo run -p app -- http-stress --profile smoke --warmup-seconds 1 --measure-seconds 2 --concurrency 2 --command-count 16 --shard-count 2 --ingress-capacity 8 --ring-size 16` | ✅ | ⬜ pending |
| 13-02-02 | 02 | 2 | TEST-03 | T-13-07, T-13-08 | Criterion stays secondary and reuses the shared smoke profile path instead of forking a new measurement implementation | integration/bench | `cargo test -p app external_process_http_stress_smoke -- --nocapture` | ✅ | ⬜ pending |
| 13-02-03 | 02 | 2 | TEST-03, TEST-04 | T-13-07 | Documentation separates steady-state live HTTP evidence from Phase 12 smoke and Criterion output and names run-duration/deadline metadata explicitly | docs grep | `rg -n "steady-state|smoke|baseline|burst|hot-key|warmup|measurement|run_duration_seconds|concurrency|deadline_policy|drain_timeout_seconds|Criterion" docs/stress-results.md docs/template-guide.md` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- None. Every Phase 13 task has a concrete automated command and an owning file path in Plans 13-01 and 13-02.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Measured interval excludes service boot, migrations, readiness probing, harness compilation, and PostgreSQL container startup | TEST-04 | Requires reading emitted timestamps/logs around the live run boundaries | Run the documented Phase 13 command, confirm the report has distinct `warmup_seconds`, `measurement_seconds`, and `run_duration_seconds` fields, and confirm throughput/latency counters start after warmup completes. |
| Resource and deadline metadata matches the local run environment | OBS-02 | CPU/core count, concurrency, and drain semantics are environment- and profile-dependent | Compare the report's `core_count`, `concurrency`, `deadline_policy`, and `drain_timeout_seconds` fields with the local machine and command-line profile settings. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands.
- [x] Sampling continuity: no 3 consecutive tasks without automated verify.
- [x] Validation graph matches the actual two-plan, five-task phase structure.
- [x] No watch-mode flags.
- [x] Feedback latency target documented.
- [x] `nyquist_compliant: true` set in frontmatter.

**Approval:** pending

---
phase: 11
slug: evidence-recovery-and-runnable-http-service
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-21
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for evidence repair and runnable-service closure.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness, `tokio::test`, plus doc/file verification via `rg` / `test -f` |
| **Quick run command** | `cargo test -p app --no-run && cargo test -p adapter-http -- --nocapture` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | targeted checks under a few minutes; full workspace depends on local Docker/Testcontainers availability |

## Per-Task Verification Map

| Task ID | Plan | Requirement | Automated Command | Status |
|---------|------|-------------|-------------------|--------|
| 11-01-01 | 01 | API-04, DOC-01 | `test -f .planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VERIFICATION.md` | passed |
| 11-01-02 | 01 | API-04, DOC-01 | `rg -n "API-02|API-04|OBS-01|TEST-03|TEST-04|DOC-01" .planning/REQUIREMENTS.md .planning/ROADMAP.md .planning/STATE.md .planning/v1.0-MILESTONE-AUDIT.md` | passed |
| 11-01-03 | 01 | API-04, DOC-01 | `cargo test -p app duplicate_product_lines_emit_distinct_reserve_keys -- --nocapture && cargo test -p app duplicate_product_line_failure_releases_distinct_prior_lines -- --nocapture && cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture && cargo test -p app commerce_process_manager -- --nocapture && cargo test -p es-runtime runtime_duplicate -- --nocapture` | passed |
| 11-02-01 | 02 | API-02, OBS-01 | `cargo test -p app --no-run` | passed |
| 11-02-02 | 02 | API-02, TEST-04 | `cargo test -p adapter-http -- --nocapture` | passed |
| 11-02-03 | 02 | TEST-04 | `cargo test -p app serve_smoke -- --nocapture` | passed |
| 11-02-04 | 02 | DOC-01 | `rg -n "app serve|healthz|stress-smoke|CommandGateway|WebSocket and gRPC gateways" docs/template-guide.md docs/stress-results.md` | passed |

## Wave 0 Requirements

- [x] `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VERIFICATION.md`
- [x] `.planning/phases/11-evidence-recovery-and-runnable-http-service/11-01-PLAN.md`
- [x] `.planning/phases/11-evidence-recovery-and-runnable-http-service/11-02-PLAN.md`
- [x] `crates/app/src/main.rs` / `crates/app/src/serve.rs`
- [x] `crates/app/tests/serve_smoke.rs`

## Validation Audit 2026-04-21

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

### Audit Evidence

- `cargo test -p app duplicate_product_lines_emit_distinct_reserve_keys -- --nocapture` ✅
- `cargo test -p app duplicate_product_line_failure_releases_distinct_prior_lines -- --nocapture` ✅
- `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` ✅
- `cargo test -p app commerce_process_manager -- --nocapture` ✅
- `cargo test -p es-runtime runtime_duplicate -- --nocapture` ✅
- `cargo test -p adapter-http -- --nocapture` ✅
- `cargo test -p app --no-run` ✅
- `cargo test -p app serve_smoke -- --nocapture` ✅

## Validation Sign-Off

- [x] All planned tasks have automated verification.
- [x] Sampling continuity preserved.
- [x] Wave 0 artifacts now exist.
- [x] No watch-mode commands.
- [x] `nyquist_compliant: true` is justified by execution evidence.

**Approval:** complete

---
phase: 08
slug: runtime-duplicate-command-replay
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-19
---

# Phase 08 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Cargo test with Tokio async tests; SQLx/Testcontainers for PostgreSQL integration |
| **Config file** | `Cargo.toml` workspace and crate manifests |
| **Quick run command** | `cargo test -p es-runtime duplicate -- --nocapture` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~180 seconds for targeted duplicate suites; full workspace depends on PostgreSQL container startup |

---

## Sampling Rate

- **After every task commit:** Run the smallest crate command that covers the edited boundary.
- **After every plan wave:** Run targeted duplicate suites across runtime, store, adapter, and app as applicable.
- **Before `$gsd-verify-work`:** Full suite must be green.
- **Max feedback latency:** 300 seconds for targeted duplicate suites.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 0 | RUNTIME-03 | T-08-01 | Tenant-scoped duplicate lookup happens before aggregate rehydration or decision | runtime regression | `cargo test -p es-runtime duplicate -- --nocapture` | ✅ | ⬜ pending |
| 08-01-02 | 01 | 0 | STORE-03 | T-08-02 | Durable dedupe response preserves original committed result and reply payload | PostgreSQL integration | `cargo test -p es-store-postgres --test dedupe duplicate_ -- --test-threads=1 --nocapture` | ✅ | ⬜ pending |
| 08-01-03 | 01 | 1 | RUNTIME-05 | T-08-02 | Duplicate replies are emitted only from prior committed durable results | runtime async | `cargo test -p es-runtime duplicate -- --nocapture` | ✅ | ⬜ pending |
| 08-02-01 | 02 | 1 | API-01 | T-08-03 | HTTP retries use gateway/runtime replay, not adapter-local state | adapter async | `cargo test -p adapter-http duplicate -- --nocapture` | ✅ | ⬜ pending |
| 08-02-02 | 02 | 1 | API-03 | T-08-02 | Duplicate HTTP responses preserve committed append fields and typed reply DTO | adapter async | `cargo test -p adapter-http duplicate -- --nocapture` | ✅ | ⬜ pending |
| 08-03-01 | 03 | 1 | INT-04 | T-08-04 | Process-manager replay with deterministic keys returns duplicate success after prior commit | app/process-manager | `cargo test -p app process_manager_duplicate -- --nocapture` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/es-runtime/tests/runtime_flow.rs` — failing duplicate regression proving same-key retry bypasses `decide`, rehydration, and append on warm cache hit.
- [ ] `crates/es-store-postgres/tests/dedupe.rs` — durable duplicate response payload shape test for append metadata plus typed reply payload or wrapper JSON.
- [ ] `crates/adapter-http/tests/commerce_api.rs` — duplicate HTTP retry response contract test for original durable append fields and typed reply DTO.
- [ ] `crates/app` process-manager test — crash/retry simulation where follow-up commands already committed and offset has not advanced.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Threat References

| Ref | Threat | Mitigation |
|-----|--------|------------|
| T-08-01 | Cross-tenant idempotency collision | Scope cache and durable lookup by `(tenant_id, idempotency_key)` |
| T-08-02 | Replay result substitution | Persist/decode the original committed response payload instead of recomputing from mutated state |
| T-08-03 | Adapter-local retry state bypasses runtime guarantees | Keep HTTP adapter thin and route duplicate behavior through `CommandGateway` |
| T-08-04 | Process-manager crash/retry repeats side effects | Reuse deterministic follow-up idempotency keys with runtime/store duplicate replay |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies.
- [x] Sampling continuity: no 3 consecutive tasks without automated verify.
- [x] Wave 0 covers all MISSING references.
- [x] No watch-mode flags.
- [x] Feedback latency < 300s for targeted duplicate suites.
- [x] `nyquist_compliant: true` set in frontmatter.

**Approval:** approved 2026-04-19

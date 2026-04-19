---
phase: 08
slug: runtime-duplicate-command-replay
status: passed
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-19
---

# Phase 08 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` / integration tests |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test -p es-runtime` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p es-runtime`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Requirement-Level Verification Sampling Map

| Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01 Durable replay substrate | 1 | STORE-03, RUNTIME-05 | T-08-01, T-08-02 | Durable replay storage stores typed command replies and returns original append data for repeated tenant/idempotency keys. | integration | `cargo test -p es-store-postgres command_reply_payload -- --nocapture`<br>`cargo test -p es-store-postgres command_replay -- --nocapture`<br>`cargo test -p es-store-postgres duplicate_idempotency_key_returns_original_result -- --nocapture` | yes | green |
| 08-02 Runtime replay ordering | 2 | STORE-03, RUNTIME-03, RUNTIME-05 | T-08-01, T-08-02, T-08-03 | Runtime codec, shard-local cache replay, durable lookup replay, and duplicate append branch checks return original committed replies before fresh domain decisions. | unit/integration | `cargo test -p es-runtime command_replay_contract -- --nocapture`<br>`cargo test -p app single_service_stress_smoke -- --nocapture`<br>`cargo test -p es-runtime runtime_duplicate -- --nocapture`<br>`cargo test -p es-runtime duplicate_replay_returns_original_reply_after_state_mutation -- --nocapture` | yes | green |
| 08-03 External replay consumers | 3 | STORE-03, RUNTIME-05, INT-04, API-01, API-03 | T-08-01, T-08-02, T-08-03, T-08-04 | HTTP duplicate retry and process-manager duplicate follow-up retry preserve original committed outcomes without adapter or process-manager-local dedupe state. | adapter/app integration | `cargo test -p adapter-http duplicate_place_order_retry_returns_original_response -- --nocapture`<br>`cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` | yes | green |

---

## Wave 0 Requirements

- [x] Existing Rust test infrastructure covers runtime, storage, adapter, and app crates.
- [x] Phase plans must add or update tests for runtime warm replay, durable fallback replay, HTTP duplicate retry, and process-manager follow-up retry.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | N/A | All Phase 8 behaviors should have automated verification. | N/A |

---

## Validation Audit 2026-04-20

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** automated Phase 08 requirement-level sampling coverage present

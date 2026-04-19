---
phase: 08
slug: runtime-duplicate-command-replay
status: draft
nyquist_compliant: false
wave_0_complete: false
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

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | RUNTIME-03, RUNTIME-05 | T-08-01 | Duplicate commands do not execute fresh domain decisions before idempotency replay | unit | `cargo test -p es-runtime duplicate` | W0 | pending |
| 08-02-01 | 02 | 1 | STORE-03 | T-08-02 | Durable dedupe replay remains authoritative after runtime cache miss/restart | integration | `cargo test -p es-store-postgres dedupe` | W0 | pending |
| 08-03-01 | 03 | 2 | API-01, API-03, INT-04 | T-08-03 | HTTP and process-manager retries return original committed outcomes | integration | `cargo test --workspace duplicate` | W0 | pending |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

- [ ] Existing Rust test infrastructure covers runtime, storage, adapter, and app crates.
- [ ] Phase plans must add or update tests for runtime warm replay, durable fallback replay, HTTP duplicate retry, and process-manager follow-up retry.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | N/A | All Phase 8 behaviors should have automated verification. | N/A |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

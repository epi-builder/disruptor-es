---
phase: 10
slug: duplicate-safe-process-manager-follow-up-keys
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-20
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness plus `tokio::test` for async app tests |
| **Config file** | Workspace `Cargo.toml`; no separate app test config |
| **Quick run command** | `cargo test -p app commerce_process_manager -- --nocapture` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~60 seconds for app process-manager tests; workspace runtime depends on PostgreSQL/container availability |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p app commerce_process_manager -- --nocapture`
- **After every plan wave:** Run `cargo test -p app commerce_process_manager -- --nocapture && cargo test -p es-runtime runtime_duplicate -- --nocapture`
- **Before `$gsd-verify-work`:** Full suite must be green, or any Docker/PostgreSQL environment blocker must be called out explicitly with the narrower passing commands
- **Max feedback latency:** 60 seconds for app process-manager sampling

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 10-01-01 | 01 | 1 | DOM-04, INT-04 | T-10-01 | Duplicate same-product order lines emit distinct reserve idempotency keys | app unit | `cargo test -p app duplicate_product_lines_emit_distinct_reserve_keys -- --nocapture` | No W0 | pending |
| 10-01-02 | 01 | 1 | DOM-05, INT-04 | T-10-02 | Failed duplicate-line reservation releases prior successful duplicate-line reservations with distinct keys | app unit | `cargo test -p app duplicate_product_line_failure_releases_distinct_prior_lines -- --nocapture` | No W0 | pending |
| 10-01-03 | 01 | 1 | STORE-03, RUNTIME-05 | T-10-03 | Reprocessing the same process-manager event replays committed follow-up outcomes instead of appending new commands | integration-style app unit with real `CommandEngine` and replay-aware store | `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` | Yes | pending |
| 10-01-04 | 01 | 1 | STORE-03, INT-04 | T-10-01 | Replay-aware multi-line coverage records one replay record per distinct line-aware idempotency key | app unit/integration-style app unit | `cargo test -p app commerce_process_manager -- --nocapture` | Partial W0 | pending |

---

## Wave 0 Requirements

- [ ] `crates/app/src/commerce_process_manager.rs` — add `duplicate_product_lines_emit_distinct_reserve_keys`.
- [ ] `crates/app/src/commerce_process_manager.rs` — add `duplicate_product_line_failure_releases_distinct_prior_lines`.
- [ ] `crates/app/src/commerce_process_manager.rs` — extend replay-aware store/key assertions if needed so duplicate line replay coverage can prove one replay record per line-aware idempotency key.

---

## Manual-Only Verifications

All phase behaviors should have automated verification.

---

## Threat References

| Threat | Description | Mitigation |
|--------|-------------|------------|
| T-10-01 | Duplicate same-product order lines replay the wrong prior reserve result | Include stable line ordinal in reserve idempotency keys and assert duplicate product lines emit distinct keys |
| T-10-02 | Compensation release uses product-only identity and replays or releases the wrong prior line | Carry the original line ordinal for every successful reserve and use it in release idempotency keys |
| T-10-03 | Process-manager retry bypasses runtime/store replay or appends fresh follow-up commands | Keep follow-up commands routed through `CommandGateway` and verify replay through existing `CommandEngine` tests |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter after validation evidence is complete

**Approval:** pending

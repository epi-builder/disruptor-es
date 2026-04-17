---
phase: 04
slug: commerce-fixture-domain
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-17
---

# Phase 04 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` with existing `proptest` dev dependency |
| **Config file** | `Cargo.toml`, `crates/example-commerce/Cargo.toml` |
| **Quick run command** | `cargo test -p example-commerce` |
| **Full suite command** | `cargo test -p example-commerce && cargo test --workspace` |
| **Estimated runtime** | ~30-120 seconds depending on PostgreSQL integration test cache/container state |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p example-commerce`
- **After every plan wave:** Run `cargo test -p example-commerce && cargo test --workspace`
- **Before `$gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | DOM-01, DOM-02 | T-04-01 / T-04-03 | User lifecycle rejects invalid transitions through typed errors | unit/property | `cargo test -p example-commerce user` | W0 | pending |
| 04-02-01 | 02 | 1 | DOM-01, DOM-03 | T-04-01 / T-04-02 | Product inventory never goes negative and reservation/release errors are explicit | unit/property | `cargo test -p example-commerce product` | W0 | pending |
| 04-03-01 | 03 | 2 | DOM-01, DOM-04, DOM-05 | T-04-01 / T-04-04 | Order lifecycle rejects inactive users, unavailable products, duplicate placement, and invalid terminal transitions | unit/property | `cargo test -p example-commerce order` | W0 | pending |
| 04-04-01 | 04 | 2 | TEST-01, DOM-05 | T-04-01 / T-04-02 / T-04-04 | Generated command sequences exercise `decide` plus `apply`, not event replay alone | property/integration | `cargo test -p example-commerce && cargo test --workspace` | W0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/example-commerce/src/user.rs` - user aggregate tests for registration and activation lifecycle
- [ ] `crates/example-commerce/src/product.rs` - product aggregate tests for inventory adjustment, reservation, release, and negative inventory rejection
- [ ] `crates/example-commerce/src/order.rs` - order aggregate tests for placement, confirmation, rejection, cancellation, and relationship-assumption rejection
- [ ] `crates/example-commerce/src/tests.rs` or module-local test helpers - deterministic metadata and proptest strategy helpers
- [ ] `crates/example-commerce/tests/dependency_boundaries.rs` remains green and continues to reject runtime/storage/adapter dependencies in the domain fixture

---

## Manual-Only Verifications

All Phase 4 behaviors have automated verification. No manual-only checks are expected.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

---
phase: 04
slug: commerce-fixture-domain
status: draft
nyquist_compliant: true
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
| 04-01 | 04-01 foundation | 1 | DOM-01 | T-04-01 / T-04-02 / T-04-04 | Commerce ID and quantity constructors reject invalid values, and the domain facade does not add runtime/storage/adapter dependencies | unit | `cargo test -p example-commerce` | W0 | pending |
| 04-02 | 04-02 user | 2 | DOM-01, DOM-02, DOM-05 | T-04-05 / T-04-06 / T-04-07 | User lifecycle rejects invalid transitions through typed errors and replayable user events | unit/replay | `cargo test -p example-commerce user` | W0 | pending |
| 04-03 | 04-03 product | 2 | DOM-01, DOM-03, DOM-05 | T-04-09 / T-04-10 / T-04-11 / T-04-12 | Product inventory never goes negative and reservation/release errors are explicit typed results | unit/property | `cargo test -p example-commerce product` | W0 | pending |
| 04-04 | 04-04 order/tests | 3 | DOM-01, DOM-04, DOM-05, TEST-01 | T-04-14 / T-04-15 / T-04-16 / T-04-17 / T-04-18 | Order lifecycle rejects inactive users, unavailable products, duplicate placement, invalid terminal transitions, and generated command sequences exercise `decide` plus `apply` | property/integration | `cargo test -p example-commerce && cargo test --workspace` | W0 | pending |

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

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

---
phase: 05
slug: cqrs-projection-and-query-catch-up
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-18
---

# Phase 05 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness with Tokio async tests and Testcontainers PostgreSQL |
| **Config file** | none; workspace `Cargo.toml` centralizes dependencies and lints |
| **Quick run command** | `cargo test -p es-projection -p es-store-postgres projections -- --nocapture` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~90 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p es-projection` or the most specific targeted package test named in the plan.
- **After every plan wave:** Run `cargo test --workspace`.
- **Before `$gsd-verify-work`:** Full suite must be green.
- **Max feedback latency:** 120 seconds for targeted checks; full workspace run is acceptable at wave boundaries.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | PROJ-04 | T-05-01 / T-05-02 | Query wait policy rejects invalid positions and bounds waits | unit | `cargo test -p es-projection minimum_position -- --nocapture` | No - Wave 0 | pending |
| 05-02-01 | 02 | 1 | PROJ-01 | T-05-01 / T-05-03 | Tenant-scoped offsets and read models commit atomically | integration | `cargo test -p es-store-postgres projections_offset_commits_with_read_models -- --nocapture` | No - Wave 0 | pending |
| 05-03-01 | 03 | 2 | PROJ-02 | T-05-01 / T-05-04 | Read models derive only from committed events | integration | `cargo test -p es-store-postgres projections_build_commerce_read_models -- --nocapture` | No - Wave 0 | pending |
| 05-04-01 | 04 | 2 | PROJ-03 | T-05-03 / T-05-04 | Restart resumes from checkpoint without duplicate effects | integration | `cargo test -p es-store-postgres projections_resume_without_duplicate_effects -- --nocapture` | No - Wave 0 | pending |
| 05-04-02 | 04 | 2 | PROJ-04 | T-05-02 | Minimum-position query returns lag timeout instead of blocking indefinitely | unit + integration | `cargo test -p es-projection minimum_position -- --nocapture` | No - Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/es-projection/tests/minimum_position.rs` - covers PROJ-04 query wait policy.
- [ ] `crates/es-store-postgres/tests/projections.rs` - covers PROJ-01, PROJ-02, and PROJ-03 against PostgreSQL.
- [ ] `crates/es-store-postgres/src/projection.rs` - PostgreSQL repository for projector offsets and read models.
- [ ] A new SQLx migration adding `projector_offsets`, `order_summary_read_models`, and `product_inventory_read_models`.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Sign-Off

- [x] All tasks have automated verify commands or Wave 0 dependencies.
- [x] Sampling continuity: no 3 consecutive tasks without automated verify.
- [x] Wave 0 covers all missing references.
- [x] No watch-mode flags.
- [x] Feedback latency target is documented.
- [x] `nyquist_compliant: true` set in frontmatter.

**Approval:** approved 2026-04-18

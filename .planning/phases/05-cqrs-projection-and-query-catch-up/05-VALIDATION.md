---
phase: 05
slug: cqrs-projection-and-query-catch-up
status: verified
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-18
verified: 2026-04-18T05:34:35Z
---

# Phase 05 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness with Tokio async tests and Testcontainers PostgreSQL |
| **Config file** | none; workspace `Cargo.toml` centralizes dependencies and lints |
| **Quick run command** | `cargo test -p es-projection minimum_position -- --nocapture` and `cargo test -p es-store-postgres --test projections -- --test-threads=1 --nocapture` |
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
| 05-01-01 | 01 | 1 | PROJ-04 | T-05-01 / T-05-02 | Query wait policy rejects invalid positions and bounds waits | unit | `cargo test -p es-projection minimum_position -- --nocapture` | Yes - `crates/es-projection/tests/minimum_position.rs` | green |
| 05-02-01 | 02 | 1 | PROJ-02 | T-05-06 / T-05-08 / T-05-09 | Commerce event payloads round-trip through typed serde DTOs without domain dependency drift | unit | `cargo test -p example-commerce projection_payload -- --nocapture` | Yes - `crates/example-commerce/src/order.rs`, `crates/example-commerce/src/product.rs` | green |
| 05-03-01 | 03 | 2 | PROJ-01 | T-05-01 / T-05-03 / T-05-12 | Tenant-scoped offsets and read models commit atomically | integration | `cargo test -p es-store-postgres --test projections projections_offset_commits_with_read_models -- --exact --test-threads=1 --nocapture` | Yes - `crates/es-store-postgres/tests/projections.rs` | green |
| 05-03-02 | 03 | 2 | PROJ-02 | T-05-06 / T-05-10 / T-05-11 | Read models derive only from committed events | integration | `cargo test -p es-store-postgres --test projections projections_build_commerce_read_models -- --exact --test-threads=1 --nocapture` | Yes - `crates/es-store-postgres/tests/projections.rs` | green |
| 05-03-03 | 03 | 2 | PROJ-03 | T-05-03 / T-05-04 / T-05-12 | Restart resumes from checkpoint without duplicate effects | integration | `cargo test -p es-store-postgres --test projections projections_resume_without_duplicate_effects -- --exact --test-threads=1 --nocapture` | Yes - `crates/es-store-postgres/tests/projections.rs` | green |
| 05-03-04 | 03 | 2 | PROJ-04 | T-05-02 / T-05-14 | Minimum-position query returns lag timeout instead of blocking indefinitely | unit + integration | `cargo test -p es-store-postgres --test projections projections_queries_wait_for_minimum_position -- --exact --test-threads=1 --nocapture` | Yes - `crates/es-store-postgres/tests/projections.rs` | green |
| 05-03-05 | 03 | 2 | PROJ-01 / PROJ-03 | T-05-12 | Stale concurrent catch-up cannot move projector offset backward | integration regression | `cargo test -p es-store-postgres --test projections projector_offset_does_not_move_backward_on_stale_upsert -- --exact --test-threads=1 --nocapture` | Yes - `crates/es-store-postgres/tests/projections.rs` | green |
| 05-03-06 | 03 | 2 | PROJ-01 / PROJ-02 | T-05-10 / T-05-13 | Tenant predicates isolate read models and malformed handled payloads do not advance offsets | integration | `cargo test -p es-store-postgres --test projections -- --test-threads=1 --nocapture` | Yes - `crates/es-store-postgres/tests/projections.rs` | green |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [x] `crates/es-projection/tests/minimum_position.rs` - covers PROJ-04 query wait policy.
- [x] `crates/example-commerce/src/order.rs` and `crates/example-commerce/src/product.rs` - cover typed commerce projection payload serde round trips.
- [x] `crates/es-store-postgres/tests/projections.rs` - covers PROJ-01, PROJ-02, PROJ-03, and PROJ-04 against PostgreSQL.
- [x] `crates/es-store-postgres/src/projection.rs` - PostgreSQL repository for projector offsets and read models.
- [x] `crates/es-store-postgres/migrations/20260418000000_projection_read_models.sql` - adds `projector_offsets`, `order_summary_read_models`, and `product_inventory_read_models`.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Validation Audit 2026-04-18

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |

### Commands Run

| Command | Result |
|---------|--------|
| `cargo test -p es-projection minimum_position -- --nocapture` | PASS - 8 passed |
| `cargo test -p example-commerce projection_payload -- --nocapture` | PASS - 4 passed |
| `cargo test -p es-store-postgres --test projections -- --test-threads=1 --nocapture` | PASS - 7 passed |

### Requirement Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| PROJ-01 | COVERED | PostgreSQL projection tests verify atomic read-model and offset commits, malformed payload rollback, and monotonic offset behavior. |
| PROJ-02 | COVERED | Commerce payload round-trip tests and PostgreSQL projection tests verify order summary and product inventory rows derive from committed events. |
| PROJ-03 | COVERED | PostgreSQL projection tests verify restart resume and duplicate-effect prevention from saved checkpoints. |
| PROJ-04 | COVERED | Projection contract tests and PostgreSQL query tests verify bounded minimum-position waits and typed lag errors. |

---

## Validation Sign-Off

- [x] All tasks have automated verify commands or Wave 0 dependencies.
- [x] Sampling continuity: no 3 consecutive tasks without automated verify.
- [x] Wave 0 covers all missing references.
- [x] No watch-mode flags.
- [x] Feedback latency target is documented.
- [x] `nyquist_compliant: true` set in frontmatter.

**Approval:** approved 2026-04-18

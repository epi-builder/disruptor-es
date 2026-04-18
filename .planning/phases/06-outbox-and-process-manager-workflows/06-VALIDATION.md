---
phase: 06
slug: outbox-and-process-manager-workflows
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-18
---

# Phase 06 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness with Tokio async tests and Testcontainers PostgreSQL |
| **Config file** | none; workspace `Cargo.toml` centralizes dependencies and lints |
| **Quick run command** | `cargo test -p es-outbox && cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run the focused package test for the touched crate.
- **After PostgreSQL outbox changes:** Run `cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture`.
- **After every plan wave:** Run `cargo test -p es-outbox && cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture`.
- **Before `$gsd-verify-work`:** Full suite must be green.
- **Max feedback latency:** 180 seconds for targeted checks; full workspace run is acceptable at wave boundaries.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | INT-01 / INT-02 / INT-03 | T-06-01 / T-06-02 / T-06-03 | Outbox contracts validate topics, source event references, batch limits, worker IDs, statuses, retry settings, and publisher idempotency keys before storage or publisher calls. | unit | `cargo test -p es-outbox -- --nocapture` | No - Wave 0 | pending |
| 06-02-01 | 02 | 1 | INT-01 / INT-03 | T-06-01 / T-06-02 / T-06-04 | PostgreSQL schema enforces tenant scoping, unique `(tenant_id, source_event_id, topic)`, valid statuses, positive attempts, and due-time fields. | integration | `cargo test -p es-store-postgres --test outbox outbox_is_idempotent_by_source_event_and_topic -- --test-threads=1 --nocapture` | No - Wave 0 | pending |
| 06-03-01 | 03 | 2 | INT-01 | T-06-01 / T-06-04 / T-06-05 | Append transaction creates event rows, command dedupe, and derived outbox rows atomically; conflicts, rollback, and duplicate idempotency replay do not create duplicate outbox rows. | integration | `cargo test -p es-store-postgres --test outbox append_creates_outbox_rows_atomically -- --test-threads=1 --nocapture` | No - Wave 0 | pending |
| 06-04-01 | 04 | 2 | INT-02 / INT-03 | T-06-02 / T-06-03 / T-06-06 | Dispatcher claims pending rows with bounded batches, publishes through `Publisher`, marks success, schedules retry on failure, and preserves deterministic idempotency keys. | unit + integration | `cargo test -p es-outbox dispatcher -- --nocapture && cargo test -p es-store-postgres --test outbox dispatcher_marks_successful_rows_published -- --test-threads=1 --nocapture` | No - Wave 0 | pending |
| 06-05-01 | 05 | 3 | INT-04 | T-06-03 / T-06-07 / T-06-08 | Process manager reads committed order/product events, submits follow-up commands through command gateways, and advances durable offsets only after replies finish or events are intentionally skipped. | unit + integration | `cargo test -p es-outbox process_manager -- --nocapture` | No - Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/es-outbox/src/error.rs` - typed outbox, dispatcher, publisher, and process-manager errors.
- [ ] `crates/es-outbox/src/models.rs` - validated outbox message, topic, status, source event reference, batch limit, worker ID, retry policy, and dispatch outcome types.
- [ ] `crates/es-outbox/src/publisher.rs` - `Publisher` trait and in-memory idempotent test publisher.
- [ ] `crates/es-outbox/src/dispatcher.rs` - storage-neutral dispatch orchestration.
- [ ] `crates/es-outbox/src/process_manager.rs` - process-manager contracts and commerce workflow test support.
- [ ] `crates/es-store-postgres/migrations/*_outbox.sql` - outbox and process-manager offset tables.
- [ ] `crates/es-store-postgres/src/outbox.rs` - PostgreSQL claim, mark-published, retry, failed, and process-manager offset repository.
- [ ] `crates/es-store-postgres/tests/outbox.rs` - container-backed outbox atomicity, claim, retry, tenant isolation, and idempotency tests.

---

## Manual-Only Verifications

All Phase 06 behaviors should have automated verification. Broker-specific manual verification is out of scope because Phase 06 defines a publisher trait and fake publisher, not a real NATS/Kafka adapter.

---

## Validation Sign-Off

- [ ] All tasks have automated verify commands or Wave 0 dependencies.
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify.
- [ ] Wave 0 covers all missing references.
- [ ] No watch-mode flags.
- [ ] Feedback latency target is documented.
- [ ] `nyquist_compliant: true` set in frontmatter after implementation validation passes.

**Approval:** pending

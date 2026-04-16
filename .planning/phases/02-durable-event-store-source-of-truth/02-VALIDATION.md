---
phase: 02
slug: durable-event-store-source-of-truth
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-17
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` with `sqlx` migrations and PostgreSQL integration tests |
| **Config file** | `Cargo.toml`, `crates/es-store-postgres/Cargo.toml`, `crates/es-store-postgres/migrations/` |
| **Quick run command** | `cargo test -p es-store-postgres --lib` |
| **Full suite command** | `cargo test --workspace` plus PostgreSQL-backed integration tests when database/container runtime is available |
| **Estimated runtime** | ~30-180 seconds depending on PostgreSQL container startup |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p es-store-postgres --lib` when storage library code exists.
- **After every plan wave:** Run `cargo test --workspace`.
- **Before `$gsd-verify-work`:** Full workspace tests and PostgreSQL-backed storage integration tests must be green or explicitly blocked by unavailable local container/database runtime.
- **Max feedback latency:** 180 seconds for local code checks; PostgreSQL container startup may exceed this on first pull.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | STORE-01, STORE-02, STORE-03, STORE-04, STORE-05 | T-02-01 / T-02-02 | Schema constraints enforce stream revision, event identity, tenant-scoped dedupe, snapshot ordering, and global-position reads | migration/schema | `cargo test -p es-store-postgres --lib` | W0 | pending |
| 02-02-01 | 02 | 1 | STORE-01, STORE-02, STORE-05 | T-02-01 / T-02-03 | Append API exposes typed committed results and rejects invalid/empty appends before persistence | unit | `cargo test -p es-store-postgres --lib` | W0 | pending |
| 02-03-01 | 03 | 2 | STORE-01, STORE-03 | T-02-01 / T-02-02 | PostgreSQL transaction provides OCC conflict behavior and idempotency replay without duplicate events | integration | `cargo test -p es-store-postgres --test append_occ --test dedupe` | W0 | pending |
| 02-04-01 | 04 | 2 | STORE-04, STORE-05 | T-02-04 | Snapshot plus stream-event reads and global-position reads use committed event-store state only | integration | `cargo test -p es-store-postgres --test snapshots --test global_reads` | W0 | pending |

*Status: pending, green, red, flaky*

---

## Wave 0 Requirements

- [ ] `crates/es-store-postgres/tests/support/mod.rs` — PostgreSQL test pool/container helpers and migration setup.
- [ ] `crates/es-store-postgres/tests/append_occ.rs` — append success and optimistic concurrency coverage.
- [ ] `crates/es-store-postgres/tests/dedupe.rs` — tenant/idempotency replay coverage.
- [ ] `crates/es-store-postgres/tests/snapshots.rs` — latest snapshot plus subsequent event reads.
- [ ] `crates/es-store-postgres/tests/global_reads.rs` — global-position catch-up reads.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| PostgreSQL container/runtime availability | TEST-02 support for Phase 2 storage tests | Local Docker or external PostgreSQL may be unavailable in some agent environments | If integration tests cannot start PostgreSQL, record the exact runtime error and run all non-DB checks; do not mark STORE integration behavior verified until a real PostgreSQL run passes. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies.
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify.
- [ ] Wave 0 covers all MISSING references.
- [ ] No watch-mode flags.
- [ ] Feedback latency < 180s for local checks.
- [ ] `nyquist_compliant: true` set in frontmatter after plan verification confirms coverage.

**Approval:** pending

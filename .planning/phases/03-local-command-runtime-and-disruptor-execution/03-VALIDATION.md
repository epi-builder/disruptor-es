---
phase: 03
slug: local-command-runtime-and-disruptor-execution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-17
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` for unit/integration coverage, with fake-store tests in `es-runtime` and existing PostgreSQL-backed storage checks in `es-store-postgres` where runtime/storage integration needs confirmation |
| **Config file** | `Cargo.toml`, `crates/es-runtime/Cargo.toml`, `crates/es-store-postgres/Cargo.toml` |
| **Quick run command** | `cargo test -p es-runtime` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~20-120 seconds depending on workspace growth and whether storage-backed integration coverage is included |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p es-runtime`
- **After every plan wave:** Run `cargo test -p es-runtime && cargo test -p es-store-postgres`
- **Before `$gsd-verify-work`:** `cargo test --workspace` plus targeted grep checks for forbidden global state and ring/durability misuse must be green
- **Max feedback latency:** 120 seconds for local runtime checks; allow up to 180 seconds when runtime/storage integration expands

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | RUNTIME-01, RUNTIME-02 | T-03-01 / T-03-02 | Bounded ingress rejects full capacity explicitly and routes tenant-scoped keys deterministically | unit | `cargo test -p es-runtime gateway router` | W0 | pending |
| 03-02-01 | 02 | 1 | RUNTIME-03, RUNTIME-04 | T-03-02 / T-03-03 | Shard-local aggregate and dedupe caches stay single-owner; accepted routed commands pass through `DisruptorPath::try_publish` before shard handoff; disruptor publication returns typed overload instead of spinning through hidden backpressure | unit | `cargo test -p es-runtime shard_cache && cargo test -p es-runtime disruptor_path && cargo test -p es-runtime shard_handle` | W0 | pending |
| 03-03-01 | 03 | 2 | RUNTIME-05, RUNTIME-06 | T-03-04 / T-03-05 | Cache misses rehydrate from durable storage before decide; replies are emitted only after durable append; OCC conflicts never mutate shard cache with newly decided events | unit/integration | `cargo test -p es-runtime cache_miss_rehydrates_before_decide && cargo test -p es-runtime reply_after_commit && cargo test -p es-runtime conflict_does_not_mutate_cache` | W0 | pending |
| 03-04-01 | 04 | 2 | RUNTIME-01, RUNTIME-05, RUNTIME-06 | T-03-01 / T-03-04 / T-03-05 | End-to-end runtime path maps overload/conflict/store failures to typed outcomes while preserving durable source-of-truth semantics and committed cache advancement when reply receivers are dropped | integration | `cargo test -p es-runtime runtime_flow -- --nocapture` and `cargo test --workspace` | W0 | pending |

*Status: pending, green, red, flaky*

---

## Wave 0 Requirements

- [ ] `crates/es-runtime/src/error.rs` — typed runtime errors for overload, unavailable, conflict, and store failures
- [ ] `crates/es-runtime/src/router.rs` — stable tenant-aware routing plus golden tests
- [ ] `crates/es-runtime/src/gateway.rs` — bounded ingress and reply channel behavior tests
- [ ] `crates/es-runtime/src/shard.rs` — shard-local aggregate cache ownership, dedupe cache ownership, disruptor-backed shard handle, rehydration-before-decide, and command processor tests
- [ ] `crates/es-runtime/src/disruptor_path.rs` — `try_publish` wrapper and full-ring behavior tests
- [ ] `crates/es-runtime/tests/` or equivalent module tests — reply-after-commit, conflict-without-cache-mutation, and runtime flow coverage

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Ring wait strategy and async storage bridge are operationally sane under the first implementation spike | RUNTIME-04, RUNTIME-05 | Initial compile/runtime ergonomics may expose a better local bridge shape than research predicted | Record whether the chosen disruptor processor path can invoke or delegate async append without hidden blocking; if not, document the observed constraint before revising plans |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies.
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify.
- [ ] Wave 0 covers all MISSING references.
- [ ] No watch-mode flags.
- [ ] Feedback latency < 180s for local checks.
- [ ] `nyquist_compliant: true` set in frontmatter after plan verification confirms coverage.

**Approval:** pending

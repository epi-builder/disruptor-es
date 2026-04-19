---
phase: 09
slug: tenant-scoped-runtime-aggregate-cache
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-20
---

# Phase 09 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Cargo test with Rust `#[test]` and `#[tokio::test]` |
| **Config file** | Root `Cargo.toml` |
| **Quick run command** | `cargo test -p es-runtime shard_cache -- --nocapture` |
| **Full suite command** | `cargo test -p es-runtime -- --nocapture` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p es-runtime shard_cache -- --nocapture`
- **After every plan wave:** Run `cargo test -p es-runtime -- --nocapture`
- **Before `$gsd-verify-work`:** Full runtime suite must be green
- **Max feedback latency:** 30 seconds for quick cache sampling

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | RUNTIME-03 | T-09-01 | Aggregate cache keys include tenant identity and stream identity | unit | `cargo test -p es-runtime shard_cache -- --nocapture` | Yes | passed |
| 09-01-02 | 01 | 1 | STORE-04 | T-09-02 | Same-stream different-tenant cache miss calls tenant-scoped rehydration | runtime regression | `cargo test -p es-runtime same_stream_different_tenant_rehydrates_independently -- --nocapture` | Yes | passed |
| 09-01-03 | 01 | 1 | DOM-05 | T-09-01 | Tenant B command is not decided against tenant A cached state | runtime regression | `cargo test -p es-runtime same_stream_different_tenant_preserves_domain_state -- --nocapture` | Yes | passed |
| 09-01-04 | 01 | 1 | RUNTIME-05 | T-09-03 | Duplicate replay still happens before aggregate cache lookup or rehydration | regression | `cargo test -p es-runtime runtime_duplicate -- --nocapture` | Yes | passed |
| 09-01-05 | 01 | 1 | RUNTIME-06 | T-09-01 | Optimistic conflict does not mutate another tenant's cached state | regression | `cargo test -p es-runtime conflict_does_not_mutate_cache -- --nocapture` | Yes | passed |

---

## Wave 0 Requirements

- [x] `crates/es-runtime/tests/shard_disruptor.rs` - update cache unit tests to use tenant plus stream keys.
- [x] `crates/es-runtime/tests/runtime_flow.rs` - add same-stream/different-tenant regression tests.
- [x] `crates/es-runtime/tests/runtime_flow.rs` - extend `FakeStore` support to record `load_rehydration` calls by `(TenantId, StreamId)` and return tenant-specific batches.

---

## Manual-Only Verifications

All phase behaviors have automated verification.

---

## Threat References

| Threat | Description | Mitigation |
|--------|-------------|------------|
| T-09-01 | Cross-tenant state bleed through stream-only in-memory aggregate cache | Use `AggregateCacheKey { tenant_id, stream_id }` for every aggregate cache hit, fill, and commit |
| T-09-02 | Tenant-scoped storage bypass due to cache hit | Make `AggregateCache::get` impossible to call with only `StreamId` |
| T-09-03 | Duplicate replay order regresses during cache refactor | Preserve Phase 8 order: shard-local dedupe, durable replay, then aggregate cache or rehydration |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all missing references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter after validation evidence is complete

**Approval:** passed

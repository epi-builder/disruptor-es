---
phase: 09-tenant-scoped-runtime-aggregate-cache
verified: 2026-04-19T21:29:07Z
status: passed
score: 6/6 must-haves verified
overrides_applied: 0
---

# Phase 9: Tenant-Scoped Runtime Aggregate Cache Verification Report

**Phase Goal:** Shard-owned runtime aggregate state remains isolated by tenant as well as stream, so cache hits cannot bypass tenant-scoped rehydration or evaluate commands against another tenant's state.
**Verified:** 2026-04-19T21:29:07Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Aggregate cache entries are keyed by tenant identity plus stream ID while remaining shard-owned and free of global mutable business-state locks. | VERIFIED | `AggregateCache` stores `HashMap<AggregateCacheKey, A::State>` and `AggregateCacheKey` contains `tenant_id` and `stream_id` in `crates/es-runtime/src/cache.rs:7`. Production scan found no `Arc`, `Mutex`, `RwLock`, `parking_lot`, `static mut`, or stream-only aggregate cache API in `cache.rs`, `shard.rs`, or `lib.rs`. |
| 2 | Runtime command processing cannot use tenant A cached aggregate state for tenant B on the same stream ID. | VERIFIED | `ShardState::process_next_handoff` constructs one `AggregateCacheKey` from `envelope.metadata.tenant_id` and `envelope.stream_id` at `crates/es-runtime/src/shard.rs:218`; `same_stream_different_tenant_preserves_domain_state` proves tenant A returns `8` and tenant B returns `42`, not tenant A-derived state, in `crates/es-runtime/tests/runtime_flow.rs:781`. |
| 3 | A stream-only aggregate cache hit cannot bypass tenant-scoped `load_rehydration`. | VERIFIED | Cache APIs accept `AggregateCacheKey`, not `StreamId`, in `crates/es-runtime/src/cache.rs:26` and `crates/es-runtime/src/cache.rs:41`. On cache miss, shard passes tenant and stream into `rehydrate_state` at `crates/es-runtime/src/shard.rs:226`; `rehydrate_state` calls `load_rehydration(tenant_id, stream_id)` at `crates/es-runtime/src/shard.rs:442`. |
| 4 | Phase 8 duplicate replay order remains shard-local dedupe, durable replay, aggregate cache or rehydration, decide, append, cache commit, reply. | VERIFIED | Shard-local dedupe check returns before durable lookup at `crates/es-runtime/src/shard.rs:174`; durable replay lookup returns before cache key construction at `crates/es-runtime/src/shard.rs:187`; cache/rehydration starts only after `cache_key` creation at `crates/es-runtime/src/shard.rs:218`; committed append updates cache and dedupe before reply at `crates/es-runtime/src/shard.rs:310`. |
| 5 | Store conflicts and duplicate append outcomes do not corrupt shard-local tenant-scoped aggregate cache state, including the WR-01 cross-engine idempotency race. | VERIFIED | Conflict branch does not commit cache and `conflict_does_not_mutate_cache` keeps cached value `10` in `crates/es-runtime/tests/runtime_flow.rs:572`. Duplicate append replay now rehydrates tenant-scoped state and commits refreshed cache, or invalidates on refresh failure, in `crates/es-runtime/src/shard.rs:340`; `duplicate_after_warmed_cache_refreshes_from_durable_rehydration` verifies stale value `10` becomes durable value `25` after duplicate replay in `crates/es-runtime/tests/runtime_flow.rs:988`. |
| 6 | Regression tests prove same-stream, different-tenant commands preserve isolated domain state and conflict behavior. | VERIFIED | `shard_cache_isolates_same_stream_across_tenants` proves cache key separation in `crates/es-runtime/tests/shard_disruptor.rs:153`; `same_stream_different_tenant_rehydrates_independently` verifies tenant-specific rehydration calls in `crates/es-runtime/tests/runtime_flow.rs:725`; `same_stream_different_tenant_preserves_domain_state` verifies isolated replies in `crates/es-runtime/tests/runtime_flow.rs:781`; `conflict_does_not_mutate_cache` verifies conflict-safe cache behavior in `crates/es-runtime/tests/runtime_flow.rs:572`. |

**Score:** 6/6 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/es-runtime/src/cache.rs` | `AggregateCacheKey` and tenant-scoped `AggregateCache` API | VERIFIED | Exists and substantive. `gsd-tools verify artifacts` passed. Stores `HashMap<AggregateCacheKey, A::State>` and exposes `get`, `get_or_default`, `commit_state`, and `invalidate` by `AggregateCacheKey`. |
| `crates/es-runtime/src/shard.rs` | `process_next_handoff` cache lookup/fill/commit using one `AggregateCacheKey` | VERIFIED | Exists and substantive. Wired through import and use. One key is constructed after duplicate replay misses and reused for cache lookup, rehydration fill, committed replacement, and duplicate-append refresh/invalidate. |
| `crates/es-runtime/tests/shard_disruptor.rs` | Aggregate cache tenant-key unit coverage | VERIFIED | Exists and substantive. Contains `cache_key(...)` helper and `shard_cache_isolates_same_stream_across_tenants`. Targeted cache tests passed. |
| `crates/es-runtime/tests/runtime_flow.rs` | Same-stream different-tenant runtime regression coverage and WR-01 coverage | VERIFIED | Exists and substantive. Contains tenant-specific fake store rehydration support, same-stream tenant tests, duplicate replay ordering tests, conflict tests, and WR-01 duplicate-append cache refresh test. Runtime flow tests passed. |
| `.planning/phases/09-tenant-scoped-runtime-aggregate-cache/09-VALIDATION.md` | Nyquist validation evidence after tests exist | VERIFIED | Exists and contains `nyquist_compliant: true`; validation table marks tenant cache, tenant rehydration, duplicate replay, and conflict checks passed. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/es-runtime/src/shard.rs` | `crates/es-runtime/src/cache.rs` | `AggregateCacheKey` | VERIFIED | Manual check: imported at `crates/es-runtime/src/shard.rs:11`, constructed at `crates/es-runtime/src/shard.rs:218`, used for `cache.get`, `commit_state`, and `invalidate`. The exact plan regex failed because of formatting, but the semantic link is present. |
| `crates/es-runtime/src/shard.rs` | `crates/es-runtime/src/store.rs` | Tenant-scoped `load_rehydration` | VERIFIED | Manual check: `process_next_handoff` passes `&envelope.metadata.tenant_id` and `&envelope.stream_id` into `rehydrate_state`, which calls `store.load_rehydration(tenant_id, stream_id)` at `crates/es-runtime/src/shard.rs:442`. The exact plan regex failed because the call is wrapped in `rehydrate_state`. |
| `crates/es-runtime/tests/runtime_flow.rs` | `crates/es-runtime/src/shard.rs` | Same stream, different tenant test path | VERIFIED | `gsd-tools verify key-links` found `same_stream_different_tenant_rehydrates_independently`; tests exercise `ShardState::process_next_handoff`. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `crates/es-runtime/src/cache.rs` | `states: HashMap<AggregateCacheKey, A::State>` | `ShardState::process_next_handoff` cache fill and committed append branches | Yes | VERIFIED |
| `crates/es-runtime/src/shard.rs` | `current_state` | Tenant-scoped cache hit or `load_rehydration(tenant_id, stream_id)` plus decoded snapshots/events | Yes | VERIFIED |
| `crates/es-runtime/src/shard.rs` | Duplicate append cache refresh state | `lookup_command_replay` success followed by tenant-scoped `rehydrate_state`; cache commit or invalidate | Yes | VERIFIED |
| `crates/es-runtime/tests/runtime_flow.rs` | Tenant-specific fake rehydration batches | `set_tenant_rehydration((TenantId, StreamId), RehydrationBatch)` and recorded `rehydration_calls` | Yes | VERIFIED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Tenant-scoped cache unit behavior | `cargo test -p es-runtime shard_cache -- --nocapture` | 3 cache tests passed; pre-existing missing-docs warning in `shard_disruptor.rs` did not fail tests. | PASS |
| Runtime tenant isolation, duplicate replay, conflict, and WR-01 flow behavior | `cargo test -p es-runtime --test runtime_flow -- --nocapture` | 18 runtime flow tests passed, including same-stream tenant tests, conflict cache test, and `duplicate_after_warmed_cache_refreshes_from_durable_rehydration`. | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| STORE-04 | `09-01-PLAN.md` | Aggregate rehydration can load latest snapshot and replay subsequent stream events. | SATISFIED | Cache miss and duplicate append refresh call tenant-scoped `rehydrate_state`, which decodes snapshot/events after `load_rehydration(tenant_id, stream_id)`. Same-stream tenant tests verify both tenants rehydrate independently. |
| RUNTIME-03 | `09-01-PLAN.md` | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. | SATISFIED | `ShardState` owns `AggregateCache` and `DedupeCache`; production scan found no global lock patterns in phase production files. |
| RUNTIME-05 | `09-01-PLAN.md` | Command replies are sent only after durable append commit succeeds. | SATISFIED | Committed branch updates cache/dedupe and sends reply after `store.append(...).await`; `reply_is_sent_after_append_commit` remains in `runtime_flow.rs` and runtime tests pass. Duplicate replay branches return original durable replay records rather than fresh decision replies. |
| RUNTIME-06 | `09-01-PLAN.md` | Optimistic concurrency conflicts surface as typed errors without corrupting shard-local cache. | SATISFIED | Store error branch sends `RuntimeError::from_store_error(error)` without cache mutation; `conflict_does_not_mutate_cache` verifies cached value remains `10`. |
| DOM-05 | `09-01-PLAN.md` | Domain invariants prevent invalid operations and must not be evaluated against another tenant's state. | SATISFIED | `same_stream_different_tenant_preserves_domain_state` proves tenant B command decides from tenant B rehydrated value `40`, returning `42`, not tenant A's warmed/committed state. |

All requirement IDs declared in the plan frontmatter are accounted for: STORE-04, RUNTIME-03, RUNTIME-05, RUNTIME-06, DOM-05. `.planning/REQUIREMENTS.md` traceability maps each of these IDs to Phase 9, with RUNTIME-05 and DOM-05 also carried into Phase 10.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/es-runtime/src/shard.rs` | 56 | `ShardHandoffToken::placeholder` | INFO | Functional disruptor initialization scaffold. It is not an unimplemented stub and is used by `ShardHandle::new`. |

No production global mutable business-state locks, stream-only aggregate cache API, hardcoded empty production data source, TODO/FIXME implementation marker, or unimplemented runtime path was found in the phase production files.

### Human Verification Required

None. The phase behavior is covered by code inspection and automated Rust tests.

### Gaps Summary

No blocking gaps found. The phase goal is achieved: aggregate hot state is tenant-scoped, tenant-specific rehydration cannot be bypassed by stream-only cache hits, duplicate replay ordering is preserved, and the WR-01 duplicate append replay path refreshes or invalidates tenant-scoped cache state instead of leaving stale state after a cross-engine idempotency race.

---

_Verified: 2026-04-19T21:29:07Z_
_Verifier: Claude (gsd-verifier)_

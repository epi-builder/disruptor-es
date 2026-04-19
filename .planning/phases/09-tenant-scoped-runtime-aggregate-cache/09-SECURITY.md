---
phase: 09-tenant-scoped-runtime-aggregate-cache
secured: 2026-04-20
asvs_level: 1
threats_total: 3
threats_closed: 3
threats_open: 0
block_on: open
status: verified
---

# Phase 09 Security Verification

## Scope

Verified the Phase 09 threat register from `09-01-PLAN.md` against the implemented runtime cache and shard processing code. Implementation files were read-only during this audit.

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| command envelope -> shard-owned hot state | Tenant, stream, and idempotency identity from an accepted command controls which processor-local state may be read before domain decision. | tenant ID, stream ID, idempotency key, aggregate state |
| shard runtime -> durable event store | Runtime may skip storage rehydration only when its local cache key is at least as specific as the tenant-scoped storage query. | tenant-scoped rehydration inputs and cached aggregate state |
| duplicate replay -> aggregate execution | Duplicate commands must return replayed outcomes before aggregate cache lookup, rehydration, decision, append, or cache mutation. | replay records, command replies, dedupe keys |

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-09-01 | I/T | mitigate | CLOSED | `crates/es-runtime/src/cache.rs:7` stores `HashMap<AggregateCacheKey, A::State>`; `crates/es-runtime/src/cache.rs:58` defines `AggregateCacheKey` with `tenant_id` and `stream_id`; `crates/es-runtime/tests/shard_disruptor.rs:152` verifies same-stream tenant isolation. Negative grep found no stream-only aggregate cache API patterns. |
| T-09-02 | E/T | mitigate | CLOSED | `crates/es-runtime/src/shard.rs:218` constructs `AggregateCacheKey` from `envelope.metadata.tenant_id` and `envelope.stream_id` after durable replay misses; `crates/es-runtime/src/shard.rs:223`, `crates/es-runtime/src/shard.rs:235`, and `crates/es-runtime/src/shard.rs:319` use the key for cache hit, rehydration fill, and committed cache replacement. Runtime tenant tests at `crates/es-runtime/tests/runtime_flow.rs:725` and `crates/es-runtime/tests/runtime_flow.rs:782` passed. |
| T-09-03 | T/R | mitigate | CLOSED | `crates/es-runtime/src/shard.rs:175` checks shard-local dedupe before cache lookup; `crates/es-runtime/src/shard.rs:187` performs durable `lookup_command_replay` before `cache_key` construction at `crates/es-runtime/src/shard.rs:218`; `crates/es-runtime/tests/runtime_flow.rs:898` preserves `runtime_duplicate_store_hit_skips_rehydrate_decide_encode_and_append`. Targeted `runtime_duplicate` tests passed. |

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-09-01 | I/T | `AggregateCache` in `crates/es-runtime/src/cache.rs` | mitigate | Tenant-scoped `AggregateCacheKey`, `HashMap<AggregateCacheKey, A::State>`, no stream-only cache APIs, and same-stream tenant isolation coverage. | closed |
| T-09-02 | E/T | `ShardState::process_next_handoff` in `crates/es-runtime/src/shard.rs` | mitigate | One `AggregateCacheKey` is constructed after durable replay misses and reused for cache hit, rehydration fill, and committed cache replacement. | closed |
| T-09-03 | T/R | duplicate replay ordering in `ShardState::process_next_handoff` | mitigate | Shard-local dedupe and durable replay lookup run before aggregate cache lookup; duplicate replay tests remain green. | closed |

## Unregistered Flags

None. `09-01-SUMMARY.md` does not contain a `## Threat Flags` section.

## Accepted Risks Log

No accepted risks.

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-20 | 3 | 3 | 0 | gsd-security-auditor |

## Verification Commands

All targeted commands exited 0:

```bash
cargo test -p es-runtime shard_cache -- --nocapture
cargo test -p es-runtime same_stream_different_tenant_rehydrates_independently -- --nocapture
cargo test -p es-runtime same_stream_different_tenant_preserves_domain_state -- --nocapture
cargo test -p es-runtime runtime_duplicate -- --nocapture
cargo test -p es-runtime conflict_does_not_mutate_cache -- --nocapture
```

Supporting grep checks:

```bash
rg -n "pub struct AggregateCacheKey|HashMap<AggregateCacheKey, A::State>|self\\.cache\\.get\\(&cache_key\\)|commit_state\\(cache_key|lookup_command_replay|runtime_duplicate_store_hit_skips_rehydrate_decide_encode_and_append" crates/es-runtime/src crates/es-runtime/tests
rg -n "cache\\.get\\(&envelope\\.stream_id\\)|commit_state\\(envelope\\.stream_id\\.clone\\(|HashMap<StreamId, A::State>|pub fn get\\(&self, stream_id: &StreamId\\)" crates/es-runtime/src crates/es-runtime/tests
```

The positive grep found the declared mitigation patterns. The negative grep returned no matches.

## Notes

Cargo emitted the pre-existing missing-docs warning for `crates/es-runtime/tests/shard_disruptor.rs`; it did not affect security verification.

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-20

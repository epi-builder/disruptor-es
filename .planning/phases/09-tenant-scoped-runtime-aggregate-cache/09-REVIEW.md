---
phase: 09-tenant-scoped-runtime-aggregate-cache
reviewed: 2026-04-19T21:20:40Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - crates/es-runtime/src/cache.rs
  - crates/es-runtime/src/lib.rs
  - crates/es-runtime/src/shard.rs
  - crates/es-runtime/tests/shard_disruptor.rs
  - crates/es-runtime/tests/runtime_flow.rs
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 09: Code Review Report

**Reviewed:** 2026-04-19T21:20:40Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Reviewed the tenant-scoped runtime aggregate cache changes, shard processing path, public exports, and added runtime/disruptor tests. Tenant scoping is applied consistently for direct cache and dedupe keys, and the scoped tests pass. One correctness issue remains in the duplicate append branch: a cross-engine idempotency race can leave shard-local aggregate state stale after PostgreSQL reports a durable duplicate.

## Warnings

### WR-01: Duplicate append replay leaves aggregate cache stale

**File:** `crates/es-runtime/src/shard.rs:337`
**Issue:** When `store.append()` returns `AppendOutcome::Duplicate`, the runtime decodes the original command reply and records the dedupe cache, but it does not refresh or invalidate the aggregate cache entry built before the append attempt. PostgreSQL can return `Duplicate` for concurrent idempotent appends, including a race where the initial `lookup_command_replay()` returned `None`, this shard rehydrated an older state, and another engine committed the command before this append acquired the dedupe result. In that case, the next command for the same tenant/stream can decide against stale cached state and produce incorrect replies or follow-up events.
**Fix:** Refresh the tenant-scoped aggregate state from the durable store, or invalidate the cache entry, before caching the duplicate replay. A refresh keeps subsequent commands hot and correct:

```rust
Ok(Some(replay)) => {
    let outcome = replay_command_outcome::<A, C>(codec, &replay);
    if outcome.is_ok() {
        let refreshed = rehydrate_state(store, codec, &envelope).await?;
        self.cache.commit_state(cache_key.clone(), refreshed);
        self.dedupe.record(dedupe_key, DedupeRecord { replay });
    }
    let _ = envelope.reply.send(outcome);
}
```

If replay refresh errors should not fail an already committed duplicate reply, add an explicit `AggregateCache::invalidate(&AggregateCacheKey)` method and remove the stale entry instead, forcing the next command to rehydrate.

---

_Reviewed: 2026-04-19T21:20:40Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

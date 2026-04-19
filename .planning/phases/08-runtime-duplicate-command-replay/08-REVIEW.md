---
phase: 08-runtime-duplicate-command-replay
reviewed: 2026-04-19T14:52:32Z
depth: standard
files_reviewed: 20
files_reviewed_list:
  - crates/adapter-http/Cargo.toml
  - crates/adapter-http/tests/commerce_api.rs
  - crates/app/Cargo.toml
  - crates/app/src/commerce_process_manager.rs
  - crates/app/src/stress.rs
  - crates/es-runtime/src/cache.rs
  - crates/es-runtime/src/command.rs
  - crates/es-runtime/src/shard.rs
  - crates/es-runtime/src/store.rs
  - crates/es-runtime/tests/common/mod.rs
  - crates/es-runtime/tests/runtime_flow.rs
  - crates/es-runtime/tests/shard_disruptor.rs
  - crates/es-store-postgres/src/error.rs
  - crates/es-store-postgres/src/event_store.rs
  - crates/es-store-postgres/src/lib.rs
  - crates/es-store-postgres/src/models.rs
  - crates/es-store-postgres/src/sql.rs
  - crates/es-store-postgres/tests/dedupe.rs
  - crates/example-commerce/src/order.rs
  - crates/example-commerce/src/product.rs
findings:
  critical: 1
  warning: 1
  info: 0
  total: 2
status: issues_found
---

# Phase 08: Code Review Report

**Reviewed:** 2026-04-19T14:52:32Z
**Depth:** standard
**Files Reviewed:** 20
**Status:** issues_found

## Summary

Reviewed the Phase 08 runtime duplicate-command replay changes across the runtime, PostgreSQL store, commerce process manager, aggregates, and integration tests. The durable reply replay path is broadly covered, but two correctness issues remain: tenant state can bleed through the runtime aggregate cache, and process-manager idempotency keys collide for repeated product lines.

`Cargo.lock` was provided in the input scope but filtered as a lockfile per the review workflow, so it was not counted as reviewed source.

## Critical Issues

### CR-01: Aggregate Cache Is Not Tenant-Scoped

**File:** `crates/es-runtime/src/cache.rs:8`
**Issue:** `AggregateCache` stores state by `StreamId` only. `ShardState::process_next_handoff` then reads and commits cached state using only `envelope.stream_id` (`crates/es-runtime/src/shard.rs:218`, `crates/es-runtime/src/shard.rs:223`, `crates/es-runtime/src/shard.rs:308`). Because durable storage and command metadata are tenant-scoped, two tenants with the same aggregate stream id can share cached aggregate state when routed to the same shard. Tenant B can see tenant A's state and get false domain errors such as `AlreadyPlaced`, or future commands can be decided from the wrong tenant's history.
**Fix:**
```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AggregateCacheKey {
    pub tenant_id: es_core::TenantId,
    pub stream_id: es_core::StreamId,
}

pub struct AggregateCache<A: Aggregate> {
    states: HashMap<AggregateCacheKey, A::State>,
}
```

Use `AggregateCacheKey { tenant_id: envelope.metadata.tenant_id.clone(), stream_id: envelope.stream_id.clone() }` for every cache `get` and `commit_state` in `ShardState::process_next_handoff`. Add a regression test that processes `tenant-a/order-1` then `tenant-b/order-1` and verifies tenant B rehydrates/decides from its own empty or stored state.

## Warnings

### WR-01: Process-Manager Idempotency Keys Collide for Duplicate Product Lines

**File:** `crates/app/src/commerce_process_manager.rs:68`
**Issue:** The reservation loop builds deterministic idempotency keys from process-manager name, source event id, action, and `product_id` only (`crates/app/src/commerce_process_manager.rs:78`). If an order contains two lines for the same product, the second `ReserveInventory` command reuses the first line's key, so runtime dedupe can replay the first reserve instead of reserving the second quantity. The same collision exists for release keys (`crates/app/src/commerce_process_manager.rs:115`) when compensating previously reserved duplicate-product lines.
**Fix:** Include a stable line ordinal in both reserve and release keys, or coalesce duplicate product lines before submitting follow-up commands. For example:
```rust
for (line_index, line) in lines.into_iter().enumerate() {
    let reserve_key = format!(
        "pm:{}:{}:reserve:{}:{}",
        self.name.as_str(),
        event.event_id,
        line_index,
        line.product_id.as_str(),
    );
    // use the same line_index in reserved_lines and release keys
}
```

Add a process-manager test with two order lines sharing the same `ProductId` and assert two distinct reserve idempotency keys are submitted and replay independently.

---

_Reviewed: 2026-04-19T14:52:32Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

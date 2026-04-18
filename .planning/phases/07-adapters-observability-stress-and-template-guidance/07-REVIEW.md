---
phase: 07-adapters-observability-stress-and-template-guidance
reviewed: 2026-04-18T15:19:55Z
depth: standard
files_reviewed: 37
files_reviewed_list:
  - Cargo.toml
  - benches/adapter_only.rs
  - benches/domain_only.rs
  - benches/projector_outbox.rs
  - benches/ring_only.rs
  - benches/storage_only.rs
  - crates/adapter-http/Cargo.toml
  - crates/adapter-http/src/commerce.rs
  - crates/adapter-http/src/error.rs
  - crates/adapter-http/src/lib.rs
  - crates/adapter-http/tests/commerce_api.rs
  - crates/adapter-http/tests/dependency_boundaries.rs
  - crates/app/Cargo.toml
  - crates/app/migrations/20260417000000_event_store.sql
  - crates/app/migrations/20260418000000_projection_read_models.sql
  - crates/app/migrations/20260418010000_outbox.sql
  - crates/app/src/lib.rs
  - crates/app/src/main.rs
  - crates/app/src/observability.rs
  - crates/app/src/stress.rs
  - crates/es-outbox/Cargo.toml
  - crates/es-outbox/src/dispatcher.rs
  - crates/es-runtime/Cargo.toml
  - crates/es-runtime/src/engine.rs
  - crates/es-runtime/src/gateway.rs
  - crates/es-runtime/src/lib.rs
  - crates/es-runtime/src/shard.rs
  - crates/es-store-postgres/Cargo.toml
  - crates/es-store-postgres/src/event_store.rs
  - crates/es-store-postgres/src/projection.rs
  - crates/es-store-postgres/tests/phase7_integration.rs
  - docs/hot-path-rules.md
  - docs/stress-results.md
  - docs/template-guide.md
  - migrations/20260417000000_event_store.sql
  - migrations/20260418000000_projection_read_models.sql
  - migrations/20260418010000_outbox.sql
findings:
  critical: 0
  warning: 5
  info: 0
  total: 5
status: issues_found
---

# Phase 7: Code Review Report

**Reviewed:** 2026-04-18T15:19:55Z
**Depth:** standard
**Files Reviewed:** 37
**Status:** issues_found

## Summary

Reviewed the Phase 7 adapter, runtime backpressure, observability, stress, projection, outbox, benchmark, migration, and template-guidance changes. The boundary direction is mostly consistent with the template rules, but several correctness and signal-quality issues remain in the runtime and stress/projection paths.

## Warnings

### WR-01: Shard Ring Overload Leaves Accepted Commands Without Replies

**File:** `crates/es-runtime/src/shard.rs:405`

**Issue:** `ShardHandle::accept_routed` publishes to the per-shard disruptor ring with `self.path.try_publish(token.clone())?`. If the ring is full, the `?` returns `RuntimeError::ShardOverloaded` after the command has already been accepted by `CommandGateway`; the `routed.envelope.reply` sender is then dropped without an explicit runtime error. HTTP callers in `submit_command` wait on the oneshot receiver, so shard-level backpressure can surface as a dropped reply or a hung request instead of a clean overload response.

**Fix:** Handle `try_publish` errors inside `accept_routed` or `CommandEngine::process_one` and always complete the reply channel for commands already accepted at ingress. For example, change the API to report a processed overload:

```rust
let sequence = match self.path.try_publish(token.clone()) {
    Ok(sequence) => sequence,
    Err(error) => {
        let _ = routed.envelope.reply.send(Err(error));
        return Ok(0); // or return a Processed/Rejected enum instead of u64
    }
};
```

Then add a runtime test where `ring_size = 1`, the shard ring is saturated, and the accepted command's receiver resolves to `RuntimeError::ShardOverloaded`.

### WR-02: Runtime Idempotency Replays Can Fail Before The Store Dedupe Check

**File:** `crates/es-runtime/src/shard.rs:171`

**Issue:** Duplicate command handling occurs only after `decide` and `store.append`. A retried create command with the same tenant/idempotency key is first rehydrated into the already-updated aggregate state and then passed through `A::decide` at line 188. If the aggregate rejects the repeated command because the stream already exists, the runtime returns a domain error and never reaches the durable `AppendOutcome::Duplicate` branch at line 275. This breaks idempotent retry semantics even though the store layer correctly returns the original append result for duplicate keys.

**Fix:** Check runtime/store idempotency before rehydration and domain decision, and return the original command outcome for duplicate keys. Because the generic runtime currently stores only `CommittedAppend`, the durable dedupe record should also include enough response data to reconstruct the original `A::Reply`, or the runtime should persist a command-result payload through the codec:

```rust
if let Some(record) = self.dedupe.get(&DedupeKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    idempotency_key: envelope.idempotency_key.clone(),
}) {
    let reply = codec.decode_reply(&record.response_payload)?;
    let _ = envelope.reply.send(Ok(CommandOutcome::new(reply, record.append.clone())));
    return Ok(true);
}
```

Add a runtime integration test that submits the same create command twice with the same idempotency key and asserts that both replies are successful duplicates rather than a domain rejection.

### WR-03: Projection Can Panic On Event Type/Payload Mismatch

**File:** `crates/es-store-postgres/src/projection.rs:242`

**Issue:** `apply_projection_event` treats mismatched `event_type` and decoded payload variants as unreachable. Event type and JSON payload are independent stored fields, and `NewEvent` can be constructed with an event type that does not match the serialized enum variant. If a stored row says `OrderPlaced` but the payload decodes to another `OrderEvent` variant, the projector panics instead of returning `ProjectionError::PayloadDecode` or ignoring an unknown event. The same pattern appears for all order and product variants.

**Fix:** Replace the `unreachable!` destructuring with explicit validation that returns a projection error. For example:

```rust
let decoded = decode_order_payload(event)?;
let OrderEvent::OrderPlaced { order_id, user_id, lines } = decoded else {
    return Err(ProjectionError::PayloadDecode {
        event_type: event.event_type.clone(),
        schema_version: event.schema_version,
    });
};
```

Apply the same guard to the other order/product branches and add a test with a mismatched event type and payload variant.

### WR-04: Projection Lag Metric Underreports Backlog

**File:** `crates/es-store-postgres/src/projection.rs:108`

**Issue:** `es_projection_lag` is set to `last_global_position - current_offset` for the batch just read. With a batch limit of 100 and a backlog of 10,000 events, the gauge reports about 100 rather than the remaining distance to the tenant's latest committed global position. When no events are read it sets lag to zero without checking whether this projector is actually caught up to the event store for that tenant. This produces a false freshness signal.

**Fix:** Compute lag against the tenant's durable max global position, not the current batch size. Query `SELECT COALESCE(max(global_position), 0) FROM events WHERE tenant_id = $1`, and after commit set lag to `(tenant_latest_position - last_global_position).max(0)`. In the idle branch, set lag to `(tenant_latest_position - current_offset).max(0)` instead of unconditional zero.

### WR-05: Integrated Stress Report Emits Synthetic Or Always-Zero Signals

**File:** `crates/app/src/stress.rs:222`

**Issue:** The stress runner reports fields that look operationally meaningful but are not measuring the named thing. `append_latency.record(elapsed)` records full command round-trip latency, not append latency. `shard_depth_max` is derived from `ingress_depth_max.min(config.ring_size)` rather than observed shard state. `sample_projection_lag` computes local lag variables but always returns `Ok(0)`, so projection lag is reported as zero even when catch-up is limited or backlog remains. These false signals conflict with the stress-results guidance that separates queue pressure, append latency, projection lag, and outbox lag.

**Fix:** Either remove/rename these fields until they are measured, or populate them from real instrumentation. At minimum:

```rust
// Track append latency from the event-store append span/metric or return it from the runtime.
// Track shard depth from ShardState::pending_handoffs per shard.
let tenant_lag = (tenant_latest_global_position - after).max(0);
max_projection_lag = max_projection_lag.max(tenant_lag);
```

Update the smoke tests to assert a nonzero lag in a controlled backlog case, and avoid presenting synthetic queue depths as measured values.

---

_Reviewed: 2026-04-18T15:19:55Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

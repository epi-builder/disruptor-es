# Phase 8: Runtime Duplicate Command Replay - Research

**Researched:** 2026-04-19 [VERIFIED: environment current_date]
**Domain:** Rust event-sourced command runtime idempotency across shard cache, PostgreSQL command dedupe, HTTP adapter replies, and process-manager replay [VERIFIED: .planning/ROADMAP.md]
**Confidence:** HIGH for local architecture and required change points; MEDIUM for durable reply-payload scope because the existing storage schema stores append summaries but not typed runtime replies [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-runtime/src/command.rs]

## User Constraints

No phase-specific `CONTEXT.md` exists for Phase 8, so there are no locked decisions, discretion areas, or deferred ideas to copy for this phase. [VERIFIED: `node ... gsd-tools.cjs init phase-op "8"` returned `has_context: false`; VERIFIED: `find .planning/phases -name '*CONTEXT.md'`]

Project constraints that apply:

- Prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: AGENTS.md instructions provided in prompt]
- Keep the implementation Rust-first. [VERIFIED: AGENTS.md project doc]
- Treat the event store as the source of truth; disruptor rings must never be durable state. [VERIFIED: AGENTS.md project doc]
- Route the same aggregate or ordered partition key to the same shard owner. [VERIFIED: AGENTS.md project doc]
- Keep hot business state single-owner and processor-local where practical; avoid shared mutable state in adapter handlers. [VERIFIED: AGENTS.md project doc]
- Publish externally only through outbox rows committed in the same transaction as domain events. [VERIFIED: AGENTS.md project doc]
- Use GSD workflow artifacts for repo-changing work. [VERIFIED: AGENTS.md workflow section]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STORE-03 | Command deduplication returns the prior committed result for repeated tenant/idempotency key. [VERIFIED: .planning/REQUIREMENTS.md] | PostgreSQL already has tenant-scoped `command_dedup` with transaction-level advisory locking and `response_payload`; Phase 8 must use it before domain decision on cache miss and must extend the durable payload if typed replies must survive restart. [VERIFIED: crates/es-store-postgres/src/sql.rs; CITED: https://www.postgresql.org/docs/current/functions-admin.html] |
| RUNTIME-03 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. [VERIFIED: .planning/REQUIREMENTS.md] | `ShardState` owns `AggregateCache` and `DedupeCache`; Phase 8 should add lookup/replay inside `ShardState::process_next_handoff` without introducing `Arc<Mutex<_>>` hot business maps. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/src/cache.rs] |
| RUNTIME-05 | Command replies are sent only after durable event-store append commit succeeds. [VERIFIED: .planning/REQUIREMENTS.md] | Current first-attempt path is commit-gated, but duplicate replay currently depends on a fresh domain decision; Phase 8 must send replay replies only from already committed cache/store records. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| INT-04 | Process-manager follow-up commands use the same command gateway. [VERIFIED: .planning/REQUIREMENTS.md] | `CommerceOrderProcessManager` submits deterministic follow-up idempotency keys through `CommandGateway`; Phase 8 must prove replay after offset-not-advanced retry. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs] |
| API-01 | HTTP command endpoints decode requests, attach metadata, send through bounded ingress, and await replies. [VERIFIED: .planning/REQUIREMENTS.md] | `adapter-http` already creates `CommandEnvelope`s and awaits `CommandOutcome`; Phase 8 should verify duplicate HTTP requests replay the first response contract. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| API-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads. [VERIFIED: .planning/REQUIREMENTS.md] | `CommandSuccess` maps `CommandOutcome.reply` plus `CommittedAppend`; Phase 8 must preserve the original success/error shape rather than recomputing it from mutated aggregate state. [VERIFIED: crates/adapter-http/src/commerce.rs] |

</phase_requirements>

## Summary

Phase 8 should implement duplicate replay as an idempotency layer in the runtime, ordered before rehydration and before `A::decide`. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] The first lookup should be shard-local and cheap, using the tenant/idempotency key that is already carried on `CommandEnvelope`; on a hit, the shard should send a replayed `CommandOutcome` and must not load aggregate state, call `decide`, encode events, or append. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-runtime/src/cache.rs]

The existing durable PostgreSQL path is the correct source of truth for restart/cache-miss behavior because it serializes by tenant/idempotency key with `pg_advisory_xact_lock`, checks `command_dedup`, and stores a JSON response payload in the same transaction after events/outbox rows are inserted. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: migrations/20260417000000_event_store.sql; CITED: https://www.postgresql.org/docs/current/explicit-locking.html] The current payload contains `CommittedAppend`, not `A::Reply`, so a strict "same HTTP reply after restart" guarantee requires a durable typed reply payload or a formally owned replay-decoder path. [VERIFIED: crates/es-store-postgres/src/models.rs; VERIFIED: crates/es-runtime/src/command.rs]

**Primary recommendation:** Add a runtime-facing idempotency lookup/replay contract that returns a full `CommandOutcome<A::Reply>` before aggregate decision; back it with shard-local cache for warm hits and durable PostgreSQL dedupe for cache misses/restarts. [VERIFIED: local code audit]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Warm duplicate replay | Runtime shard | API adapter | The shard owns `DedupeCache` and can reply before rehydration/decision; the adapter only forwards idempotency keys and renders returned outcomes. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/adapter-http/src/commerce.rs] |
| Durable duplicate replay | Database / Storage | Runtime shard | PostgreSQL owns authoritative tenant/idempotency rows and transaction serialization; runtime consumes the result before domain decision on cache misses. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-runtime/src/store.rs] |
| Original API response shape | Runtime shard | HTTP adapter | Runtime must return the original `CommandOutcome`; HTTP maps that outcome into `CommandSuccess` with correlation ID and typed reply DTO. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/adapter-http/src/commerce.rs] |
| Process-manager retry safety | App composition / Process manager | Runtime shard, Storage | Process managers already submit deterministic follow-up keys through gateways; runtime/storage must make replays no-op with original replies before offsets advance. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs] |
| Outbox non-duplication | Storage | Outbox dispatcher | Append-created outbox rows are inserted in the same transaction as events and are not created on `AppendOutcome::Duplicate`. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-store-postgres/tests/outbox.rs] |

## Standard Stack

### Core

| Library / Component | Version | Purpose | Why Standard |
|---------------------|---------|---------|--------------|
| Rust workspace | Rust 1.85.1 available; workspace `rust-version = "1.85"` [VERIFIED: `rustc --version`; VERIFIED: Cargo.toml] | Implement runtime/storage/API changes. | The project is Rust-first and uses Rust 2024 workspace policy. [VERIFIED: Cargo.toml; VERIFIED: AGENTS.md project doc] |
| `tokio` | 1.52.0 [VERIFIED: Cargo.toml] | Oneshot replies and async runtime processing. | Runtime and adapter paths already use `tokio::sync::oneshot` and async store calls. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/adapter-http/src/commerce.rs] |
| `sqlx` | 0.8.6 [VERIFIED: Cargo.toml] | PostgreSQL event-store and dedupe SQL. | Existing store implementation uses explicit SQL transactions and compile-time typed models. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: Cargo.toml] |
| PostgreSQL `command_dedup` | Migration-backed schema [VERIFIED: migrations/20260417000000_event_store.sql] | Durable source of truth for tenant/idempotency replay. | Existing append transaction checks and records dedupe inside PostgreSQL. [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| `serde` / `serde_json` | `serde` 1.0.228, `serde_json` 1.0.149 [VERIFIED: Cargo.toml] | Serialize durable append/reply payloads when schema is extended. | Existing store DTOs and event payloads already derive/use serde JSON. [VERIFIED: crates/es-store-postgres/src/models.rs] |
| `axum` / `tower` | `axum` 0.8.9, `tower` 0.5.3 [VERIFIED: Cargo.toml] | HTTP duplicate retry verification. | Existing adapter uses Axum routes and Tower `oneshot` tests. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: crates/adapter-http/tests/commerce_api.rs] |

### Supporting

| Library / Component | Version | Purpose | When to Use |
|---------------------|---------|---------|-------------|
| `metrics` | 0.24.3 [VERIFIED: Cargo.toml] | Count/label runtime cache hits and duplicate replay outcomes. | Add or reuse metrics around runtime dedupe hits and durable dedupe hits. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-store-postgres/src/event_store.rs] |
| `tracing` | 0.1.44 [VERIFIED: Cargo.toml] | Record duplicate replay spans with command/correlation/idempotency context. | Extend existing `shard.process_handoff` and `http.command` spans. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/adapter-http/src/commerce.rs] |
| `testcontainers` | 0.25.0 with `testcontainers-modules` 0.13.0 [VERIFIED: Cargo.toml] | Real PostgreSQL integration tests for restart/cache-miss dedupe. | Use for durable store replay tests and process-manager integration retry cases. [VERIFIED: crates/es-store-postgres/tests/dedupe.rs; VERIFIED: crates/es-store-postgres/tests/outbox.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Shard-local `DedupeCache` + PostgreSQL source of truth | Global in-memory dedupe map | Reject: a global map violates shard-local hot-state ownership and fails restart/source-of-truth requirements. [VERIFIED: AGENTS.md constraints; VERIFIED: crates/es-runtime/src/cache.rs] |
| Durable typed replay payload | Re-run `A::decide` and rely on `AppendOutcome::Duplicate` | Reject: the audit proves re-running decision can surface fresh domain errors before storage dedupe. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: crates/es-runtime/src/shard.rs] |
| PostgreSQL `command_dedup` | Disruptor sequence or ring state | Reject: rings are in-process execution fabric only and not durable state. [VERIFIED: AGENTS.md constraints; VERIFIED: crates/es-runtime/src/shard.rs] |

**Installation:** No new packages should be added for Phase 8. [VERIFIED: Cargo.toml already contains runtime, storage, HTTP, metrics, tracing, serde, sqlx, and testcontainers dependencies]

## Architecture Patterns

### System Architecture Diagram

```text
HTTP / Process Manager retry
        |
        v
CommandGateway bounded ingress
        |
        v
Shard owner receives handoff
        |
        v
Build DedupeKey(tenant_id, idempotency_key)
        |
        +--> shard-local dedupe hit?
        |        |
        |        v
        |   send original CommandOutcome; stop before rehydrate/decide/append
        |
        +--> durable dedupe lookup hit?
        |        |
        |        v
        |   hydrate shard cache if needed; record local dedupe; send original CommandOutcome; stop before decide/append
        |
        v
rehydrate aggregate state -> A::decide -> encode events -> PostgreSQL append transaction
        |
        +--> committed
        |        |
        |        v
        |   update aggregate cache; record full dedupe outcome; reply success
        |
        +--> duplicate returned by append race
                 |
                 v
            record full dedupe outcome; reply replayed outcome without applying new decision events
```

### Recommended Project Structure

```text
crates/es-runtime/src/
├── cache.rs        # Extend DedupeRecord to carry replayable CommandOutcome data, not append only.
├── command.rs      # Add typed replay payload contract if needed by RuntimeEventCodec.
├── shard.rs        # Add pre-decision dedupe lookup and replay branch.
├── store.rs        # Add runtime-facing durable dedupe lookup method.
└── tests/
    └── runtime_flow.rs  # Prove cache-hit and durable-fallback duplicate replay skips decide/append.

crates/es-store-postgres/src/
├── models.rs      # Extend durable command dedupe response model if full reply persistence is chosen.
├── sql.rs         # Add/select dedupe lookup before append, or generalize existing private select.
└── tests/
    └── dedupe.rs  # Prove stored replay payload returns original result after restart.

crates/adapter-http/tests/
└── commerce_api.rs # Prove duplicate retry returns original HTTP response contract.

crates/app/src/
└── commerce_process_manager.rs # Add deterministic retry test for follow-up command replay.
```

### Pattern 1: Idempotency Lookup Before Execution

**What:** Check `DedupeKey { tenant_id, idempotency_key }` before loading aggregate state or calling `A::decide`. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: crates/es-runtime/src/shard.rs]

**When to use:** Every `ShardState::process_next_handoff` command, regardless of HTTP or process-manager source. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/app/src/commerce_process_manager.rs]

**Example:**

```rust
// Source: crates/es-runtime/src/shard.rs + crates/es-runtime/src/cache.rs
let dedupe_key = DedupeKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    idempotency_key: envelope.idempotency_key.clone(),
};

if let Some(record) = self.dedupe.get(&dedupe_key) {
    let outcome = record.to_command_outcome::<A, C>(codec)?;
    let _ = envelope.reply.send(Ok(outcome));
    return Ok(true);
}
```

### Pattern 2: Durable Dedupe Is a Read Path, Not Only an Append Outcome

**What:** Expose a runtime-facing store method that can lookup a committed idempotency result by tenant/idempotency key before append. [VERIFIED: crates/es-runtime/src/store.rs currently exposes only `append` and `load_rehydration`; VERIFIED: crates/es-store-postgres/src/sql.rs has private dedupe select helpers]

**When to use:** Cache miss, process restart, or any duplicate submitted to a different runtime instance in a future deployment. [VERIFIED: .planning/ROADMAP.md Phase 8 success criteria]

**Example:**

```rust
// Source: crates/es-runtime/src/store.rs existing trait style
fn lookup_dedupe(
    &self,
    tenant_id: es_core::TenantId,
    idempotency_key: String,
) -> BoxFuture<'_, es_store_postgres::StoreResult<Option<StoredCommandOutcome>>>;
```

### Pattern 3: Persist Enough to Replay the Response

**What:** Store enough durable data to return the original `CommandOutcome<A::Reply>` without running `A::decide`. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/models.rs]

**When to use:** Required for restart/cache-miss duplicate replay if the API must return typed reply payloads after runtime memory is gone. [VERIFIED: .planning/ROADMAP.md Phase 8 success criteria; VERIFIED: crates/adapter-http/src/commerce.rs]

**Example:**

```rust
// Source: proposed shape based on crates/es-runtime/src/command.rs
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StoredCommandOutcome {
    pub append: es_store_postgres::CommittedAppend,
    pub reply_payload: serde_json::Value,
    pub reply_type: String,
}
```

### Pattern 4: Process-Manager Replay Uses the Same Idempotency Contract

**What:** Keep deterministic follow-up keys in the existing `pm:{manager}:{source_event_id}:{action}:{target_id}` shape, and rely on runtime/store replay when the same source event is processed again before offset advancement. [VERIFIED: crates/app/src/commerce_process_manager.rs]

**When to use:** Crash/retry after follow-up commands succeeded but before `advance_process_manager_offset`. [VERIFIED: crates/es-outbox/src/process_manager.rs]

**Example:**

```rust
// Source: crates/app/src/commerce_process_manager.rs
format!(
    "pm:{}:{}:reserve:{}",
    self.name.as_str(),
    event.event_id,
    product_id.as_str()
)
```

### Anti-Patterns to Avoid

- **Checking idempotency after rehydration/decision:** This recreates the current bug where duplicate commands can see already-mutated state and produce fresh domain errors. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: crates/es-runtime/src/shard.rs]
- **Returning `decision.reply` for `AppendOutcome::Duplicate`:** The current duplicate append branch uses the fresh decision reply, which is not necessarily the original reply. [VERIFIED: crates/es-runtime/src/shard.rs]
- **Applying events from a duplicate decision to cache:** The current code avoids applying duplicate decision events, and Phase 8 must preserve that behavior. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/tests/runtime_flow.rs]
- **Treating runtime cache as authoritative:** `DedupeCache` is an optimization; PostgreSQL remains authoritative after cache miss/restart. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: .planning/ROADMAP.md Phase 8 success criteria]
- **Adding adapter-side duplicate logic:** HTTP has no aggregate state and should not mutate or inspect hot business state directly. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: AGENTS.md constraints]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Durable idempotency serialization | A custom in-memory or file lock | PostgreSQL `command_dedup` plus transaction-level advisory locks | Existing store already serializes tenant/idempotency append attempts; PostgreSQL docs define transaction-level advisory locks that wait and auto-release at transaction end. [VERIFIED: crates/es-store-postgres/src/sql.rs; CITED: https://www.postgresql.org/docs/current/functions-admin.html; CITED: https://www.postgresql.org/docs/current/explicit-locking.html] |
| Retry response storage | Recomputing reply by re-running domain logic | Persist or decode a stored replay payload | Re-running `decide` is the broken path and can surface fresh domain errors. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| Process-manager duplicate suppression | Per-manager memory set | Existing deterministic idempotency keys through `CommandGateway` | Offset replay after crash must work after process restart, so durable command dedupe must own correctness. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs] |
| HTTP duplicate retry handling | Endpoint-local cache | Runtime/store idempotency contract | HTTP tests should prove behavior, but HTTP should remain a thin adapter. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: AGENTS.md constraints] |
| New event sourcing framework | External CQRS/event-sourcing crate | Existing project-owned runtime/store contracts | The project already owns precise append, outbox, projection, and shard semantics. [VERIFIED: Cargo.toml; VERIFIED: crates/es-runtime/src/store.rs; VERIFIED: crates/es-store-postgres/src/sql.rs] |

**Key insight:** Idempotency is an execution-boundary invariant, not a domain invariant; it must intercept repeated commands before the domain sees already-mutated state. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; CITED: https://docs.stripe.com/api/idempotent_requests]

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | PostgreSQL `command_dedup` rows store tenant/idempotency key, stream/revision/global-position metadata, event IDs, and `response_payload`. [VERIFIED: migrations/20260417000000_event_store.sql; VERIFIED: crates/es-store-postgres/src/sql.rs] | Add/read enough durable response data to replay `CommandOutcome<A::Reply>` after restart. If schema changes, add a migration and backfill/compat decode for existing `CommittedAppend` payloads. [VERIFIED: crates/es-store-postgres/src/models.rs] |
| Live service config | None found; the app binary does not currently compose a runnable HTTP service entrypoint and no external service UI config was found in repo evidence. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: crates/app/src/main.rs] | None for Phase 8. |
| OS-registered state | None found in repo evidence; no systemd/launchd/pm2/task registration files were found in the provided/local scope. [VERIFIED: `find`/repo scan context] | None for Phase 8. |
| Secrets/env vars | No idempotency-specific secret or environment variable names were found in the scanned runtime/store/adapter/app paths. [VERIFIED: `rg idempotency crates ...`] | None for Phase 8. |
| Build artifacts | Existing Rust build artifacts are under `target/`, but Phase 8 does not rename packages or installed artifacts. [VERIFIED: repo listing] | None beyond normal `cargo test` rebuilds. |

## Common Pitfalls

### Pitfall 1: Replaying Too Late

**What goes wrong:** Duplicate commands rehydrate already-mutated state and call `A::decide`, producing fresh domain errors before durable dedupe can return the original result. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

**Why it happens:** The current `process_next_handoff` order is rehydrate/cache -> decide -> encode -> append, and `DedupeCache::get` is not called. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/src/cache.rs]

**How to avoid:** Lookup local and durable dedupe immediately after extracting the envelope and before aggregate state access. [VERIFIED: local architecture]

**Warning signs:** Tests assert store append returns `Duplicate`, but no test proves `decide` was skipped. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

### Pitfall 2: Persisting Only Append Metadata

**What goes wrong:** After restart, storage can replay `CommittedAppend` but cannot produce `A::Reply` for `CommandOutcome<A::Reply>` without re-running domain logic. [VERIFIED: crates/es-store-postgres/src/models.rs; VERIFIED: crates/es-runtime/src/command.rs]

**Why it happens:** `command_dedup.response_payload` currently serializes `CommittedAppend` only. [VERIFIED: crates/es-store-postgres/src/sql.rs]

**How to avoid:** Store a versioned command-outcome replay payload or add a codec-owned durable reply decoder that uses committed event payloads without calling `decide`. [VERIFIED: local code constraints]

**Warning signs:** Duplicate path calls `CommandOutcome::new(decision.reply, committed)` in an `AppendOutcome::Duplicate` branch. [VERIFIED: crates/es-runtime/src/shard.rs]

### Pitfall 3: Confusing Retryable Validation With Stored Results

**What goes wrong:** The implementation stores request validation failures or pre-execution errors as durable idempotency outcomes, making transient/invalid requests sticky. [CITED: https://docs.stripe.com/api/idempotent_requests]

**Why it happens:** Generic idempotency advice is often copied without separating "endpoint execution began" from "request never entered command execution." [CITED: https://docs.stripe.com/api/idempotent_requests]

**How to avoid:** For this phase, persist replay data only for durably committed append results unless the planner explicitly adds durable error-result scope. [ASSUMED]

**Warning signs:** Schema changes add durable rows before append commit or for adapter validation errors. [VERIFIED: crates/es-store-postgres/src/sql.rs current append order]

### Pitfall 4: Process-Manager Offset Tests That Do Not Simulate Retry

**What goes wrong:** Tests prove deterministic keys but do not prove a crash/retry before offset advancement returns original follow-up replies. [VERIFIED: crates/app/src/commerce_process_manager.rs tests; VERIFIED: crates/es-outbox/src/process_manager.rs]

**Why it happens:** Existing process-manager tests use fake gateways and manually complete replies; they do not drive the full runtime/store dedupe path. [VERIFIED: crates/app/src/commerce_process_manager.rs]

**How to avoid:** Add a test that processes the same `OrderPlaced` event twice with the same durable store state and asserts follow-up appends are not duplicated. [VERIFIED: .planning/ROADMAP.md Phase 8 success criteria]

**Warning signs:** Test only checks idempotency key string formatting. [VERIFIED: crates/app/src/commerce_process_manager.rs]

### Pitfall 5: Deadlocks From Inconsistent Lock Ordering

**What goes wrong:** New SQL paths acquire stream and idempotency locks in a different order from append, increasing deadlock risk. [CITED: https://www.postgresql.org/docs/current/explicit-locking.html]

**Why it happens:** PostgreSQL can deadlock when transactions acquire locks in inconsistent orders; the current append order is dedupe lock first, then stream lock. [VERIFIED: crates/es-store-postgres/src/sql.rs; CITED: https://www.postgresql.org/docs/current/explicit-locking.html]

**How to avoid:** Keep any durable dedupe lookup read-only or use the same dedupe-first ordering as append. [VERIFIED: crates/es-store-postgres/src/sql.rs]

**Warning signs:** New code takes stream lock before idempotency lock or holds a transaction open while awaiting runtime/domain work. [CITED: https://www.postgresql.org/docs/current/explicit-locking.html]

## Code Examples

### Existing Broken Ordering

```rust
// Source: crates/es-runtime/src/shard.rs
let current_state = if let Some(cached) = self.cache.get(&envelope.stream_id) {
    cached.clone()
} else {
    rehydrate_state(store, codec, &envelope).await?
};

let decision = A::decide(&current_state, envelope.command, &envelope.metadata)?;
```

### Existing Durable Dedupe Transaction Shape

```rust
// Source: crates/es-store-postgres/src/sql.rs
acquire_dedupe_lock(&mut tx, &request).await?;

if let Some(committed) = select_dedupe_result(&mut tx, &request).await? {
    tx.commit().await?;
    return Ok(AppendOutcome::Duplicate(committed));
}
```

### Existing HTTP Response Mapping

```rust
// Source: crates/adapter-http/src/commerce.rs
let outcome = receiver.await.map_err(|_| ApiError::ReplyDropped)??;
Ok(CommandSuccess::from_outcome(stream_id, outcome, map_reply))
```

### Existing Process-Manager Deterministic Key

```rust
// Source: crates/app/src/commerce_process_manager.rs
format!(
    "pm:{}:{}:confirm:{}",
    self.name.as_str(),
    event.event_id,
    order_id.as_str()
)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Idempotency checked only at durable append after domain decision. [VERIFIED: crates/es-runtime/src/shard.rs] | Idempotency checked before execution and returns the first response body/status for retries. [CITED: https://docs.stripe.com/api/idempotent_requests] | This is established API idempotency practice; local Phase 8 exists because the runtime currently violates it. [VERIFIED: .planning/ROADMAP.md] | Runtime duplicate replay must be a pre-decision gate, not an append-only side effect. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| Runtime cache stores append summary only. [VERIFIED: crates/es-runtime/src/cache.rs] | Replay cache stores enough to reproduce the caller-visible outcome. [CITED: https://docs.stripe.com/api/idempotent_requests] | Needed now because API and process-manager retries require original reply shapes. [VERIFIED: .planning/ROADMAP.md] | Extend local and durable records or introduce a replay decoder; do not return fresh `decision.reply`. [VERIFIED: crates/es-runtime/src/shard.rs] |
| Session/global locks for idempotency. [ASSUMED] | Transaction-level PostgreSQL advisory locks and row-backed dedupe. [VERIFIED: crates/es-store-postgres/src/sql.rs; CITED: https://www.postgresql.org/docs/current/explicit-locking.html] | Already implemented in Phase 2 store. [VERIFIED: .planning/STATE.md] | Preserve dedupe-first lock ordering and auto-release behavior. [CITED: https://www.postgresql.org/docs/current/functions-admin.html] |

**Deprecated/outdated:**

- Do not treat `AppendOutcome::Duplicate(committed)` as enough for typed runtime replay unless the reply can be recovered without `decide`. [VERIFIED: crates/es-store-postgres/src/models.rs; VERIFIED: crates/es-runtime/src/command.rs]
- Do not rely on ring sequence or in-memory cache for correctness after restart. [VERIFIED: AGENTS.md constraints; VERIFIED: .planning/ROADMAP.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 8 should persist/replay only durably committed append results, not adapter validation failures or pre-append domain errors, unless the user expands scope. | Common Pitfalls | If the user expects Stripe-style replay of all endpoint execution errors, the plan must add a broader durable API idempotency result table and HTTP status/body persistence. |
| A2 | Existing committed reply types can be serialized to JSON or decoded from committed event payloads without adding a new external crate. | Architecture Patterns | If a reply cannot be serialized or deterministically decoded, planner must add trait bounds/API changes or narrow replay guarantees. |
| A3 | No runtime state outside PostgreSQL and process memory needs migration for Phase 8. | Runtime State Inventory | If deployed services exist outside repo evidence, operators may need to drain/restart them after schema/runtime changes. |

## Open Questions (RESOLVED)

1. **Should durable idempotency store typed replies or derive replies from committed events?**
   - What we know: `CommandOutcome<R>` needs `reply: R` and `append: CommittedAppend`; durable `response_payload` currently stores only `CommittedAppend`. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/sql.rs]
   - What's unclear: Whether all future domains can provide a stable reply decoder from committed event payloads. [ASSUMED]
   - Recommendation: Persist a versioned reply payload through a runtime codec extension because it is the most direct way to satisfy restart/API replay. [VERIFIED: local code constraints]
   - RESOLVED: Durable dedupe persists typed reply payloads using `CommandReplayRecord { append, reply }` inside `command_dedup.response_payload`; do not reconstruct replies by calling `A::decide`.

2. **Should "error reply shape" include non-committed domain/API errors?**
   - What we know: Current durable dedupe row is inserted after events/outbox rows and committed append metadata are available. [VERIFIED: crates/es-store-postgres/src/sql.rs]
   - What's unclear: Whether the phrase means accepted domain failure events/replies or all HTTP errors. [ASSUMED]
   - Recommendation: Scope Phase 8 to committed command outcomes; keep adapter validation and pre-append domain errors retryable unless requirements are clarified. [ASSUMED]
   - RESOLVED: Phase 8 replays committed command outcomes only; pre-append validation/domain errors are not persisted or replayed in this phase.

3. **Should mismatched payload reuse of an idempotency key be rejected?**
   - What we know: Stripe compares request parameters to prevent accidental misuse; current local store keys only by tenant/idempotency and does not store a request fingerprint. [CITED: https://docs.stripe.com/api/idempotent_requests; VERIFIED: migrations/20260417000000_event_store.sql]
   - What's unclear: Whether Phase 8 requires misuse detection or only duplicate replay. [ASSUMED]
   - Recommendation: Do not add request fingerprinting in Phase 8 unless planner has capacity after replay correctness; document it as future hardening. [ASSUMED]
   - RESOLVED: Mismatched command payload reuse is deferred hardening unless covered by lightweight request fingerprint validation already planned.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust `cargo` | Unit/integration tests | yes [VERIFIED: `cargo --version`] | 1.85.1 | none |
| Rust `rustc` | Workspace compile | yes [VERIFIED: `rustc --version`] | 1.85.1 | none |
| Docker | Testcontainers PostgreSQL tests | yes [VERIFIED: `docker --version`] | 29.3.1 client | Use existing unit/fake-store tests if Docker daemon is unavailable, but durable restart tests need PostgreSQL. |
| `cargo-nextest` | Optional faster test runner | no [VERIFIED: `command -v cargo-nextest`] | - | Use `cargo test`. |
| `psql` | Manual DB inspection | no [VERIFIED: `command -v psql`] | - | Use SQLx/testcontainers tests. |

**Missing dependencies with no fallback:**

- None for code implementation; durable integration verification requires Docker/Testcontainers, and Docker CLI is present. [VERIFIED: environment audit]

**Missing dependencies with fallback:**

- `cargo-nextest` is missing; use `cargo test`. [VERIFIED: environment audit]
- `psql` is missing; use SQLx-driven tests. [VERIFIED: environment audit]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness with `tokio::test`, Testcontainers for PostgreSQL integration, Tower `oneshot` for Axum tests. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs; VERIFIED: crates/es-store-postgres/tests/dedupe.rs; VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| Config file | none; workspace tests are driven by Cargo. [VERIFIED: repo scan] |
| Quick run command | `cargo test -p es-runtime runtime_duplicate -- --nocapture` after adding named tests. [VERIFIED: Cargo.toml workspace] |
| Full suite command | `cargo test --workspace --all-targets` [VERIFIED: Cargo.toml workspace] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| STORE-03 | Durable dedupe lookup replays original committed outcome on runtime cache miss/restart. [VERIFIED: .planning/ROADMAP.md] | integration | `cargo test -p es-store-postgres duplicate_idempotency_key_returns_original_result -- --nocapture` plus new replay-payload test | existing file yes; new test needed [VERIFIED: crates/es-store-postgres/tests/dedupe.rs] |
| RUNTIME-03 | Shard-local dedupe hit skips rehydration, `decide`, encode, and append. [VERIFIED: .planning/ROADMAP.md] | unit/integration | `cargo test -p es-runtime runtime_duplicate_cache_hit_skips_decide_and_append -- --nocapture` | no; Wave 0 gap [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| RUNTIME-05 | Duplicate reply is sent from committed replay data only. [VERIFIED: .planning/ROADMAP.md] | unit/integration | `cargo test -p es-runtime duplicate_replay_returns_original_reply_after_state_mutation -- --nocapture` | no; Wave 0 gap [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| INT-04 | Process-manager follow-up retry reuses deterministic keys and receives original outcomes. [VERIFIED: .planning/ROADMAP.md] | integration | `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` | no; Wave 0 gap [VERIFIED: crates/app/src/commerce_process_manager.rs] |
| API-01 | HTTP duplicate retry travels through adapter/gateway/runtime path. [VERIFIED: .planning/ROADMAP.md] | adapter integration | `cargo test -p adapter-http duplicate_place_order_retry_returns_original_response -- --nocapture` | no; Wave 0 gap [VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| API-03 | Duplicate HTTP response contains original stream revision, global positions, event IDs, correlation behavior, and typed reply/error shape. [VERIFIED: .planning/ROADMAP.md] | adapter integration | `cargo test -p adapter-http duplicate_place_order_retry_returns_original_response -- --nocapture` | no; Wave 0 gap [VERIFIED: crates/adapter-http/tests/commerce_api.rs] |

### Sampling Rate

- **Per task commit:** `cargo test -p es-runtime runtime_flow -- --nocapture` or the narrow package touched. [VERIFIED: Cargo workspace]
- **Per wave merge:** `cargo test -p es-runtime && cargo test -p adapter-http && cargo test -p app` [VERIFIED: Cargo workspace]
- **Phase gate:** `cargo test --workspace --all-targets` before `/gsd-verify-work`. [VERIFIED: Cargo workspace]

### Wave 0 Gaps

- [ ] `crates/es-runtime/tests/runtime_flow.rs` - add cache-hit and durable-fallback duplicate replay tests for RUNTIME-03/RUNTIME-05. [VERIFIED: file exists]
- [ ] `crates/es-store-postgres/tests/dedupe.rs` - add durable replay-payload test if schema/model is extended for STORE-03. [VERIFIED: file exists]
- [ ] `crates/adapter-http/tests/commerce_api.rs` - add duplicate retry response contract for API-01/API-03. [VERIFIED: file exists]
- [ ] `crates/app/src/commerce_process_manager.rs` tests or a new `crates/app/tests/...` integration test - add process-manager retry replay case for INT-04. [VERIFIED: file exists]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | No authentication is in Phase 8 scope. [VERIFIED: .planning/ROADMAP.md Phase 8] |
| V3 Session Management | no | No sessions are in Phase 8 scope. [VERIFIED: .planning/ROADMAP.md Phase 8] |
| V4 Access Control | yes | Tenant-scoped idempotency keys must always include `tenant_id` in cache and durable lookups. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: crates/es-store-postgres/src/sql.rs] |
| V5 Input Validation | yes | Continue rejecting empty idempotency keys and invalid tenant IDs before runtime submission. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/adapter-http/src/commerce.rs] |
| V6 Cryptography | no | No cryptography is introduced; UUIDs and JSON payloads already exist. [VERIFIED: Cargo.toml; VERIFIED: crates/es-core/src/lib.rs] |
| V10 Server-Side Request Forgery | no | No outbound network request is introduced. [VERIFIED: .planning/ROADMAP.md Phase 8] |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant idempotency replay | Elevation / Information disclosure | Include `tenant_id` in `DedupeKey` and SQL primary key/lookups. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: migrations/20260417000000_event_store.sql] |
| Idempotency-key reuse with different command payload | Tampering | Recommended future hardening: store request fingerprint and reject mismatches; Stripe documents parameter comparison for this reason. [CITED: https://docs.stripe.com/api/idempotent_requests] |
| Replay cache poisoning before commit | Tampering | Record local and durable dedupe only after append commit or durable duplicate lookup. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Duplicate outbox publication | Tampering / Repudiation | Keep outbox rows inserted only during first append transaction and use `(tenant_id, source_event_id, topic)` conflict handling. [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Lock contention / deadlock | Denial of service | Preserve consistent lock ordering and keep transactions short. [VERIFIED: crates/es-store-postgres/src/sql.rs; CITED: https://www.postgresql.org/docs/current/explicit-locking.html] |

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` - affected requirements STORE-03, RUNTIME-03, RUNTIME-05, INT-04, API-01, API-03. [VERIFIED: local file]
- `.planning/ROADMAP.md` - Phase 8 goal, success criteria, gap closure scope. [VERIFIED: local file]
- `.planning/STATE.md` - prior decisions on PostgreSQL dedupe, advisory locks, deterministic process-manager keys, and runtime/store boundaries. [VERIFIED: local file]
- `.planning/v1.0-MILESTONE-AUDIT.md` - blocker evidence and broken flows. [VERIFIED: local file]
- `crates/es-runtime/src/shard.rs` - current processing order and duplicate branch. [VERIFIED: local file]
- `crates/es-runtime/src/cache.rs` - current `DedupeCache` and `DedupeRecord` shape. [VERIFIED: local file]
- `crates/es-runtime/src/command.rs` - `CommandEnvelope`, `CommandOutcome`, and codec contracts. [VERIFIED: local file]
- `crates/es-runtime/src/store.rs` - runtime store trait currently lacks public dedupe lookup. [VERIFIED: local file]
- `crates/es-store-postgres/src/sql.rs` and `models.rs` - durable append/dedupe transaction and persisted model shape. [VERIFIED: local file]
- `migrations/20260417000000_event_store.sql` - `command_dedup` schema. [VERIFIED: local file]
- `crates/adapter-http/src/commerce.rs` and tests - HTTP request/reply contract. [VERIFIED: local file]
- `crates/app/src/commerce_process_manager.rs` and `crates/es-outbox/src/process_manager.rs` - process-manager retry composition. [VERIFIED: local file]
- `Cargo.toml` - standard stack versions and workspace test setup. [VERIFIED: local file]

### Secondary (MEDIUM confidence)

- PostgreSQL 18 docs on advisory lock functions and explicit/advisory locking behavior: https://www.postgresql.org/docs/current/functions-admin.html and https://www.postgresql.org/docs/current/explicit-locking.html [CITED: official docs]
- Stripe docs on API idempotency returning the first result for retries and comparing parameters: https://docs.stripe.com/api/idempotent_requests [CITED: official docs]

### Tertiary (LOW confidence)

- None used. [VERIFIED: source list]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH - all recommended libraries/components are already in `Cargo.toml`; no new dependency is recommended. [VERIFIED: Cargo.toml]
- Architecture: HIGH for pre-decision lookup and tier ownership because local code shows the exact broken order; MEDIUM for durable typed reply persistence because it requires a design decision between storing reply payloads and codec-based replay reconstruction. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-store-postgres/src/models.rs]
- Pitfalls: HIGH for local pitfalls from audit/code; MEDIUM for generic API idempotency guidance from Stripe because this project is not Stripe but the retry invariant is directly relevant. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; CITED: https://docs.stripe.com/api/idempotent_requests]

**Research date:** 2026-04-19 [VERIFIED: environment current_date]
**Valid until:** 2026-05-19 for local architecture; re-check if storage schema or runtime contracts change before planning. [ASSUMED]

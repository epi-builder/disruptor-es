# Phase 09: Tenant-Scoped Runtime Aggregate Cache - Research

**Researched:** 2026-04-20 [VERIFIED: system date]
**Domain:** Rust event-sourced command runtime, shard-local aggregate cache identity, tenant-scoped rehydration [VERIFIED: .planning/ROADMAP.md]
**Confidence:** HIGH [VERIFIED: local code audit + targeted tests]

## Summary

Phase 9 is a narrow correctness refactor in `es-runtime`, not a new library-selection phase. [VERIFIED: .planning/ROADMAP.md] The current runtime already routes by tenant plus partition key and already performs duplicate replay lookup by tenant plus idempotency key. [VERIFIED: crates/es-runtime/src/router.rs:45] [VERIFIED: crates/es-runtime/src/shard.rs:170] The aggregate cache is the outlier: `AggregateCache` stores `HashMap<StreamId, A::State>`, and `ShardState::process_next_handoff` checks `self.cache.get(&envelope.stream_id)` before tenant-scoped rehydration. [VERIFIED: crates/es-runtime/src/cache.rs:7] [VERIFIED: crates/es-runtime/src/shard.rs:218]

The planner should keep the fix inside the shard-owned runtime state model. [VERIFIED: .planning/REQUIREMENTS.md] Introduce an owned composite cache key containing `TenantId` and `StreamId`, update `AggregateCache` APIs to accept that key or tenant+stream inputs, and update `ShardState` warm, lookup, rehydration cache-fill, and commit paths to use the same composite key. [VERIFIED: crates/es-runtime/src/cache.rs] Do not add global locks, database schema changes, new crates, or adapter-level tenant guards for this phase. [VERIFIED: .planning/ROADMAP.md]

**Primary recommendation:** Use a first-class `AggregateCacheKey { tenant_id: TenantId, stream_id: StreamId }` and make every aggregate-cache hit, fill, and commit path use that key before and after tenant-scoped rehydration. [VERIFIED: crates/es-runtime/src/cache.rs] [CITED: https://doc.rust-lang.org/std/collections/struct.HashMap.html]

## Project Constraints (from AGENTS.md / Project Context)

- Prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: user-provided AGENTS.md]
- Core implementation is Rust-first around `disruptor-rs`/`disruptor`. [VERIFIED: user-provided AGENTS.md] [VERIFIED: Cargo.toml]
- Event store is the source of truth; disruptor rings must not be treated as durable state. [VERIFIED: user-provided AGENTS.md] [VERIFIED: .planning/REQUIREMENTS.md]
- The same aggregate or ordered partition key must map to the same shard owner. [VERIFIED: user-provided AGENTS.md] [VERIFIED: crates/es-runtime/src/router.rs]
- Hot business state should stay single-owner and processor-local where practical; avoid shared mutable state in adapter handlers. [VERIFIED: user-provided AGENTS.md] [VERIFIED: .planning/REQUIREMENTS.md]
- External publication must flow through committed outbox rows, not direct command-handler publication. [VERIFIED: user-provided AGENTS.md] [VERIFIED: .planning/REQUIREMENTS.md]
- `CLAUDE.md` was not present at repository root, and no project `.claude/skills/` or `.agents/skills/` entries were found. [VERIFIED: `find . -maxdepth 3 ...`]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STORE-04 | Aggregate rehydration can load latest snapshot and replay subsequent stream events. [VERIFIED: .planning/REQUIREMENTS.md] | Runtime must not skip `load_rehydration(&tenant_id, &stream_id)` due to a stream-only cache hit. [VERIFIED: crates/es-runtime/src/store.rs:11] [VERIFIED: crates/es-runtime/src/shard.rs:218] |
| RUNTIME-03 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. [VERIFIED: .planning/REQUIREMENTS.md] | Keep `AggregateCache` inside `ShardState`; only change its key identity. [VERIFIED: crates/es-runtime/src/shard.rs:71] |
| RUNTIME-05 | Command replies are sent only after durable event-store append commit succeeds. [VERIFIED: .planning/REQUIREMENTS.md] | Preserve the existing append-then-cache-commit-then-reply sequence. [VERIFIED: crates/es-runtime/src/shard.rs:294] |
| RUNTIME-06 | Optimistic concurrency conflicts surface as typed errors without corrupting shard-local cache. [VERIFIED: .planning/REQUIREMENTS.md] | Preserve existing conflict tests and add tenant-isolation conflict coverage. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| DOM-05 | Domain invariants prevent invalid state. [VERIFIED: .planning/REQUIREMENTS.md] | Prevent runtime from evaluating a tenant's command against another tenant's aggregate state. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |

</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Aggregate cache identity | API / Backend runtime | Database / Storage | The shard runtime owns hot aggregate state, while storage remains authoritative for tenant-scoped rehydration. [VERIFIED: crates/es-runtime/src/shard.rs:71] [VERIFIED: crates/es-runtime/src/store.rs:11] |
| Tenant-scoped rehydration | Database / Storage | API / Backend runtime | PostgreSQL stream reads filter by `tenant_id` and `stream_id`; runtime decides when to call that boundary. [VERIFIED: crates/es-store-postgres/src/sql.rs:510] [VERIFIED: crates/es-runtime/src/shard.rs:221] |
| Tenant-aware routing | API / Backend runtime | — | `PartitionRouter::route` hashes tenant ID and partition key before selecting a shard. [VERIFIED: crates/es-runtime/src/router.rs:45] |
| Duplicate replay interaction | API / Backend runtime | Database / Storage | Shard-local dedupe is tenant-scoped and durable lookup is tenant-scoped before aggregate rehydration. [VERIFIED: crates/es-runtime/src/cache.rs:51] [VERIFIED: crates/es-runtime/src/shard.rs:187] |
| Regression tests | Test suite | — | Existing `es-runtime` tests are the closest executable surface for this correctness gap. [VERIFIED: cargo test -p es-runtime -- --nocapture] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std `HashMap` | std 1.85 workspace floor; docs checked against std 1.95.0 page | Shard-local map storage for cache keys to aggregate state | Existing implementation already uses `HashMap`; Rust docs require keys to implement `Eq` and `Hash`, commonly derived for custom key types. [VERIFIED: Cargo.toml] [CITED: https://doc.rust-lang.org/std/collections/struct.HashMap.html] |
| `es-core` typed IDs | workspace crate 0.1.0 | `TenantId` and `StreamId` identity components | Existing runtime/store APIs already carry these typed IDs, avoiding raw-string cache keys. [VERIFIED: cargo metadata --no-deps] [VERIFIED: crates/es-core/src/lib.rs] |
| `es-runtime` | workspace crate 0.1.0 | Command routing, shard state, aggregate cache, dedupe cache | The bug lives in this crate and should be fixed here. [VERIFIED: cargo metadata --no-deps] [VERIFIED: crates/es-runtime/src/cache.rs] |
| `es-store-postgres` | workspace crate 0.1.0 | Durable append, tenant-scoped rehydration, durable replay | Runtime should continue delegating rehydration and replay to this boundary. [VERIFIED: crates/es-runtime/src/store.rs] [VERIFIED: crates/es-store-postgres/src/event_store.rs] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio` | resolved 1.52.1 | Async test/runtime execution | Keep existing `#[tokio::test]` runtime-flow tests. [VERIFIED: cargo info tokio --locked] |
| `futures` | 0.3.32 | `BoxFuture` in runtime store trait | Preserve the current `RuntimeEventStore` trait shape. [VERIFIED: cargo info futures --locked] [VERIFIED: crates/es-runtime/src/store.rs:1] |
| `metrics` | 0.24.3 | Existing runtime histograms/gauges | No new metrics are required unless the planner wants an optional cache-hit/miss tenant label later. [VERIFIED: cargo info metrics --locked] [VERIFIED: crates/es-runtime/src/shard.rs] |
| `sqlx` | 0.8.6 | PostgreSQL event-store implementation | No schema or SQL change is required for cache-key isolation because storage reads already filter by tenant. [VERIFIED: cargo info sqlx --locked] [VERIFIED: crates/es-store-postgres/src/sql.rs:510] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Composite typed key | Nested `HashMap<TenantId, HashMap<StreamId, A::State>>` | Nested maps add extra API complexity and do not improve correctness for the current cache operations. [ASSUMED] |
| Runtime cache key fix | Adapter-level tenant validation | Adapter validation cannot protect process-manager/runtime callers and does not fix the pre-rehydration cache hit. [VERIFIED: crates/es-runtime/src/shard.rs:218] |
| Runtime cache key fix | PostgreSQL schema/index changes | Storage is already tenant-scoped for stream reads, so DB changes do not address the in-memory cache identity bug. [VERIFIED: crates/es-store-postgres/src/sql.rs:510] |
| Existing `HashMap` | New cache crate | The phase needs identity correctness, not eviction, TTL, or concurrency features. [VERIFIED: .planning/ROADMAP.md] [ASSUMED] |

**Installation:** No new dependencies. [VERIFIED: Cargo.toml]

```bash
cargo test -p es-runtime -- --nocapture
```

**Version verification:** Existing dependency versions were verified with `cargo metadata --no-deps` and `cargo info --locked`; no package additions are recommended. [VERIFIED: cargo metadata --no-deps] [VERIFIED: cargo info tokio --locked] [VERIFIED: cargo info sqlx --locked]

## Architecture Patterns

### System Architecture Diagram

```text
CommandEnvelope
  |
  v
ShardState::process_next_handoff
  |
  +--> DedupeCache lookup by (tenant_id, idempotency_key)
  |      |
  |      +--> hit: replay stored reply, skip aggregate state
  |      |
  |      +--> miss
  |
  +--> RuntimeEventStore::lookup_command_replay(tenant_id, idempotency_key)
  |      |
  |      +--> hit: cache dedupe record, replay reply
  |      |
  |      +--> miss
  |
  +--> AggregateCache lookup by (tenant_id, stream_id)
         |
         +--> hit: decide against same tenant's cached state
         |
         +--> miss: RuntimeEventStore::load_rehydration(tenant_id, stream_id)
                    |
                    v
              decode snapshot/events -> fill AggregateCache[(tenant_id, stream_id)]
                    |
                    v
              decide -> append -> apply staged events -> commit same cache key
```

All arrows represent the runtime path that must remain tenant-scoped before any aggregate decision is made. [VERIFIED: crates/es-runtime/src/shard.rs]

### Recommended Project Structure

```text
crates/es-runtime/src/
├── cache.rs          # Add AggregateCacheKey and update AggregateCache API
├── shard.rs          # Construct and reuse AggregateCacheKey from envelope metadata + stream
├── lib.rs            # Re-export AggregateCacheKey if tests need it
└── tests/
    ├── shard_disruptor.rs  # Unit coverage for composite cache key behavior
    └── runtime_flow.rs     # Regression coverage for same stream across two tenants
```

This structure matches the current runtime crate layout and keeps the fix local. [VERIFIED: cargo metadata --no-deps] [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

### Pattern 1: Typed Composite Cache Key

**What:** Define a cache key struct with owned `TenantId` and `StreamId`, deriving `Clone`, `Debug`, `Eq`, `Hash`, and `PartialEq`. [CITED: https://doc.rust-lang.org/std/collections/struct.HashMap.html]

**When to use:** Use it for every aggregate state cache operation: default insert, lookup, rehydration fill, and post-commit state replacement. [VERIFIED: crates/es-runtime/src/cache.rs]

**Example:**

```rust
// Source: Rust HashMap docs require Eq + Hash keys; local DedupeKey already follows this pattern.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AggregateCacheKey {
    pub tenant_id: TenantId,
    pub stream_id: StreamId,
}
```

### Pattern 2: One Key Per Handoff

**What:** Construct the aggregate cache key once after duplicate replay misses and pass it through cache lookup, rehydration fill, and commit. [VERIFIED: crates/es-runtime/src/shard.rs:170] [VERIFIED: crates/es-runtime/src/shard.rs:218]

**When to use:** Use this in `process_next_handoff` so tenant identity cannot drift between lookup and commit. [VERIFIED: crates/es-runtime/src/shard.rs]

**Example:**

```rust
// Source: local process_next_handoff uses envelope.metadata.tenant_id and envelope.stream_id.
let cache_key = AggregateCacheKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    stream_id: envelope.stream_id.clone(),
};

let current_state = if let Some(cached) = self.cache.get(&cache_key) {
    cached.clone()
} else {
    let rehydrated = rehydrate_state(store, codec, &envelope).await?;
    self.cache.commit_state(cache_key.clone(), rehydrated.clone());
    rehydrated
};
```

### Pattern 3: Preserve Commit-Gated Mutation

**What:** Keep staged state local until `AppendOutcome::Committed`; only then apply decision events and replace the cache entry. [VERIFIED: crates/es-runtime/src/shard.rs:294]

**When to use:** Every successful non-duplicate command path. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

**Example:**

```rust
// Source: local shard.rs commit path, with key changed from stream_id to AggregateCacheKey.
let mut staged_state = current_state;
for event in &decision.events {
    A::apply(&mut staged_state, event);
}
self.cache.commit_state(cache_key, staged_state);
```

### Anti-Patterns to Avoid

- **Stream-only cache keys:** They can skip tenant-specific rehydration and reuse another tenant's aggregate state. [VERIFIED: crates/es-runtime/src/cache.rs:8] [VERIFIED: crates/es-runtime/src/shard.rs:218]
- **Global `Arc<Mutex<HashMap<...>>>` business-state cache:** It violates the shard-owned hot-state requirement. [VERIFIED: .planning/REQUIREMENTS.md] [ASSUMED]
- **Adapter-only mitigation:** It does not cover process-manager or direct runtime callers. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: crates/es-runtime/src/shard.rs]
- **Changing routing to hide cache bleed:** Routing already includes tenant identity; cache identity must still be correct because two tenants can share a shard. [VERIFIED: crates/es-runtime/src/router.rs:51] [VERIFIED: crates/es-runtime/tests/router_gateway.rs]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Composite key hashing/equality | Manual hash concatenation or delimited strings | Rust derived `Eq`/`Hash` on `AggregateCacheKey` | Rust docs support derived custom key traits, and typed IDs avoid delimiter/collision mistakes. [CITED: https://doc.rust-lang.org/std/collections/struct.HashMap.html] [VERIFIED: crates/es-runtime/src/cache.rs:52] |
| Tenant isolation at runtime | Bespoke tenant guard before every aggregate command | Cache-key identity plus existing tenant-scoped store calls | The failure mode is the stream-only cache lookup before rehydration. [VERIFIED: crates/es-runtime/src/shard.rs:218] |
| Durable tenant separation | In-memory cache as source of truth | Existing PostgreSQL rehydration and append contracts | Storage reads already filter by tenant and stream. [VERIFIED: crates/es-store-postgres/src/sql.rs:510] |
| Duplicate replay | Aggregate-cache-based idempotency | Existing `DedupeCache` and durable `lookup_command_replay` | Duplicate replay is already tenant-scoped separately from aggregate state. [VERIFIED: crates/es-runtime/src/cache.rs:51] [VERIFIED: crates/es-runtime/src/store.rs:18] |

**Key insight:** The aggregate cache is an optimization over authoritative tenant-scoped storage, so its key must be at least as specific as the storage query it can skip. [VERIFIED: crates/es-runtime/src/store.rs:11] [VERIFIED: crates/es-runtime/src/shard.rs:218]

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | PostgreSQL event-store reads are already tenant-scoped by `WHERE tenant_id = $1 AND stream_id = $2`. [VERIFIED: crates/es-store-postgres/src/sql.rs:510] | No data migration for Phase 9. [VERIFIED: local SQL audit] |
| Live service config | No phase-specific live service config was found in the phase scope. [VERIFIED: .planning/ROADMAP.md] | None. [VERIFIED: local phase audit] |
| OS-registered state | No OS-level registrations are involved in shard-local in-process cache identity. [VERIFIED: .planning/ROADMAP.md] | None. [VERIFIED: local phase audit] |
| Secrets/env vars | No secret or environment variable names participate in aggregate cache keys. [VERIFIED: crates/es-runtime/src/cache.rs] | None. [VERIFIED: local code audit] |
| Build artifacts | Rust build artifacts may contain old compiled code until rebuild, but Cargo tests rebuild changed crates. [VERIFIED: cargo test -p es-runtime -- --nocapture] | Run targeted runtime tests after edits. [VERIFIED: cargo test -p es-runtime -- --nocapture] |

## Common Pitfalls

### Pitfall 1: Fixing Rehydration But Not Cache Lookup

**What goes wrong:** Runtime still checks a stream-only cache entry and never calls tenant-scoped rehydration. [VERIFIED: crates/es-runtime/src/shard.rs:218]
**Why it happens:** The storage trait already accepts tenant ID, so the missing tenant identity is easy to overlook in the cache layer. [VERIFIED: crates/es-runtime/src/store.rs:11]
**How to avoid:** Make `AggregateCache::get` impossible to call with only `StreamId`. [VERIFIED: crates/es-runtime/src/cache.rs]
**Warning signs:** Any remaining `cache.get(&envelope.stream_id)` or `commit_state(envelope.stream_id.clone(), ...)` call. [VERIFIED: crates/es-runtime/src/shard.rs:218] [VERIFIED: crates/es-runtime/src/shard.rs:308]

### Pitfall 2: Updating Commit Path But Not Rehydration Fill

**What goes wrong:** Cache misses rehydrate tenant-scoped state but store it under a different identity than later commits or reads. [ASSUMED]
**Why it happens:** The current code fills cache on miss and commits after append in two separate blocks. [VERIFIED: crates/es-runtime/src/shard.rs:221] [VERIFIED: crates/es-runtime/src/shard.rs:308]
**How to avoid:** Construct one `AggregateCacheKey` and clone it for every cache operation in the handoff. [VERIFIED: crates/es-runtime/src/shard.rs]
**Warning signs:** Multiple ad hoc `AggregateCacheKey { ... }` constructions inside one handoff. [ASSUMED]

### Pitfall 3: Regressing Duplicate Replay Order

**What goes wrong:** Duplicate commands may rehydrate or decide before replay lookup, reintroducing Phase 8 bugs. [VERIFIED: .planning/STATE.md]
**Why it happens:** Cache refactors can accidentally move aggregate-cache lookup above dedupe lookup. [ASSUMED]
**How to avoid:** Preserve the sequence: shard-local dedupe, durable replay, aggregate cache/rehydration, decide, append. [VERIFIED: crates/es-runtime/src/shard.rs]
**Warning signs:** `load_rehydration` is called during `runtime_duplicate_cache_hit_skips_decide_and_append` or `runtime_duplicate_store_hit_skips_rehydrate_decide_encode_and_append`. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

### Pitfall 4: Assuming Tenant-Aware Routing Is Sufficient

**What goes wrong:** Two tenants can still land on the same local shard, so shard-owned state must isolate tenant identities internally. [VERIFIED: crates/es-runtime/src/router.rs] [ASSUMED]
**Why it happens:** Routing includes tenant ID, but shard selection is modulo shard count and does not create one shard per tenant. [VERIFIED: crates/es-runtime/src/router.rs:56]
**How to avoid:** Treat routing and cache identity as separate isolation layers. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
**Warning signs:** Tests only assert different route outputs for one sample tenant pair. [VERIFIED: crates/es-runtime/tests/router_gateway.rs:103]

## Code Examples

### Aggregate Cache API Shape

```rust
// Source: local DedupeKey pattern + Rust HashMap custom key guidance.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AggregateCacheKey {
    pub tenant_id: TenantId,
    pub stream_id: StreamId,
}

pub struct AggregateCache<A: Aggregate> {
    states: HashMap<AggregateCacheKey, A::State>,
}
```

### Same-Stream Different-Tenant Regression Shape

```rust
// Source: crates/es-runtime/tests/runtime_flow.rs fake aggregate/store style.
// Test intent: tenant-a warms "counter-1" to value 8; tenant-b with the same
// stream id rehydrates from tenant-b state and replies from tenant-b value.
assert_eq!(8, tenant_a_outcome.reply);
assert_eq!(3, tenant_b_outcome.reply);
assert_eq!(2, state.cache().len());
```

### Store Fake Should Record Rehydration Inputs

```rust
// Source: RuntimeEventStore::load_rehydration signature.
fn load_rehydration(
    &self,
    tenant_id: &TenantId,
    stream_id: &StreamId,
) -> BoxFuture<'_, StoreResult<RehydrationBatch>>;
```

Record the `(tenant_id, stream_id)` pairs in the fake store so the test proves tenant B rehydration was called despite tenant A's same-stream cached state. [VERIFIED: crates/es-runtime/src/store.rs:11]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Stream-only aggregate cache | Tenant+stream aggregate cache | Required by Phase 9 after the 2026-04-20 milestone audit | Prevents tenant cache bleed while preserving shard-local ownership. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| Runtime duplicate replay after/around decision | Pre-decision shard-local and durable replay lookup | Phase 8 completed 2026-04-19 | Phase 9 must preserve the replay-before-rehydration order. [VERIFIED: .planning/STATE.md] |
| Storage-only tenant scoping | Storage plus runtime cache tenant scoping | Phase 9 target | A cache can bypass storage, so cache identity must match tenant-scoped storage identity. [VERIFIED: crates/es-runtime/src/shard.rs:218] |

**Deprecated/outdated:**

- Stream-only `AggregateCache` APIs are outdated for the current tenant-scoped runtime/store design. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- Tests that warm a cache with only `StreamId` are outdated and should be updated to warm by `AggregateCacheKey`. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs:431]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Nested maps add extra API complexity and do not improve correctness for the current cache operations. | Standard Stack | Planner might prefer nested maps for future tenant-level eviction. |
| A2 | The phase needs identity correctness, not eviction, TTL, or concurrency features. | Standard Stack | If eviction is required, a cache crate or policy work would be needed. |
| A3 | Global `Arc<Mutex<HashMap<...>>>` would violate shard-owned hot-state intent. | Anti-Patterns | If runtime ownership changes later, locking guidance would need revision. |
| A4 | Multiple ad hoc key constructions inside one handoff are a warning sign. | Pitfalls | Planner may still produce correct code if constructions are identical, but review burden increases. |
| A5 | Cache refactors can accidentally move aggregate-cache lookup above dedupe lookup. | Pitfalls | If planner preserves order explicitly, this risk is mitigated. |
| A6 | Two tenants can still land on the same local shard under modulo routing. | Pitfalls | Exact collisions depend on shard count and hash values; tests can force one shard to prove isolation. |

## Open Questions

1. **Should `AggregateCacheKey` be public?** [VERIFIED: crates/es-runtime/src/lib.rs]
   - What we know: tests currently import `AggregateCache`, `DedupeKey`, and `ShardState` from the crate API. [VERIFIED: crates/es-runtime/tests/shard_disruptor.rs]
   - What's unclear: whether downstream crates should construct aggregate cache keys directly. [ASSUMED]
   - Recommendation: Re-export `AggregateCacheKey` if integration tests or documented runtime extension points need it; otherwise keep construction methods on `AggregateCache`. [ASSUMED]

2. **Should `get_or_default` survive?** [VERIFIED: crates/es-runtime/src/cache.rs:25]
   - What we know: runtime command flow uses explicit `get` plus rehydration, while tests use `get_or_default`. [VERIFIED: crates/es-runtime/src/shard.rs:218] [VERIFIED: crates/es-runtime/tests/shard_disruptor.rs]
   - What's unclear: whether any future runtime path should insert default aggregate state without storage rehydration. [ASSUMED]
   - Recommendation: Keep it only if tests need direct cache unit coverage, but require an `AggregateCacheKey` argument. [VERIFIED: crates/es-runtime/src/cache.rs]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | Build and tests | yes | rustc 1.85.1 | None needed. [VERIFIED: rustc --version] |
| Cargo | Test execution | yes | cargo 1.85.1 | None needed. [VERIFIED: cargo --version] |
| `es-runtime` test suite | Regression validation | yes | workspace 0.1.0 | None needed. [VERIFIED: cargo metadata --no-deps] |
| PostgreSQL/Testcontainers | Optional store integration confirmation | not required for core Phase 9 tests | Existing test stack present | Use no-DB runtime fake tests for phase gate; run DB tests only if store code changes. [VERIFIED: Cargo.toml] |

**Missing dependencies with no fallback:** None for this phase. [VERIFIED: cargo test -p es-runtime -- --nocapture]

**Missing dependencies with fallback:** PostgreSQL is not required for the core runtime-cache regression because the fake `RuntimeEventStore` can prove rehydration inputs. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Cargo test with Rust `#[test]` and `#[tokio::test]`. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| Config file | Root `Cargo.toml`; workspace lints are inherited. [VERIFIED: Cargo.toml] |
| Quick run command | `cargo test -p es-runtime shard_cache -- --nocapture` [VERIFIED: executed 2026-04-20] |
| Full suite command | `cargo test -p es-runtime -- --nocapture` [VERIFIED: executed 2026-04-20] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| STORE-04 | Same-stream tenant B calls tenant-scoped rehydration after tenant A warmed cache. [VERIFIED: phase criteria] | unit/integration fake store | `cargo test -p es-runtime same_stream_different_tenant_rehydrates_independently -- --nocapture` | Missing; add in Wave 0. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| RUNTIME-03 | Aggregate cache stores two entries for same stream across two tenants. [VERIFIED: phase criteria] | unit | `cargo test -p es-runtime shard_cache -- --nocapture` | Existing file, update tests. [VERIFIED: crates/es-runtime/tests/shard_disruptor.rs] |
| RUNTIME-05 | Successful reply still follows durable append and commits only after append. [VERIFIED: .planning/REQUIREMENTS.md] | regression | `cargo test -p es-runtime reply_is_sent_after_append_commit -- --nocapture` | Exists. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| RUNTIME-06 | Conflict does not mutate the tenant-scoped cache entry. [VERIFIED: .planning/REQUIREMENTS.md] | regression | `cargo test -p es-runtime conflict_does_not_mutate_cache -- --nocapture` | Exists, update for composite key. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| DOM-05 | Runtime does not evaluate tenant B against tenant A's state. [VERIFIED: phase criteria] | regression | `cargo test -p es-runtime same_stream_different_tenant_preserves_domain_state -- --nocapture` | Missing; add in Wave 0. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |

### Sampling Rate

- **Per task commit:** `cargo test -p es-runtime shard_cache -- --nocapture` [VERIFIED: executed 2026-04-20]
- **Per wave merge:** `cargo test -p es-runtime -- --nocapture` [VERIFIED: executed 2026-04-20]
- **Phase gate:** `cargo test --workspace` if only runtime tests pass and no DB/store code changed; DB integration only if storage APIs are edited. [ASSUMED]

### Wave 0 Gaps

- [ ] `crates/es-runtime/tests/shard_disruptor.rs` - update `shard_cache_inserts_default_state_locally` and `shard_cache_commits_only_explicit_state` to use tenant+stream keys. [VERIFIED: crates/es-runtime/tests/shard_disruptor.rs]
- [ ] `crates/es-runtime/tests/runtime_flow.rs` - add same-stream/different-tenant regression proving tenant B rehydrates independently and cache length is 2. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]
- [ ] `FakeStore` in `runtime_flow.rs` - record `load_rehydration` calls by `(TenantId, StreamId)` and support tenant-specific batches. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 9 does not authenticate callers. [VERIFIED: .planning/ROADMAP.md] |
| V3 Session Management | no | Phase 9 does not handle sessions. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | yes | Tenant identity must be part of in-memory aggregate cache identity. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| V5 Input Validation | yes | Continue using typed `TenantId` and `StreamId` constructors instead of raw strings. [VERIFIED: crates/es-core/src/lib.rs] |
| V6 Cryptography | no | Phase 9 does not introduce cryptographic behavior. [VERIFIED: .planning/ROADMAP.md] |

### Known Threat Patterns for Runtime Cache Tenant Isolation

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant state bleed through stream-only in-memory cache | Information Disclosure / Tampering | Composite tenant+stream aggregate cache key. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| Tenant-scoped storage bypass due to cache hit | Tampering | Cache key specificity must match `load_rehydration(tenant_id, stream_id)`. [VERIFIED: crates/es-runtime/src/store.rs:11] |
| Duplicate replay confused with aggregate state caching | Repudiation / Tampering | Preserve separate `DedupeKey { tenant_id, idempotency_key }` and aggregate cache key. [VERIFIED: crates/es-runtime/src/cache.rs:51] |

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` - phase requirement IDs and architectural constraints. [VERIFIED: local file read]
- `.planning/STATE.md` - prior decisions and Phase 8 replay ordering context. [VERIFIED: local file read]
- `.planning/ROADMAP.md` - Phase 9 scope, dependencies, and success criteria. [VERIFIED: local file read]
- `.planning/v1.0-MILESTONE-AUDIT.md` - current correctness gap evidence. [VERIFIED: local file read]
- `crates/es-runtime/src/cache.rs` - current aggregate and dedupe cache implementation. [VERIFIED: local file read]
- `crates/es-runtime/src/shard.rs` - command processing order and cache access path. [VERIFIED: local file read]
- `crates/es-runtime/src/router.rs` - tenant-aware routing implementation. [VERIFIED: local file read]
- `crates/es-runtime/src/store.rs` - runtime store trait for tenant-scoped rehydration and replay. [VERIFIED: local file read]
- `crates/es-store-postgres/src/sql.rs` - tenant-scoped stream read SQL. [VERIFIED: local file read]
- `cargo test -p es-runtime -- --nocapture` - current runtime suite passes before the Phase 9 fix. [VERIFIED: command executed 2026-04-20]
- Rust std `HashMap` docs - custom key requirements and derived `Eq`/`Hash` guidance. [CITED: https://doc.rust-lang.org/std/collections/struct.HashMap.html]

### Secondary (MEDIUM confidence)

- `cargo info tokio --locked`, `cargo info futures --locked`, `cargo info metrics --locked`, `cargo info sqlx --locked` - resolved dependency metadata relevant to existing tests/runtime boundaries. [VERIFIED: cargo info commands]

### Tertiary (LOW confidence)

- None. [VERIFIED: all recommendations are from local code or official Rust docs]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH - phase uses existing workspace crates and Rust std `HashMap`; no new crate is needed. [VERIFIED: Cargo.toml] [CITED: https://doc.rust-lang.org/std/collections/struct.HashMap.html]
- Architecture: HIGH - local code directly shows the current stream-only cache bug and existing tenant-scoped storage/replay boundaries. [VERIFIED: crates/es-runtime/src/cache.rs] [VERIFIED: crates/es-runtime/src/store.rs]
- Pitfalls: HIGH - pitfalls map to exact current call sites and existing Phase 8 tests. [VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

**Research date:** 2026-04-20 [VERIFIED: system date]
**Valid until:** 2026-05-20, assuming no runtime ownership redesign before Phase 9 execution. [ASSUMED]

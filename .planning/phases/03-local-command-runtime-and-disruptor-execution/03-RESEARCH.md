# Phase 3: Local Command Runtime and Disruptor Execution - Research

**Researched:** 2026-04-17  
**Domain:** Rust local command runtime, bounded ingress, partition routing, shard-local aggregate execution, and `disruptor` 4.0.0 integration  
**Confidence:** HIGH for Rust/Tokio/storage boundaries; MEDIUM for exact `disruptor` topology until implementation spike verifies closure and async-bridge ergonomics

## User Constraints

- Phase 3 must satisfy RUNTIME-01 through RUNTIME-06. [VERIFIED: .planning/REQUIREMENTS.md]
- Requests must enter a bounded local command engine, route by aggregate or partition key to a single shard owner, execute through an in-process disruptor path, and reply only after event-store commit. [VERIFIED: .planning/ROADMAP.md]
- This is a Rust-first template; event store commit is the authoritative command success point. [VERIFIED: user prompt] [VERIFIED: .planning/PROJECT.md]
- `disruptor-rs`/the `disruptor` crate is in-process execution fabric only, not durability, not a broker, and not distributed ownership. [VERIFIED: user prompt] [VERIFIED: .planning/PROJECT.md]
- The same aggregate or ordered partition key must map to the same local shard owner under stable partition configuration. [VERIFIED: .planning/REQUIREMENTS.md]
- Hot business state should stay shard-local; do not use global `Arc<Mutex<_>>` business-state maps. [VERIFIED: user prompt] [VERIFIED: .planning/REQUIREMENTS.md]
- Phase 2 already delivered PostgreSQL append/OCC/dedupe/snapshot/global read behavior through `es-store-postgres`. [VERIFIED: .planning/STATE.md] [VERIFIED: crates/es-store-postgres/src/lib.rs]
- Distributed partition ownership and failover are deferred to v2/out of scope. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/STATE.md]
- Prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: user-provided AGENTS.md instructions]

## Summary

Phase 3 should build `es-runtime` as the sole owner of local command admission, partition routing, shard lifecycle, shard-local cache, disruptor publication, aggregate replay/decide/apply orchestration, and reply delivery. [VERIFIED: .planning/ROADMAP.md] The runtime should depend on `es-core`, `es-kernel`, and `es-store-postgres`, while adapters remain thin clients of a `CommandGateway` API. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: crates/es-kernel/src/lib.rs] [VERIFIED: crates/es-store-postgres/src/lib.rs]

Use two bounded layers: adapter-facing ingress with `tokio::sync::mpsc::Sender::try_send`, and per-shard disruptor publication with `Producer::try_publish`. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html] Blocking `send`/`publish` can hide overload and violate RUNTIME-01 because `disruptor::Producer::publish` spins when the ring is full. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html]

The shard runtime should mutate aggregate cache only after durable append succeeds. [VERIFIED: RUNTIME-05 in .planning/REQUIREMENTS.md] If `PostgresEventStore::append` returns `StoreError::StreamConflict`, preserve the existing cached state and map the error to a typed runtime conflict/retryable result. [VERIFIED: crates/es-store-postgres/src/error.rs] [VERIFIED: RUNTIME-06 in .planning/REQUIREMENTS.md]

**Primary recommendation:** Implement a generic `CommandGateway<A: Aggregate>` with bounded Tokio ingress, deterministic tenant-aware routing, one single-owner shard task per shard, `disruptor::build_single_producer(...).handle_events_and_state_with(...)` inside each shard, and replies sent via one-shot channels only after `PostgresEventStore::append` returns `AppendOutcome`. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] [VERIFIED: crates/es-store-postgres/src/event_store.rs]

## Project Constraints (from CLAUDE.md)

No `./CLAUDE.md` file exists in the repository. [VERIFIED: `test -f CLAUDE.md`] Apply the user-provided AGENTS/project instructions instead: prefer `pnpm` and `uv`; preserve GSD workflow artifacts; keep event-store source-of-truth, partition stability, shard-local hot state, outbox durability, separable runtime/adapters/projection/outbox, and separated performance tests. [VERIFIED: user-provided AGENTS.md instructions]

No project-local skills were found in `.claude/skills/` or `.agents/skills/`. [VERIFIED: `find .claude/skills .agents/skills -maxdepth 2 -name SKILL.md`]

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RUNTIME-01 | Adapter requests enter bounded ingress with explicit overload behavior. [VERIFIED: .planning/REQUIREMENTS.md] | Use bounded Tokio mpsc plus `try_send`; map `Full` to `RuntimeError::Overloaded`. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| RUNTIME-02 | Partition routing sends all commands for the same aggregate key to the same local shard owner. [VERIFIED: .planning/REQUIREMENTS.md] | Use a fixed-seed stable hash over tenant + partition key modulo configured shard count. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: cargo info twox-hash] |
| RUNTIME-03 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. [VERIFIED: .planning/REQUIREMENTS.md] | Keep `HashMap`/`moka::sync::Cache` inside `ShardState`, passed to the disruptor processor through `handle_events_and_state_with`. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] [CITED: https://docs.rs/moka/] |
| RUNTIME-04 | Shard runtime integrates the `disruptor` crate as local execution/fan-out mechanism. [VERIFIED: .planning/REQUIREMENTS.md] | Use `disruptor` 4.0.0 published 2026-03-09; do not use the older literal `disruptor-rs` 0.1.1 crate. [VERIFIED: cargo search/cargo info disruptor] |
| RUNTIME-05 | Replies are sent only after durable event-store append commit succeeds. [VERIFIED: .planning/REQUIREMENTS.md] | Call `PostgresEventStore::append(AppendRequest)` before replying; reply includes committed stream/global positions from `CommittedAppend`. [VERIFIED: crates/es-store-postgres/src/models.rs] |
| RUNTIME-06 | OCC conflicts surface as typed retryable/conflict errors without corrupting shard-local cache. [VERIFIED: .planning/REQUIREMENTS.md] | On `StoreError::StreamConflict`, leave cached state untouched and return typed conflict/retryable runtime error. [VERIFIED: crates/es-store-postgres/src/error.rs] |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Bounded command ingress | Runtime / API boundary | Adapter | Runtime owns capacity and overload semantics; adapters later call runtime instead of owning queues. [VERIFIED: API-01 and RUNTIME-01 in .planning/REQUIREMENTS.md] |
| Partition routing | Runtime | Core types | `PartitionKey` is a core value, but shard selection and ownership are runtime behavior. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: crates/es-runtime/src/lib.rs] |
| Shard ownership and hot cache | Runtime | Storage for cold rehydration | Shard owns mutable aggregate state; storage supplies snapshot/events for rebuild. [VERIFIED: RUNTIME-03 in .planning/REQUIREMENTS.md] [VERIFIED: crates/es-store-postgres/src/event_store.rs] |
| Aggregate decision and replay | Domain kernel | Runtime orchestration | `Aggregate::decide/apply` are synchronous kernel contracts; runtime calls them around storage reads/appends. [VERIFIED: crates/es-kernel/src/lib.rs] |
| Durable append and dedupe | Storage | Runtime error mapping | PostgreSQL store is the source-of-truth commit boundary; runtime maps outcomes to replies. [VERIFIED: crates/es-store-postgres/src/lib.rs] |
| Disruptor sequencing | Runtime | - | `disruptor` is in-process inter-thread communication and should stay inside shard runtime. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] |
| Distributed ownership/failover | Out of scope | - | v2 requirements explicitly defer distributed partition ownership. [VERIFIED: .planning/REQUIREMENTS.md] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust | 1.85.1 local; workspace floor 1.85 | Rust 2024 service implementation | Workspace already pins Rust 2024 with `rust-version = "1.85"`. [VERIFIED: Cargo.toml] [VERIFIED: `rustc --version`] |
| `tokio` | 1.52.1 current; workspace has 1.52.0 | Async runtime, bounded ingress, one-shot replies, shard task lifecycle | Tokio bounded mpsc provides backpressure and `try_send` for explicit overload. [VERIFIED: crates.io API] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| `disruptor` | 4.0.0, published 2026-03-09 | Per-shard in-process ring sequencing and processor-local state | Current maintained crate from `nicholassm/disruptor-rs`; supports single/multi producer, managed threads, event poller, `try_publish`, and processor state. [VERIFIED: cargo info disruptor] [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] |
| `es-core` | workspace local | Tenant, stream, partition, metadata, expected revision types | Already defines `PartitionKey`, `StreamId`, `TenantId`, `CommandMetadata`, and `ExpectedRevision`. [VERIFIED: crates/es-core/src/lib.rs] |
| `es-kernel` | workspace local | Typed deterministic aggregate contract | Already defines synchronous `Aggregate`, `Decision`, and replay. [VERIFIED: crates/es-kernel/src/lib.rs] |
| `es-store-postgres` | workspace local | Durable append, dedupe, snapshot rehydration, global reads | Phase 2 storage is complete and exposes async append/rehydration APIs. [VERIFIED: crates/es-store-postgres/src/lib.rs] |
| `thiserror` | 2.0.18, published 2026-01-18 | Public runtime error enums | Project already uses typed `thiserror` errors in core/storage; runtime should follow that pattern. [VERIFIED: Cargo.toml] [VERIFIED: cargo info thiserror] |
| `tracing` | 0.1.44, published 2025-12-18 | Runtime spans and event fields | Observability requirements need command/shard/global-position span fields; `tracing` is already the project-standard stack. [VERIFIED: .planning/research/STACK.md] [VERIFIED: cargo info tracing] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `twox-hash` | 2.1.2, published 2025-09-03 | Fast deterministic non-cryptographic hash for routing | Use with a fixed seed for shard index calculation. [VERIFIED: cargo info twox-hash] |
| `moka` | 0.12.15, published 2026-03-22 | Optional bounded shard-local cache with eviction/TTL | Use only inside a shard-owned state object if Phase 3 needs bounded cache eviction. [VERIFIED: cargo info moka] [CITED: https://docs.rs/moka/] |
| `crossbeam-channel` | 0.5.15, published 2025-04-08 | Sync bounded channels | Use only if a sync thread boundary is easier than Tokio for a shard worker; prefer Tokio at async ingress. [VERIFIED: cargo info crossbeam-channel] |
| `futures` | 0.3.32, published 2026-02-15 | `BoxFuture` and trait object async escape hatch | Use only if the runtime must erase async storage calls behind traits. [VERIFIED: cargo info futures] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tokio::sync::mpsc` bounded ingress | `crossbeam-channel::bounded` | Crossbeam is good for sync workers, but adapter-facing callers are async and Tokio docs provide async backpressure/`try_send`. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| `disruptor` 4.0.0 | literal `disruptor-rs` 0.1.1 | `disruptor` is current and documented; `disruptor-rs` is a separate older crate by version signal. [VERIFIED: cargo search disruptor] |
| Fixed-seed `twox-hash` routing | `std::collections::hash_map::DefaultHasher` | Default hasher is not a routing contract; use an explicit algorithm/seed so partition mapping is intentionally stable. [VERIFIED: RUNTIME-02 in .planning/REQUIREMENTS.md] [ASSUMED] |
| Shard-local `HashMap` | Global `Arc<Mutex<HashMap<...>>>` | Global locks violate RUNTIME-03 and serialize unrelated hot keys. [VERIFIED: RUNTIME-03 in .planning/REQUIREMENTS.md] |

**Installation:**

```bash
cargo add disruptor@4.0.0 tokio@1.52.1 thiserror@2.0.18 tracing@0.1.44 twox-hash@2.1.2 moka@0.12.15 --workspace
```

Apply manually to the workspace catalog to preserve local style; add `tokio` feature `sync` because current workspace features are `rt-multi-thread`, `macros`, and `time`. [VERIFIED: Cargo.toml] Add `moka` only if bounded eviction is implemented in Phase 3; otherwise use a shard-local `HashMap` first. [CITED: https://docs.rs/moka/]

**Version verification:** `cargo info` and crates.io API verified current versions and publish dates on 2026-04-17. [VERIFIED: cargo info] [VERIFIED: crates.io API]

## Architecture Patterns

### System Architecture Diagram

```text
Adapter-facing caller
  -> CommandGateway::try_submit(command, metadata, reply_timeout)
  -> bounded Tokio ingress try_send
     -> Full: immediate RuntimeError::Overloaded
     -> Closed: RuntimeError::Unavailable
     -> Accepted:
        -> router computes shard_id = stable_hash(tenant_id, partition_key) % shard_count
        -> per-shard mailbox / command pump
        -> disruptor Producer::try_publish(command envelope)
           -> RingBufferFull: reply RuntimeError::ShardOverloaded
           -> Published:
              -> shard-owned disruptor processor with ShardState
                 -> load cached aggregate or rehydrate from Postgres snapshot + stream events
                 -> Aggregate::decide(state, command, metadata)
                    -> Domain error: reply without append, cache unchanged
                    -> Decision events:
                       -> create NewEvent DTOs and AppendRequest
                       -> PostgresEventStore::append
                          -> Committed/Duplicate: apply events to cache, reply success
                          -> StreamConflict: cache unchanged, reply conflict/retryable
                          -> Database error: cache unchanged, reply infrastructure error
```

### Recommended Project Structure

```text
crates/es-runtime/src/
|-- lib.rs              # public facade and re-exports
|-- error.rs            # RuntimeError / RuntimeResult
|-- command.rs          # CommandEnvelope, CommandReply, CommandOutcome
|-- gateway.rs          # adapter-facing CommandGateway and bounded ingress
|-- router.rs           # stable tenant-aware partition routing
|-- shard.rs            # shard lifecycle, ShardHandle, ShardState
|-- disruptor_path.rs   # narrow wrapper around disruptor Producer/API
|-- cache.rs            # shard-local cache types and invalidation helpers
|-- store.rs            # runtime-facing trait/adapters for PostgresEventStore
`-- tests/ or ../tests/ # routing, overload, conflict, reply-after-commit tests
```

### Pattern 1: Bounded Gateway With Explicit Overload

**What:** Create a bounded Tokio mpsc ingress and use `try_send` in the adapter-facing submission path. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html]

**When to use:** Every external caller path that submits a command to the runtime. [VERIFIED: RUNTIME-01 in .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html
pub fn try_submit(&self, envelope: CommandEnvelope<A>) -> RuntimeResult<()> {
    self.ingress
        .try_send(envelope)
        .map_err(|error| match error {
            tokio::sync::mpsc::error::TrySendError::Full(_) => RuntimeError::Overloaded,
            tokio::sync::mpsc::error::TrySendError::Closed(_) => RuntimeError::Unavailable,
        })
}
```

### Pattern 2: Stable Tenant-Aware Routing

**What:** Route on `tenant_id + partition_key` with an explicit fixed-seed hash and configured shard count. [VERIFIED: RUNTIME-02 in .planning/REQUIREMENTS.md]

**When to use:** Before a command enters a shard-specific queue/ring. [VERIFIED: .planning/ROADMAP.md]

**Example:**

```rust
// Source: twox-hash crate metadata verified by cargo info; routing rule from RUNTIME-02.
pub fn shard_for(tenant: &TenantId, key: &PartitionKey, shard_count: usize) -> RuntimeResult<ShardId> {
    if shard_count == 0 {
        return Err(RuntimeError::InvalidShardCount);
    }
    let mut hasher = twox_hash::XxHash64::with_seed(0x4553_5255_4e54494d);
    use std::hash::Hasher;
    hasher.write(tenant.as_str().as_bytes());
    hasher.write_u8(0);
    hasher.write(key.as_str().as_bytes());
    Ok(ShardId((hasher.finish() as usize) % shard_count))
}
```

### Pattern 3: Disruptor Processor Owns Shard State

**What:** Use `handle_events_and_state_with` so the processor receives `&mut ShardState` for cache mutation, avoiding global locks. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/]

**When to use:** Per shard, where one processor should own aggregate cache and dedupe cache. [VERIFIED: RUNTIME-03 in .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: https://docs.rs/disruptor/4.0.0/disruptor/
let initial_state = || ShardState::<A>::new(shard_id);
let processor = |state: &mut ShardState<A>, envelope: &RuntimeEvent<A>, _seq, _end| {
    state.process(envelope);
};

let mut producer = disruptor::build_single_producer(ring_size, RuntimeEvent::<A>::empty, disruptor::BusySpinWithSpinLoopHint)
    .handle_events_and_state_with(processor, initial_state)
    .build();
```

### Pattern 4: Reply After Commit, Cache After Commit

**What:** Decide against current state, append produced events, then update cache and reply only on committed/deduped durable outcome. [VERIFIED: RUNTIME-05 in .planning/REQUIREMENTS.md]

**When to use:** Every successful command path. [VERIFIED: .planning/ROADMAP.md]

**Example:**

```rust
// Source: local storage facade and kernel contract.
let decision = A::decide(&state, command, &metadata).map_err(RuntimeError::Domain)?;
let append = AppendRequest::new(stream_id, expected_revision, metadata, idempotency_key, events)?;

match store.append(append).await {
    Ok(AppendOutcome::Committed(committed) | AppendOutcome::Duplicate(committed)) => {
        for event in &decision.events {
            A::apply(&mut cached_state, event);
        }
        reply.send(Ok(CommandOutcome::Committed { reply: decision.reply, committed })).ok();
    }
    Err(StoreError::StreamConflict { .. }) => {
        reply.send(Err(RuntimeError::Conflict)).ok();
    }
    Err(error) => {
        reply.send(Err(RuntimeError::Store(error))).ok();
    }
}
```

### Anti-Patterns to Avoid

- **Blocking `publish` for overload-sensitive command ingress:** `Producer::publish` spins while full; use `try_publish` and return typed overload. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html]
- **Treating ring sequence as event/global position:** Global position is assigned by PostgreSQL storage, not disruptor. [VERIFIED: crates/es-store-postgres/src/models.rs]
- **Updating cache before append:** A failed append or OCC conflict would leave hot state ahead of durable truth. [VERIFIED: RUNTIME-05 and RUNTIME-06 in .planning/REQUIREMENTS.md]
- **Global aggregate map behind `Arc<Mutex<_>>`:** Violates shard-local ownership and hides cross-key contention. [VERIFIED: RUNTIME-03 in .planning/REQUIREMENTS.md]
- **Randomized or implicit hash routing:** Partition stability is a requirement; use an explicit algorithm/seed. [VERIFIED: RUNTIME-02 in .planning/REQUIREMENTS.md] [ASSUMED]
- **Async/network work inside deterministic aggregate `decide`:** Kernel contract is synchronous and storage/adapter-free. [VERIFIED: crates/es-kernel/src/lib.rs]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async bounded ingress | Custom queue/semaphore | `tokio::sync::mpsc::channel` + `try_send` | Tokio documents bounded channels as backpressure primitives. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| Ring sequencing and consumer dependency machinery | Custom lock-free ring buffer | `disruptor` 4.0.0 | The crate already provides producers, ring buffer, wait strategies, managed consumers, stateful processors, and event poller. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] |
| Durable command dedupe | Runtime-only dedupe map | `PostgresEventStore::append` dedupe result | Phase 2 stores exact committed append response in durable command dedupe. [VERIFIED: .planning/STATE.md] [VERIFIED: crates/es-store-postgres/tests/dedupe.rs] |
| Aggregate replay | Runtime-specific replay logic per domain | `es-kernel::Aggregate::apply` over storage rehydration batch | Kernel owns deterministic apply; storage returns snapshots/events. [VERIFIED: crates/es-kernel/src/lib.rs] [VERIFIED: crates/es-store-postgres/src/models.rs] |
| Public error formatting | Stringly errors | `thiserror` runtime enum | Existing crates expose typed errors with `thiserror`. [VERIFIED: crates/es-store-postgres/src/error.rs] |
| Cache eviction policy | Custom LRU/LFU | `moka::sync::Cache` when eviction is needed | Moka already supports max capacity, TTL/TTI, and eviction maintenance. [CITED: https://docs.rs/moka/] |

**Key insight:** The hard part is not the ring buffer; it is preserving the invariant that runtime state is a cache of durable committed events, never a competing source of truth. [VERIFIED: .planning/PROJECT.md]

## Common Pitfalls

### Pitfall 1: Blocking Instead of Overloading
**What goes wrong:** Callers pile up because `send().await` or `publish()` waits instead of returning overload. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html]  
**Why it happens:** Backpressure APIs are easy to use in blocking mode. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html]  
**How to avoid:** Use `try_send` at ingress and `try_publish` at shard ring publication. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html] [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html]  
**Warning signs:** No test forces a full ingress/ring and asserts typed overload. [VERIFIED: current `rg --files` test scan]

### Pitfall 2: Cache Ahead of Storage
**What goes wrong:** Domain state mutates in memory before append, then storage conflict/error leaves cache inconsistent. [VERIFIED: RUNTIME-05 and RUNTIME-06 in .planning/REQUIREMENTS.md]  
**Why it happens:** Developers apply events immediately after `decide` instead of after durable append. [VERIFIED: crates/es-kernel/src/lib.rs]  
**How to avoid:** Clone or stage new state; commit cache mutation only after `AppendOutcome::Committed` or valid durable duplicate. [VERIFIED: crates/es-store-postgres/src/models.rs]  
**Warning signs:** Tests simulate `StoreError::StreamConflict` but cached state changes. [VERIFIED: crates/es-store-postgres/src/error.rs]

### Pitfall 3: Partition Route Drift
**What goes wrong:** Same key maps to different shards after restart, version change, or shard-count change. [VERIFIED: RUNTIME-02 in .planning/REQUIREMENTS.md]  
**Why it happens:** Routing uses implicit/randomized hashers or changes shard count without migration policy. [ASSUMED]  
**How to avoid:** Make the hash algorithm, seed, and shard count explicit config; test golden key-to-shard mappings. [VERIFIED: RUNTIME-02 in .planning/REQUIREMENTS.md]  
**Warning signs:** Router tests only assert range, not stable mapping for known keys. [ASSUMED]

### Pitfall 4: Conflating Dedupe Cache With Durable Dedupe
**What goes wrong:** Runtime local dedupe returns stale/missing results after restart or cross-shard mistake. [VERIFIED: STORE-03 in .planning/REQUIREMENTS.md]  
**Why it happens:** Runtime cache is mistaken for source of truth. [VERIFIED: .planning/PROJECT.md]  
**How to avoid:** Treat shard-local dedupe as optional optimization only; authoritative duplicate response comes from storage append. [VERIFIED: crates/es-store-postgres/tests/dedupe.rs]

### Pitfall 5: Async Runtime Inside Disruptor Processor
**What goes wrong:** A sync disruptor processor needs to await PostgreSQL, causing blocking, nested runtime hacks, or thread starvation. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] [VERIFIED: crates/es-store-postgres/src/event_store.rs]  
**Why it happens:** `PostgresEventStore::append` is async while disruptor processor closures are sync. [VERIFIED: crates/es-store-postgres/src/event_store.rs] [CITED: https://docs.rs/disruptor/4.0.0/disruptor/]  
**How to avoid:** Use the disruptor path as the ordered execution handoff, then have the shard owner drive async append outside the closure through a Tokio task, or use the Event Poller API controlled by a Tokio/blocking bridge after a small spike. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] [ASSUMED]  
**Warning signs:** `block_on`, nested Tokio runtime creation, or `Handle::current().block_on` appears in runtime hot path. [ASSUMED]

### Pitfall 6: Treating `disruptor` Benchmarks as Service Throughput
**What goes wrong:** Ring-only throughput is used to claim command-runtime performance. [VERIFIED: .planning/research/PITFALLS.md]  
**Why it happens:** The ring is faster than storage append, dedupe, serialization, routing, and adapter overhead. [VERIFIED: .planning/research/STACK.md]  
**How to avoid:** Phase 3 tests should prove correctness; Phase 7 benchmarks must separate ring-only, domain-only, storage-only, integrated, and E2E paths. [VERIFIED: TEST-03 and TEST-04 in .planning/REQUIREMENTS.md]

## Code Examples

### Disruptor Non-Blocking Publication

```rust
// Source: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html
match producer.try_publish(|slot| {
    *slot = runtime_event;
}) {
    Ok(sequence) => Ok(sequence),
    Err(disruptor::RingBufferFull) => Err(RuntimeError::ShardOverloaded { shard_id }),
}
```

### Stateful Processor

```rust
// Source: https://docs.rs/disruptor/4.0.0/disruptor/
let processor = |state: &mut ShardState<A>, event: &RuntimeEvent<A>, sequence, end_of_batch| {
    state.record_ring_observation(sequence, end_of_batch);
    state.enqueue_for_async_commit(event.clone());
};

let producer = disruptor::build_single_producer(ring_size, RuntimeEvent::<A>::empty, disruptor::BusySpinWithSpinLoopHint)
    .handle_events_and_state_with(processor, || ShardState::<A>::new(shard_id))
    .build();
```

### Conflict-Safe Cache Update

```rust
// Source: local storage and kernel contracts.
let mut staged_state = cached_state.clone();
for event in &decision.events {
    A::apply(&mut staged_state, event);
}

match store.append(append_request).await {
    Ok(AppendOutcome::Committed(committed) | AppendOutcome::Duplicate(committed)) => {
        *cached_state = staged_state;
        Ok(CommandOutcome::new(decision.reply, committed))
    }
    Err(StoreError::StreamConflict { .. }) => Err(RuntimeError::Conflict),
    Err(error) => Err(RuntimeError::Store(error)),
}
```

### Router Golden Test

```rust
// Source: RUNTIME-02 project requirement.
#[test]
fn same_partition_key_routes_to_same_shard() {
    let router = PartitionRouter::new(8).expect("valid shard count");
    let tenant = TenantId::new("tenant-a").unwrap();
    let key = PartitionKey::new("order-123").unwrap();

    assert_eq!(router.route(&tenant, &key), router.route(&tenant, &key));
    assert_eq!(ShardId(3), router.route(&tenant, &key)); // golden mapping
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Treat disruptor/ring as command success | Event-store append commit is success | Locked in project requirements before Phase 2 | Runtime replies only after storage append. [VERIFIED: .planning/PROJECT.md] |
| One global mutable state map | Single-owner shard-local state | Locked in RUNTIME-03 | Avoid global business-state locks. [VERIFIED: .planning/REQUIREMENTS.md] |
| Blocking queue publication | Non-blocking overload result | `tokio` and `disruptor` current APIs support `try_send`/`try_publish` | Overload is explicit and testable. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html] [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html] |
| Literal `disruptor-rs` crate | `disruptor` crate 4.0.0 | `disruptor` 4.0.0 published 2026-03-09 | Use `disruptor = "4.0.0"` dependency. [VERIFIED: cargo info disruptor] |

**Deprecated/outdated:**
- Using `disruptor-rs = "0.1.1"` as the default crate is outdated for this project because current prior research and crate metadata identify `disruptor = "4.0.0"` as the maintained implementation. [VERIFIED: .planning/research/SUMMARY.md] [VERIFIED: cargo search disruptor]
- Unbounded adapter-to-runtime queues are forbidden by RUNTIME-01 and project pitfalls. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/research/PITFALLS.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `DefaultHasher` should not be treated as a stable routing contract for persistent partition ownership. | Standard Stack / Anti-Patterns | Route implementation might unnecessarily add `twox-hash`, or a standard hasher may be acceptable for local-only stability. |
| A2 | Route drift after shard-count changes needs a migration policy beyond Phase 3. | Common Pitfalls | Planner may need to add documentation/tests limiting shard-count changes in v1. |
| A3 | Best async bridge is likely disruptor handoff plus Tokio-owned async append or Event Poller API spike. | Common Pitfalls | Implementation may discover managed-thread stateful closures are awkward for async storage and require a narrower disruptor integration wrapper. |

## Open Questions

1. **Should Phase 3 use `PostgresEventStore` concrete type or introduce a runtime storage trait?**
   - What we know: `PostgresEventStore` exposes the needed async append and rehydration methods. [VERIFIED: crates/es-store-postgres/src/event_store.rs]
   - What's unclear: Generic runtime tests are easier with a trait, but async traits add either boxed futures or an additional crate. [ASSUMED]
   - Recommendation: Define a small project-owned `RuntimeEventStore` trait returning `BoxFuture` only if tests need fake stores; otherwise start concrete and add the trait when the second implementation appears. [ASSUMED]

2. **Should shard cache use `HashMap` or `moka` in Phase 3?**
   - What we know: RUNTIME-03 requires shard-local ownership, not a specific eviction policy. [VERIFIED: .planning/REQUIREMENTS.md]
   - What's unclear: The phase has no cache-size/TTL requirement. [VERIFIED: .planning/REQUIREMENTS.md]
   - Recommendation: Use shard-local `HashMap` first; add `moka` only when bounded eviction behavior is explicitly tested. [ASSUMED]

3. **How exactly should async append run after disruptor sequencing?**
   - What we know: `disruptor` processors are sync closures; storage append is async. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/] [VERIFIED: crates/es-store-postgres/src/event_store.rs]
   - What's unclear: The cleanest bridge may be managed processor state, Event Poller API, or a shard-owned Tokio task with a synchronous handoff. [ASSUMED]
   - Recommendation: Planner should schedule an early spike/test plan proving the chosen bridge compiles and preserves ordering before implementing full command handling. [ASSUMED]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain | Build/test runtime crates | yes | rustc 1.85.1, cargo 1.85.1 | Install 1.85 via rustup if missing. [VERIFIED: `rustc --version`] |
| Docker | PostgreSQL integration tests via testcontainers | yes | Docker Server 29.3.1 | Local PostgreSQL DSN only if tests are adapted. [VERIFIED: `docker info`] |
| PostgreSQL CLI `psql` | Manual DB inspection | no | - | Not required; SQLx/testcontainers tests do not need `psql`. [VERIFIED: `command -v psql`] |
| pnpm | Node tooling per project instruction | yes | 10.32.1 | Use npm only if no pnpm path exists. [VERIFIED: `pnpm --version`] |
| uv | Python tooling per project instruction | yes | 0.11.6 | Use pip only if no uv path exists. [VERIFIED: `uv --version`] |

**Missing dependencies with no fallback:**
- None for Phase 3 implementation. [VERIFIED: environment audit]

**Missing dependencies with fallback:**
- `psql` is missing; rely on SQLx/testcontainers integration tests. [VERIFIED: environment audit]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness via cargo; Testcontainers for PostgreSQL-backed integration. [VERIFIED: Cargo.toml] |
| Config file | `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`. [VERIFIED: `rg --files`] |
| Quick run command | `cargo test -p es-runtime` |
| Full suite command | `cargo test --workspace` |

`cargo test --workspace --no-run` completed successfully on 2026-04-17. [VERIFIED: command output]

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| RUNTIME-01 | Bounded ingress returns overload when full. [VERIFIED: .planning/REQUIREMENTS.md] | unit | `cargo test -p es-runtime bounded_ingress` | No, Wave 0. [VERIFIED: `rg --files`] |
| RUNTIME-02 | Same tenant/key maps to same shard with golden routes. [VERIFIED: .planning/REQUIREMENTS.md] | unit/property | `cargo test -p es-runtime partition_router` | No, Wave 0. [VERIFIED: `rg --files`] |
| RUNTIME-03 | Shard-local cache has no global `Arc<Mutex<_>>` state map. [VERIFIED: .planning/REQUIREMENTS.md] | unit + grep | `cargo test -p es-runtime shard_cache && ! rg 'Arc<Mutex<.*(State|Cache|HashMap)' crates/es-runtime/src` | No, Wave 0. [VERIFIED: `rg --files`] |
| RUNTIME-04 | Runtime uses `disruptor` path and handles `RingBufferFull`. [VERIFIED: .planning/REQUIREMENTS.md] | unit | `cargo test -p es-runtime disruptor_path` | No, Wave 0. [VERIFIED: `rg --files`] |
| RUNTIME-05 | Reply is sent only after append returns committed/duplicate outcome. [VERIFIED: .planning/REQUIREMENTS.md] | integration/unit with fake store | `cargo test -p es-runtime reply_after_commit` | No, Wave 0. [VERIFIED: `rg --files`] |
| RUNTIME-06 | OCC conflict leaves cache unchanged and returns typed conflict. [VERIFIED: .planning/REQUIREMENTS.md] | unit/integration | `cargo test -p es-runtime conflict_does_not_mutate_cache` | No, Wave 0. [VERIFIED: `rg --files`] |

### Sampling Rate

- **Per task commit:** `cargo test -p es-runtime`
- **Per wave merge:** `cargo test -p es-runtime && cargo test -p es-store-postgres`
- **Phase gate:** `cargo test --workspace` and targeted greps for forbidden global state/ring durability assumptions.

### Wave 0 Gaps

- [ ] `crates/es-runtime/src/error.rs` - typed runtime errors for overload, unavailable, conflict, store errors. [VERIFIED: RUNTIME-01/RUNTIME-06]
- [ ] `crates/es-runtime/src/router.rs` - stable partition router plus golden tests. [VERIFIED: RUNTIME-02]
- [ ] `crates/es-runtime/src/gateway.rs` - bounded ingress and reply channel tests. [VERIFIED: RUNTIME-01/RUNTIME-05]
- [ ] `crates/es-runtime/src/shard.rs` - shard-local cache and command processor tests. [VERIFIED: RUNTIME-03]
- [ ] `crates/es-runtime/src/disruptor_path.rs` - `try_publish` wrapper and full-ring behavior tests. [VERIFIED: RUNTIME-04]
- [ ] Potential test fake for storage append outcomes. [ASSUMED]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 3 has no identity/auth boundary; adapters arrive later. [VERIFIED: .planning/ROADMAP.md] |
| V3 Session Management | no | Phase 3 has no session handling. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | yes | Route and append tenant-scoped commands with `TenantId`; never route by aggregate ID alone. [VERIFIED: crates/es-core/src/lib.rs] |
| V5 Input Validation | yes | Validate shard count, nonempty keys from core constructors, bounded queue/ring capacities. [VERIFIED: crates/es-core/src/lib.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/fn.channel.html] |
| V6 Cryptography | no | Phase 3 does not implement crypto; routing hash is non-cryptographic and must not be used for security. [VERIFIED: phase scope] |

### Known Threat Patterns for Rust Local Runtime

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Tenant route confusion | Elevation of Privilege | Include tenant ID in routing and storage append metadata. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: crates/es-store-postgres/src/models.rs] |
| Queue memory exhaustion | Denial of Service | Bounded mpsc ingress and explicit overload. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| Ring saturation hidden by spinning | Denial of Service | Use `try_publish`, not blocking `publish`. [CITED: https://docs.rs/disruptor/4.0.0/disruptor/trait.Producer.html] |
| Repudiation due to replying before commit | Repudiation | Reply only with committed stream/global positions from storage. [VERIFIED: crates/es-store-postgres/src/models.rs] |
| State corruption on OCC conflict | Tampering | Stage state and update cache only after append success. [VERIFIED: RUNTIME-06 in .planning/REQUIREMENTS.md] |

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` - RUNTIME-01 through RUNTIME-06 and out-of-scope distributed ownership.
- `.planning/ROADMAP.md` - Phase 3 goal and success criteria.
- `.planning/STATE.md` - Phase 2 completion and storage decisions.
- `Cargo.toml` - workspace versions, Rust floor, lint policy.
- `crates/es-core/src/lib.rs` - `TenantId`, `PartitionKey`, `StreamId`, `CommandMetadata`, `ExpectedRevision`.
- `crates/es-kernel/src/lib.rs` - synchronous `Aggregate`, `Decision`, `replay`.
- `crates/es-store-postgres/src/lib.rs`, `event_store.rs`, `models.rs`, `error.rs` - storage API, append outcome, committed positions, conflict errors.
- `cargo info` / crates.io API - `disruptor` 4.0.0, `tokio` 1.52.1, `twox-hash` 2.1.2, `moka` 0.12.15, `crossbeam-channel` 0.5.15, `thiserror` 2.0.18, `tracing` 0.1.44, `futures` 0.3.32.
- Context7 `/nicholassm/disruptor-rs` docs - `build_single_producer`, `try_publish`, `handle_events_and_state_with`, Event Poller API.
- Context7 `/websites/rs_tokio` docs - bounded mpsc and `try_send`.
- Context7 `/websites/rs_moka` docs - bounded cache configuration.
- docs.rs `disruptor` 4.0.0 - https://docs.rs/disruptor/4.0.0/disruptor/
- docs.rs Tokio mpsc - https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html

### Secondary (MEDIUM confidence)

- `.planning/research/STACK.md` - project stack recommendations and benchmark separation.
- `.planning/research/PITFALLS.md` - project pitfalls for disruptor benchmarking, locks, partitioning, and unbounded queues.
- `.planning/research/SUMMARY.md` - prior identification of `disruptor` crate over literal `disruptor-rs`.

### Tertiary (LOW confidence)

- Assumptions A1-A3 in the Assumptions Log, mainly around route hasher contracts and async bridging shape.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions verified via cargo/crates.io and APIs checked through Context7/docs.rs.
- Architecture: MEDIUM-HIGH - tier responsibilities are locked by requirements and current crate boundaries; exact disruptor/async bridge needs implementation spike.
- Pitfalls: HIGH for overload/cache/commit boundaries; MEDIUM for route drift and async bridge details where implementation may reveal a better local shape.

**Research date:** 2026-04-17  
**Valid until:** 2026-05-17 for project constraints; 2026-04-24 for crate currentness because `tokio` changed on 2026-04-16 and runtime crates are active.

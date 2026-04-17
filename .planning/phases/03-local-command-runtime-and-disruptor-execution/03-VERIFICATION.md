---
phase: 03-local-command-runtime-and-disruptor-execution
verified: 2026-04-17T04:41:55Z
status: passed
score: 17/17 must-haves verified
overrides_applied: 0
---

# Phase 3: Local Command Runtime and Disruptor Execution Verification Report

**Phase Goal:** Requests enter a bounded local command engine, route by aggregate/partition key to a single shard owner, execute through an in-process disruptor path, and reply only after event-store commit.  
**Verified:** 2026-04-17T04:41:55Z  
**Status:** passed  
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Adapter-facing callers can submit commands through bounded ingress and receive explicit overload behavior when capacity is exhausted. | VERIFIED | `CommandGateway::try_submit` routes then calls `sender.try_send`; `TrySendError::Full` maps to `RuntimeError::Overloaded`. Covered by `bounded_ingress_returns_overloaded_when_full` and runtime engine overload flow. |
| 2 | Commands for the same aggregate key consistently reach the same local shard owner under stable partition configuration. | VERIFIED | `PartitionRouter` uses fixed `ROUTING_HASH_SEED`, tenant bytes, separator byte, partition key bytes, and modulo shard count. Golden tests pass for same tenant/key and tenant-aware routing. |
| 3 | Shard-local aggregate and dedupe caches are owned by the shard runtime without global `Arc<Mutex<_>>` business-state maps. | VERIFIED | `ShardState` owns `AggregateCache` and `DedupeCache`; grep found no production `Arc<Mutex<.*(State|Cache|HashMap)` or matching `RwLock` patterns in `crates/es-runtime/src`. |
| 4 | The runtime uses `disruptor` as an in-process execution/fan-out mechanism, not as durability, a broker, or distributed ownership. | VERIFIED | `DisruptorPath` wraps `disruptor::build_single_producer`, `try_publish`, and `EventPoller`; durable success comes only from `CommittedAppend`, not disruptor sequence. |
| 5 | Command replies are sent only after durable event-store append succeeds, and optimistic concurrency failures surface as typed conflict or retryable errors. | VERIFIED | `ShardState::process_next_handoff` calls `store.append(...).await` before success reply; conflict errors use `RuntimeError::from_store_error` and preserve cache. |
| 6 | Runtime callers can receive typed overload, unavailable, conflict, domain, codec, and store errors. | VERIFIED | `RuntimeError` has typed variants for all listed cases; tests cover capacity errors, conflict mapping, domain, codec, and store/rehydration failures. |
| 7 | Runtime command envelopes carry metadata, idempotency key, partition key, expected revision, and one-shot reply channel. | VERIFIED | `CommandEnvelope<A>` stores command metadata, idempotency key, stream ID, partition key, expected revision, and `CommandReply`; constructor derives fields from `Aggregate`. |
| 8 | Runtime tests can use a fake event store without PostgreSQL while preserving append-after-decision semantics. | VERIFIED | `RuntimeEventStore` trait plus fake stores in integration tests record appends and produce committed, duplicate, delayed, and error outcomes without PostgreSQL. |
| 9 | Different tenants with the same aggregate key are routed using tenant-aware input. | VERIFIED | Router writes tenant ID into the hash before partition key; `tenant_is_part_of_route_input` passes. |
| 10 | The disruptor path uses non-blocking publication and maps a full ring to typed shard overload. | VERIFIED | `DisruptorPath::try_publish` uses `producer.try_publish` and maps `RingBufferFull` to `RuntimeError::ShardOverloaded`. Covered by `disruptor_path_returns_shard_overloaded_when_ring_is_full`. |
| 11 | Accepted routed commands are not processable until the disruptor consumer/poller releases the matching tenant-scoped unique handoff token. | VERIFIED | `ShardHandle::accept_routed` stores pending envelopes by `ShardHandoffToken`; `drain_released_handoffs` moves only poller-released tokens into `ShardState`. Tests cover release gating and duplicate/cross-tenant pending keys. |
| 12 | The disruptor-to-async bridge compiles and releases ordered handoff tokens without nested Tokio blocking. | VERIFIED | `poll_released` drains `EventPoller`; grep found no `block_on`, `Runtime::new`, or `Handle::current` in runtime source. |
| 13 | Cache misses rehydrate aggregate state from `RuntimeEventStore::load_rehydration` before `A::decide` runs on existing streams. | VERIFIED | `process_next_handoff` calls `rehydrate_state` on cache miss, commits rehydrated state, then calls `A::decide`; `cache_miss_rehydrates_before_decide` passes. |
| 14 | Optimistic concurrency conflicts return typed runtime conflict errors and leave shard cache unchanged. | VERIFIED | `StoreError::StreamConflict` maps to `RuntimeError::Conflict`; `conflict_does_not_mutate_cache` asserts cache and dedupe preservation. |
| 15 | Shard-local dedupe cache records committed or duplicate append summaries after durable append, but duplicate outcomes never apply newly decided events to aggregate cache. | VERIFIED | `AppendOutcome::Committed` applies staged state and records dedupe; `AppendOutcome::Duplicate` records dedupe only. Duplicate cache preservation tests pass. |
| 16 | A production runtime engine owns gateway receive, shard handoff, disruptor release drain, store append, codec usage, and reply delivery end-to-end. | VERIFIED | `CommandEngine` owns gateway, `mpsc::Receiver<RoutedCommand<A>>`, shards, store, and codec; `process_one` receives, accepts routed command, drains released handoffs, and calls `process_next_handoff`. |
| 17 | Runtime flow tests cover success, overload, conflict, and unavailable/store-failure outcomes through the command path. | VERIFIED | `cargo test -p es-runtime` covers success, ingress overload, closed ingress unavailable, conflicts, domain/codec errors, and store/rehydration failures. |

**Score:** 17/17 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/es-runtime/src/error.rs` | Runtime error contract | VERIFIED | Exists; substantive; exported by `lib.rs`; contains typed variants and store conflict mapping. |
| `crates/es-runtime/src/command.rs` | Command envelope/outcome/reply/codec contracts | VERIFIED | Exists; substantive; exported by `lib.rs`; links to `Aggregate` associated types and store DTOs. |
| `crates/es-runtime/src/store.rs` | Runtime event-store trait and PostgreSQL adapter | VERIFIED | Exists; substantive; forwards `append` and `load_rehydration` to `PostgresEventStore`. |
| `crates/es-runtime/src/router.rs` | Stable tenant-aware routing | VERIFIED | Exists; uses fixed xxHash seed and tenant separator; covered by golden route tests. |
| `crates/es-runtime/src/gateway.rs` | Bounded command gateway | VERIFIED | Exists; uses bounded Tokio mpsc and `try_send`; no awaited send found. |
| `crates/es-runtime/src/cache.rs` | Shard-local aggregate and dedupe caches | VERIFIED | Exists; cache state is local `HashMap` owned by runtime objects, not global locks. |
| `crates/es-runtime/src/disruptor_path.rs` | Nonblocking disruptor wrapper | VERIFIED | Exists; uses `try_publish` and `EventPoller` release drain. |
| `crates/es-runtime/src/shard.rs` | Shard state, handoff, commit processor | VERIFIED | Exists; owns cache/dedupe, pending table, disruptor path, and commit-gated processing. |
| `crates/es-runtime/src/engine.rs` | Production command engine | VERIFIED | Exists; wires gateway receiver, shard handles, store, codec, and reply flow. |
| `crates/es-runtime/tests/router_gateway.rs` | Router and gateway tests | VERIFIED | Exists; covers stable routes, tenant-aware routing, overload, unavailable. |
| `crates/es-runtime/tests/shard_disruptor.rs` | Cache, disruptor, shard handoff tests | VERIFIED | Exists; covers shard-local caches, full-ring overload, and release-gated processing. |
| `crates/es-runtime/tests/runtime_flow.rs` | Runtime flow tests | VERIFIED | Exists; covers commit, duplicate, conflict, failures, overload, and engine flow. |
| `crates/es-runtime/tests/common/mod.rs` | Fake store harness | VERIFIED | Exists; test-only `Arc<Mutex<Vec<AppendRequest>>>` is confined to tests. |
| `.planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md` | Phase validation metadata | VERIFIED | Frontmatter has `nyquist_compliant: true` and `wave_0_complete: true`. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `command.rs` | `es-kernel` | Aggregate associated types | WIRED | `CommandEnvelope<A: Aggregate>` calls `A::stream_id`, `A::partition_key`, and `A::expected_revision`. |
| `store.rs` | `es-store-postgres` | append/load adapter | WIRED | `PostgresRuntimeEventStore` forwards both calls to the underlying `PostgresEventStore`. |
| `gateway.rs` | `router.rs` | route before submit | WIRED | `try_submit` calls `self.router.route(...)` before `sender.try_send`. Automated regex missed this because it spans lines. |
| `gateway.rs` | Tokio mpsc | bounded ingress | WIRED | `CommandGateway::new` creates bounded `mpsc::channel`; `try_submit` uses `try_send`. |
| `disruptor_path.rs` | `disruptor` | try_publish wrapper | WIRED | Uses `build_single_producer`, `new_event_poller`, `producer.try_publish`, and poller release drain. |
| `shard.rs` | `cache.rs` | shard-owned cache/dedupe | WIRED | `ShardState` stores `AggregateCache<A>` and `DedupeCache` directly. |
| `shard.rs` | `disruptor_path.rs` | publish/drain handoffs | WIRED | `ShardHandle` calls `path.try_publish` and `path.poll_released`. |
| `shard.rs` | `store.rs` | durable append and rehydration | WIRED | `process_next_handoff` uses `RuntimeEventStore::append`; `rehydrate_state` uses `load_rehydration`. |
| `engine.rs` | `gateway.rs` / `shard.rs` | production runtime loop | WIRED | `process_one` awaits `receiver.recv`, calls `accept_routed`, `drain_released_handoffs`, then `process_next_handoff`. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `CommandGateway` | `RoutedCommand<A>` | `CommandEnvelope` submitted by caller plus `PartitionRouter::route` | Yes | FLOWING - routed command is sent through bounded mpsc receiver. |
| `CommandEngine` | `RoutedCommand<A>` | `receiver.recv().await` from gateway-created channel | Yes | FLOWING - accepted command is dispatched to owning `ShardHandle`. |
| `ShardHandle` | `ShardHandoffToken` / pending envelope | `DisruptorPath::try_publish` and pending `HashMap` | Yes | FLOWING - poller release moves matching pending envelope into processable queue. |
| `ShardState::process_next_handoff` | aggregate state and events | cache or `RuntimeEventStore::load_rehydration`, then `A::decide` and codec | Yes | FLOWING - tests verify rehydration-before-decide and codec/domain failure branches. |
| `ShardState::process_next_handoff` | `CommandOutcome` reply | `RuntimeEventStore::append` returning `Committed` or `Duplicate` | Yes | FLOWING - success reply includes `CommittedAppend.global_positions`; delayed-append test proves no early reply. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Runtime engine end-to-end commit path | `cargo test -p es-runtime runtime_engine -- --nocapture` | 1 filtered runtime engine test passed | PASS |
| Runtime flow overload/conflict/commit path | `cargo test -p es-runtime runtime_flow -- --nocapture` | 1 filtered runtime flow test passed | PASS |
| Full runtime test suite | `cargo test -p es-runtime` | 34 tests passed; one existing missing-docs warning in test crate | PASS |
| PostgreSQL store contract suite | `cargo test -p es-store-postgres` | 27 tests passed | PASS |
| Workspace suite | `cargo test --workspace` | Workspace tests and doc-tests passed | PASS |
| Forbidden production patterns | `! rg 'Arc<Mutex<.*(State|Cache|HashMap)|RwLock<.*(State|Cache|HashMap)' crates/es-runtime/src && ! rg 'block_on|Runtime::new|Handle::current' crates/es-runtime/src && ! rg '\.send\(\)\.await|send\(\s*.*\)\.await' crates/es-runtime/src/gateway.rs` | No forbidden matches | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| RUNTIME-01 | 03-01, 03-02, 03-04 | Adapter requests enter bounded ingress with explicit overload behavior. | SATISFIED | `CommandGateway` bounded mpsc `try_send`, overload and unavailable tests, runtime engine overload flow. |
| RUNTIME-02 | 03-01, 03-02 | Partition routing sends all commands for the same aggregate key to the same local shard owner. | SATISFIED | Fixed-seed tenant-aware router and golden route tests. |
| RUNTIME-03 | 03-01, 03-03, 03-04 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. | SATISFIED | `ShardState` owns caches; forbidden lock grep passes. |
| RUNTIME-04 | 03-01, 03-03 | Shard runtime integrates the `disruptor` crate as local execution/fan-out mechanism. | SATISFIED | `DisruptorPath` uses `disruptor` producer/poller with nonblocking publish and full-ring test. |
| RUNTIME-05 | 03-01, 03-04 | Command replies are sent only after durable event-store append commit succeeds. | SATISFIED | `reply_is_sent_after_append_commit` delays append and proves reply does not resolve before release; success reply includes `CommittedAppend`. |
| RUNTIME-06 | 03-01, 03-04 | OCC conflicts are surfaced as typed conflict errors without corrupting shard-local cache. | SATISFIED | `RuntimeError::Conflict` mapping and `conflict_does_not_mutate_cache` pass. |

No orphaned Phase 03 requirements were found in `REQUIREMENTS.md`; all six declared runtime requirements are claimed by phase plans and verified against code.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/es-runtime/src/shard.rs` | 49 | Internal `ShardHandoffToken::placeholder` sentinel for preallocated disruptor ring slots | INFO | Purposeful internal factory; published slots are overwritten before release and it does not reach user-visible output. |
| `crates/es-runtime/tests/shard_disruptor.rs` | 1 | Missing documentation warning from test crate | INFO | Tests pass; warning does not affect runtime behavior. |

### Human Verification Required

None. This phase is runtime code with deterministic unit/integration coverage and no visual, external manual workflow, or live service behavior required for goal verification.

### Gaps Summary

No blocking gaps found. The phase goal is achieved: command ingress is bounded, routing is stable and tenant-aware, shard-local state is single-owner, disruptor is used as a local handoff path only, and command replies are gated on durable append outcomes with typed conflict/error handling.

---

_Verified: 2026-04-17T04:41:55Z_  
_Verifier: Claude (gsd-verifier)_

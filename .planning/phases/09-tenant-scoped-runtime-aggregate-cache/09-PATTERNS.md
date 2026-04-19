# Phase 09: Tenant-Scoped Runtime Aggregate Cache - Pattern Map

**Mapped:** 2026-04-20
**Files analyzed:** 5
**Analogs found:** 5 / 5

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/es-runtime/src/cache.rs` | utility | CRUD | `crates/es-runtime/src/cache.rs` (`DedupeKey`, `DedupeCache`) | exact |
| `crates/es-runtime/src/shard.rs` | service | request-response | `crates/es-runtime/src/shard.rs` (`ShardState::process_next_handoff`) | exact |
| `crates/es-runtime/src/lib.rs` | config | transform | `crates/es-runtime/src/lib.rs` existing public re-exports | exact |
| `crates/es-runtime/tests/shard_disruptor.rs` | test | CRUD | `crates/es-runtime/tests/shard_disruptor.rs` cache and dedupe unit tests | exact |
| `crates/es-runtime/tests/runtime_flow.rs` | test | request-response | `crates/es-runtime/tests/runtime_flow.rs` fake store, replay, rehydration, conflict tests | exact |

## Pattern Assignments

### `crates/es-runtime/src/cache.rs` (utility, CRUD)

**Analog:** `crates/es-runtime/src/cache.rs`

**Imports pattern** (lines 1-4):
```rust
use std::collections::HashMap;

use es_core::{StreamId, TenantId};
use es_kernel::Aggregate;
```

**Typed composite key pattern** (lines 51-58):
```rust
/// Tenant-scoped dedupe cache key for a shard-local optimization.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DedupeKey {
    /// Tenant that owns the idempotency key.
    pub tenant_id: TenantId,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
}
```

Apply this shape to add `AggregateCacheKey` near `DedupeKey`, using owned typed IDs:
```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AggregateCacheKey {
    /// Tenant that owns the stream state.
    pub tenant_id: TenantId,
    /// Stream whose aggregate state is cached.
    pub stream_id: StreamId,
}
```

**Current aggregate cache API to replace** (lines 6-38):
```rust
/// Shard-local aggregate state cache owned by a single shard runtime.
pub struct AggregateCache<A: Aggregate> {
    states: HashMap<StreamId, A::State>,
}

impl<A: Aggregate> AggregateCache<A> {
    /// Returns cached state, inserting a default aggregate state when the stream is absent.
    pub fn get_or_default(&mut self, stream_id: &StreamId) -> A::State {
        self.states.entry(stream_id.clone()).or_default().clone()
    }

    /// Replaces the cached state after the caller has committed the staged state.
    pub fn commit_state(&mut self, stream_id: StreamId, state: A::State) {
        self.states.insert(stream_id, state);
    }

    /// Returns cached state without creating a default entry.
    pub fn get(&self, stream_id: &StreamId) -> Option<&A::State> {
        self.states.get(stream_id)
    }
}
```

Use the same API names if possible, but change `HashMap<StreamId, A::State>` to `HashMap<AggregateCacheKey, A::State>` and make `get_or_default`, `commit_state`, and `get` require `AggregateCacheKey`. Make it impossible to call aggregate cache lookup with only `StreamId`.

**Cache CRUD pattern** (lines 67-99):
```rust
/// Shard-local dedupe cache. PostgreSQL remains authoritative for command dedupe.
#[derive(Default)]
pub struct DedupeCache {
    records: HashMap<DedupeKey, DedupeRecord>,
}

impl DedupeCache {
    /// Returns a cached dedupe record for a tenant-scoped idempotency key.
    pub fn get(&self, key: &DedupeKey) -> Option<&DedupeRecord> {
        self.records.get(key)
    }

    /// Records a committed append summary in the shard-local dedupe cache.
    pub fn record(&mut self, key: DedupeKey, record: DedupeRecord) {
        self.records.insert(key, record);
    }
}
```

Copy this ownership pattern for the aggregate cache: immutable lookup by borrowed key, owned key consumed by commit/insert, no locks, no global cache.

---

### `crates/es-runtime/src/shard.rs` (service, request-response)

**Analog:** `crates/es-runtime/src/shard.rs`

**Imports pattern** (lines 10-14):
```rust
use crate::{
    AggregateCache, CommandEnvelope, CommandOutcome, DedupeCache, DedupeKey, DedupeRecord,
    DisruptorPath, RoutedCommand, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
    RuntimeResult, ShardId,
};
```

Add `AggregateCacheKey` to this grouped crate import once exported or available from `crate::cache`.

**Shard-owned state pattern** (lines 68-84):
```rust
/// Shard-owned state and processable handoff queue.
pub struct ShardState<A: Aggregate> {
    shard_id: ShardId,
    cache: AggregateCache<A>,
    dedupe: DedupeCache,
    handoffs: VecDeque<ShardHandoff<A>>,
}

impl<A: Aggregate> ShardState<A> {
    /// Creates empty state owned by one local shard.
    pub fn new(shard_id: ShardId) -> Self {
        Self {
            shard_id,
            cache: AggregateCache::new(),
            dedupe: DedupeCache::new(),
            handoffs: VecDeque::new(),
        }
    }
}
```

Preserve this ownership model. Phase 9 changes cache key identity only; do not introduce `Arc`, `Mutex`, shared adapter state, or storage coupling in `ShardState`.

**Duplicate replay ordering pattern** (lines 170-216):
```rust
let dedupe_key = DedupeKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    idempotency_key: envelope.idempotency_key.clone(),
};

if let Some(record) = self.dedupe.get(&dedupe_key) {
    let outcome = replay_command_outcome::<A, C>(codec, &record.replay);
    let _ = envelope.reply.send(outcome);
    return Ok(true);
}

match store
    .lookup_command_replay(&envelope.metadata.tenant_id, &envelope.idempotency_key)
    .await
{
    Ok(Some(replay)) => {
        let outcome = replay_command_outcome::<A, C>(codec, &replay);
        if outcome.is_ok() {
            self.dedupe
                .record(dedupe_key.clone(), DedupeRecord { replay });
        }
        let _ = envelope.reply.send(outcome);
        return Ok(true);
    }
    Ok(None) => {}
    Err(error) => {
        let _ = envelope
            .reply
            .send(Err(RuntimeError::from_store_error(error)));
        return Ok(true);
    }
}
```

Keep this order unchanged: shard-local dedupe, durable replay lookup, then aggregate cache/rehydration. The aggregate cache key should be constructed after replay misses, before cache lookup.

**Aggregate cache lookup/fill pattern to update** (lines 218-232):
```rust
let current_state = if let Some(cached) = self.cache.get(&envelope.stream_id) {
    cached.clone()
} else {
    match rehydrate_state(store, codec, &envelope).await {
        Ok(rehydrated) => {
            self.cache
                .commit_state(envelope.stream_id.clone(), rehydrated.clone());
            rehydrated
        }
        Err(error) => {
            let _ = envelope.reply.send(Err(error));
            return Ok(true);
        }
    }
};
```

Use one composite key for every operation in the handoff:
```rust
let cache_key = AggregateCacheKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    stream_id: envelope.stream_id.clone(),
};

let current_state = if let Some(cached) = self.cache.get(&cache_key) {
    cached.clone()
} else {
    match rehydrate_state(store, codec, &envelope).await {
        Ok(rehydrated) => {
            self.cache.commit_state(cache_key.clone(), rehydrated.clone());
            rehydrated
        }
        Err(error) => {
            let _ = envelope.reply.send(Err(error));
            return Ok(true);
        }
    }
};
```

**Commit-gated cache mutation pattern** (lines 299-323):
```rust
match store.append(append_request).await {
    Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
        let mut staged_state = current_state;
        for event in &decision.events {
            A::apply(&mut staged_state, event);
        }
        self.cache
            .commit_state(envelope.stream_id.clone(), staged_state);
        self.dedupe.record(
            dedupe_key,
            DedupeRecord {
                replay: es_store_postgres::CommandReplayRecord {
                    append: committed.clone(),
                    reply: command_reply_payload.clone(),
                },
            },
        );
        let reply = decision.reply;
        let _ = envelope
            .reply
            .send(Ok(CommandOutcome::new(reply, committed)));
    }
}
```

Keep the append-then-apply-then-cache-then-reply sequence. Only replace `envelope.stream_id.clone()` with the same `cache_key` created before lookup.

**Tenant-scoped rehydration boundary** (lines 406-430):
```rust
async fn rehydrate_state<A, S, C>(
    store: &S,
    codec: &C,
    envelope: &CommandEnvelope<A>,
) -> RuntimeResult<A::State>
where
    A: Aggregate,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A>,
{
    let batch = store
        .load_rehydration(&envelope.metadata.tenant_id, &envelope.stream_id)
        .await
        .map_err(RuntimeError::from_store_error)?;
    let mut state = match &batch.snapshot {
        Some(snapshot) => codec.decode_snapshot(snapshot)?,
        None => A::State::default(),
    };

    for stored in &batch.events {
        let event = codec.decode(stored)?;
        A::apply(&mut state, &event);
    }

    Ok(state)
}
```

Do not change this function's storage semantics. The bug is that stream-only cache hits skip this already tenant-scoped call.

---

### `crates/es-runtime/src/lib.rs` (config, transform)

**Analog:** `crates/es-runtime/src/lib.rs`

**Module and re-export pattern** (lines 3-13):
```rust
mod cache;
mod command;
mod disruptor_path;
mod engine;
mod error;
mod gateway;
mod router;
mod shard;
mod store;

pub use cache::{AggregateCache, DedupeCache, DedupeKey, DedupeRecord};
```

If tests or downstream crates need to construct `AggregateCacheKey`, add it to the existing cache re-export:
```rust
pub use cache::{AggregateCache, AggregateCacheKey, DedupeCache, DedupeKey, DedupeRecord};
```

Keep modules private and expose only the crate boundary types already used by integration tests.

---

### `crates/es-runtime/tests/shard_disruptor.rs` (test, CRUD)

**Analog:** `crates/es-runtime/tests/shard_disruptor.rs`

**Imports pattern** (lines 1-7):
```rust
use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision};
use es_kernel::{Aggregate, Decision};
use es_runtime::{
    AggregateCache, CommandEnvelope, DedupeCache, DedupeKey, DedupeRecord, DisruptorPath,
    RoutedCommand, RuntimeError, ShardHandle, ShardId, ShardState,
};
use es_store_postgres::{CommandReplayRecord, CommandReplyPayload, CommittedAppend};
```

Add `AggregateCacheKey` to the `es_runtime` import and `TenantId` to `es_core` if the key is public.

**Test helper pattern** (lines 67-83):
```rust
fn stream_id(value: &'static str) -> StreamId {
    StreamId::new(value).expect("stream id")
}

fn tenant_id(value: &'static str) -> es_core::TenantId {
    es_core::TenantId::new(value).expect("tenant id")
}

fn metadata(tenant: &'static str) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: tenant_id(tenant),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}
```

Add a local `cache_key(tenant, stream)` helper next to these helpers:
```rust
fn cache_key(tenant: &'static str, stream: &'static str) -> AggregateCacheKey {
    AggregateCacheKey {
        tenant_id: tenant_id(tenant),
        stream_id: stream_id(stream),
    }
}
```

**Aggregate cache unit style to update** (lines 117-142):
```rust
#[test]
fn shard_cache_inserts_default_state_locally() {
    let mut cache = AggregateCache::<CounterAggregate>::new();
    let stream_id = stream_id("counter-1");

    let state = cache.get_or_default(&stream_id);

    assert_eq!(CounterState::default(), state);
    assert_eq!(Some(&CounterState::default()), cache.get(&stream_id));
    assert_eq!(1, cache.len());
}

#[test]
fn shard_cache_commits_only_explicit_state() {
    let mut cache = AggregateCache::<CounterAggregate>::new();
    let stream_id = stream_id("counter-1");

    let mut staged_state = cache.get_or_default(&stream_id);
    staged_state.value = 7;

    assert_eq!(Some(&CounterState::default()), cache.get(&stream_id));

    cache.commit_state(stream_id.clone(), staged_state.clone());

    assert_eq!(Some(&staged_state), cache.get(&stream_id));
}
```

Convert these tests to pass `AggregateCacheKey`. Add a new same-stream/different-tenant unit assertion:
```rust
let tenant_a_key = cache_key("tenant-a", "counter-1");
let tenant_b_key = cache_key("tenant-b", "counter-1");

cache.commit_state(tenant_a_key.clone(), CounterState { value: 7 });

assert_eq!(Some(&CounterState { value: 7 }), cache.get(&tenant_a_key));
assert_eq!(None, cache.get(&tenant_b_key));
```

**Existing tenant-scoped dedupe assertion style** (lines 144-170):
```rust
#[test]
fn shard_dedupe_cache_records_tenant_scoped_committed_append() {
    let mut cache = DedupeCache::new();
    let key = DedupeKey {
        tenant_id: tenant_id("tenant-a"),
        idempotency_key: "idem-1".to_owned(),
    };

    cache.record(key.clone(), record.clone());

    assert_eq!(Some(&record), cache.get(&key));
    assert_eq!(1, cache.len());
    assert_eq!(
        None,
        cache.get(&DedupeKey {
            tenant_id: tenant_id("tenant-b"),
            idempotency_key: "idem-1".to_owned(),
        })
    );
}
```

Copy this negative cross-tenant assertion pattern for aggregate cache isolation.

---

### `crates/es-runtime/tests/runtime_flow.rs` (test, request-response)

**Analog:** `crates/es-runtime/tests/runtime_flow.rs`

**Imports pattern** (lines 3-23):
```rust
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_kernel::{Aggregate, Decision};
use es_runtime::{
    CommandEngine, CommandEngineConfig, CommandEnvelope, DedupeKey, DedupeRecord, RuntimeError,
    RuntimeEventCodec, RuntimeEventStore, ShardId, ShardState,
};
use es_store_postgres::{
    AppendOutcome, AppendRequest, CommandReplayRecord, CommandReplyPayload, CommittedAppend,
    NewEvent, RehydrationBatch, SnapshotRecord, StoreError, StoredEvent,
};
```

Add `AggregateCacheKey` to imports if helper assertions inspect cache directly with keys.

**Fake store state pattern to extend** (lines 192-206):
```rust
#[derive(Clone)]
struct FakeStore {
    inner: Arc<FakeStoreInner>,
}

struct FakeStoreInner {
    append_requests: Mutex<Vec<AppendRequest>>,
    append_outcomes: Mutex<VecDeque<Result<AppendOutcome, StoreError>>>,
    command_replay: Mutex<VecDeque<Option<CommandReplayRecord>>>,
    lookup_count: Mutex<usize>,
    rehydration: Mutex<RehydrationBatch>,
    rehydration_error: Mutex<Option<StoreError>>,
    append_gate: Mutex<Option<oneshot::Receiver<()>>>,
    append_started: Notify,
}
```

Extend this fake, rather than adding another fake store type. Add fields such as:
```rust
rehydration_calls: Mutex<Vec<(TenantId, StreamId)>>,
tenant_rehydration: Mutex<VecDeque<((TenantId, StreamId), RehydrationBatch)>>,
```

or use a keyed map if the tests need out-of-order lookup. Keep the same `Arc<Mutex<...>>` style used by the existing fake.

**Fake store setter pattern** (lines 241-259):
```rust
fn set_rehydration(&self, rehydration: RehydrationBatch) {
    *self.inner.rehydration.lock().expect("rehydration") = rehydration;
}

fn set_rehydration_error(&self, error: StoreError) {
    *self
        .inner
        .rehydration_error
        .lock()
        .expect("rehydration error") = Some(error);
}

fn set_command_replay(&self, replay: CommandReplayRecord) {
    self.set_command_replay_sequence(vec![Some(replay)]);
}

fn set_command_replay_sequence(&self, replay: Vec<Option<CommandReplayRecord>>) {
    *self.inner.command_replay.lock().expect("command replay") = replay.into();
}
```

Add tenant-specific helpers in this style:
```rust
fn set_tenant_rehydration(
    &self,
    tenant_id: TenantId,
    stream_id: StreamId,
    rehydration: RehydrationBatch,
) {
    self.inner
        .tenant_rehydration
        .lock()
        .expect("tenant rehydration")
        .push_back(((tenant_id, stream_id), rehydration));
}

fn rehydration_calls(&self) -> Vec<(TenantId, StreamId)> {
    self.inner
        .rehydration_calls
        .lock()
        .expect("rehydration calls")
        .clone()
}
```

**Tenant-specific rehydration implementation point** (lines 306-326):
```rust
fn load_rehydration(
    &self,
    _tenant_id: &TenantId,
    _stream_id: &StreamId,
) -> BoxFuture<'_, es_store_postgres::StoreResult<RehydrationBatch>> {
    let error = self
        .inner
        .rehydration_error
        .lock()
        .expect("rehydration error")
        .take();
    let batch = self.inner.rehydration.lock().expect("rehydration").clone();

    Box::pin(async move {
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(batch)
        }
    })
}
```

Change `_tenant_id` and `_stream_id` to named params, record the call, and return the tenant-specific batch when configured. Preserve the existing fallback to `self.inner.rehydration` for current tests.

**Warm cache helper to update** (lines 424-435):
```rust
fn dedupe_key() -> DedupeKey {
    DedupeKey {
        tenant_id: tenant_id(),
        idempotency_key: "idem-1".to_owned(),
    }
}

fn warm_cache(state: &mut ShardState<CounterAggregate>, value: i64) {
    state.cache_mut().commit_state(
        StreamId::new("counter-1").expect("stream id"),
        CounterState { value },
    );
}
```

Change `warm_cache` to build `AggregateCacheKey` using `tenant_id()` plus `StreamId::new("counter-1")`. If adding cross-tenant tests, add `metadata_for(tenant)` and `envelope_for(tenant, stream, idempotency_key, amount)` helpers instead of overloading the fixed `envelope()`.

**Commit-gated reply test style** (lines 465-500):
```rust
#[tokio::test]
async fn reply_is_sent_after_append_commit() {
    let (release_append, wait_for_release) = oneshot::channel();
    let store = FakeStore::with_delayed_commit(wait_for_release);
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(3);
    record_handoff(&mut state, envelope);

    let store_for_task = store.clone();
    let task = tokio::spawn(async move {
        state
            .process_next_handoff(&store_for_task, &codec)
            .await
            .expect("processed");
        state
    });

    store.wait_for_append_start().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(20), receiver)
            .await
            .is_err(),
        "reply resolved before durable append completed"
    );

    release_append.send(()).expect("release append");
    let state = task.await.expect("task joined");
}
```

Use this style for any async sequencing regression: record handoff, process once, inspect reply, store counters, and cache state.

**Conflict does not mutate cache pattern** (lines 502-539):
```rust
#[tokio::test]
async fn conflict_does_not_mutate_cache() {
    let store = FakeStore::with_append_result(Err(StoreError::StreamConflict {
        stream_id: "counter-1".to_owned(),
        expected: "exact 99".to_owned(),
        actual: Some(1),
    }));
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    warm_cache(&mut state, 10);
    let (envelope, receiver) = envelope(3);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let error = expect_runtime_error(receiver.await.expect("reply"));
    assert_eq!(1, store.appended_len());
    assert_eq!(0, state.dedupe().len());
    assert_eq!(
        Some(&CounterState { value: 10 }),
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
    );
}
```

Extend this pattern with tenant-specific cache keys to prove a conflict in tenant B does not mutate tenant A's cached state.

**Cache miss rehydration test style** (lines 627-653):
```rust
#[tokio::test]
async fn cache_miss_rehydrates_before_decide() {
    let store = FakeStore::committed();
    store.set_rehydration(RehydrationBatch {
        snapshot: None,
        events: vec![stored_event(1, 5)],
    });
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(3);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(8, outcome.reply);
}
```

Add same-stream/different-tenant regression tests by running two handoffs through the same `ShardState`, same stream ID, different tenant IDs, and tenant-specific rehydration batches. Assert two `load_rehydration` calls: `(tenant-a, counter-1)` and `(tenant-b, counter-1)`.

**Duplicate replay order regression pattern** (lines 656-688):
```rust
#[tokio::test]
async fn runtime_duplicate_cache_hit_skips_decide_and_append() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    warm_cache(&mut state, 10);
    state.dedupe_mut().record(
        dedupe_key(),
        DedupeRecord {
            replay: command_replay_record(7, 3),
        },
    );
    let (envelope, receiver) = envelope(100);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(3, outcome.reply);
    assert_eq!(0, store.appended_len());
    assert_eq!(0, store.lookup_count());
}
```

Keep these tests green. They enforce that duplicate replay remains before aggregate rehydration and append after the cache-key refactor.

**Durable replay before rehydration pattern** (lines 725-751):
```rust
#[tokio::test]
async fn runtime_duplicate_store_hit_skips_rehydrate_decide_encode_and_append() {
    let store = FakeStore::committed();
    store.set_command_replay(command_replay_record(9, 11));
    store.set_rehydration_error(StoreError::DedupeConflict {
        tenant_id: "tenant-a".to_owned(),
        idempotency_key: "idem-1".to_owned(),
    });
    let codec = CounterCodec { fail_encode: true };
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(30);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(11, outcome.reply);
    assert_eq!(0, store.appended_len());
    assert_eq!(1, store.lookup_count());
    assert!(state.cache().is_empty());
    assert_eq!(1, state.dedupe().len());
}
```

This is the key Phase 8 ordering guard. Do not move aggregate cache lookup above `lookup_command_replay`.

## Shared Patterns

### Typed ID Keys
**Source:** `crates/es-runtime/src/cache.rs`
**Apply to:** `cache.rs`, `shard.rs`, `shard_disruptor.rs`, `runtime_flow.rs`
```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DedupeKey {
    pub tenant_id: TenantId,
    pub idempotency_key: String,
}
```

Use derived `Eq` and `Hash` on typed owned IDs. Avoid string concatenation, delimiter keys, or manual hashing.

### Shard-Local Ownership
**Source:** `crates/es-runtime/src/shard.rs`
**Apply to:** `shard.rs`
```rust
pub struct ShardState<A: Aggregate> {
    shard_id: ShardId,
    cache: AggregateCache<A>,
    dedupe: DedupeCache,
    handoffs: VecDeque<ShardHandoff<A>>,
}
```

Keep cache state processor-local and owned by `ShardState`.

### Runtime Handoff Order
**Source:** `crates/es-runtime/src/shard.rs`
**Apply to:** `shard.rs`, `runtime_flow.rs`
```rust
if let Some(record) = self.dedupe.get(&dedupe_key) {
    let outcome = replay_command_outcome::<A, C>(codec, &record.replay);
    let _ = envelope.reply.send(outcome);
    return Ok(true);
}

match store
    .lookup_command_replay(&envelope.metadata.tenant_id, &envelope.idempotency_key)
    .await
{
    Ok(Some(replay)) => {
        let outcome = replay_command_outcome::<A, C>(codec, &replay);
        let _ = envelope.reply.send(outcome);
        return Ok(true);
    }
    Ok(None) => {}
    Err(error) => {
        let _ = envelope.reply.send(Err(RuntimeError::from_store_error(error)));
        return Ok(true);
    }
}
```

Only after both replay checks miss should aggregate cache lookup or rehydration happen.

### Tenant-Scoped Store Calls
**Source:** `crates/es-runtime/src/store.rs`
**Apply to:** `shard.rs`, `runtime_flow.rs` fake store
```rust
fn load_rehydration(
    &self,
    tenant_id: &es_core::TenantId,
    stream_id: &es_core::StreamId,
) -> BoxFuture<'_, es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>>;

fn lookup_command_replay(
    &self,
    tenant_id: &es_core::TenantId,
    idempotency_key: &str,
) -> BoxFuture<'_, es_store_postgres::StoreResult<Option<es_store_postgres::CommandReplayRecord>>>;
```

Fake stores should record tenant and stream inputs. Production store API already has the correct tenant-scoped boundary.

### Commit-Gated State Mutation
**Source:** `crates/es-runtime/src/shard.rs`
**Apply to:** `shard.rs`, `runtime_flow.rs`
```rust
Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
    let mut staged_state = current_state;
    for event in &decision.events {
        A::apply(&mut staged_state, event);
    }
    self.cache
        .commit_state(envelope.stream_id.clone(), staged_state);
    self.dedupe.record(dedupe_key, DedupeRecord { replay });
    let _ = envelope
        .reply
        .send(Ok(CommandOutcome::new(reply, committed)));
}
```

Cache mutation after a successful append remains the invariant. Phase 9 only changes the cache key passed to `commit_state`.

## No Analog Found

All files in Phase 9 have close local analogs. No planner fallback to research-only examples is required.

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|

## Metadata

**Analog search scope:** `crates/es-runtime/src`, `crates/es-runtime/tests`
**Files scanned:** 10 runtime source/test files
**Pattern extraction date:** 2026-04-20

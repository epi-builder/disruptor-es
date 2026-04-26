//! Runtime command flow integration tests.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_kernel::{Aggregate, Decision};
use es_runtime::{
    AggregateCacheKey, CommandEngine, CommandEngineConfig, CommandEnvelope, DedupeKey,
    DedupeRecord, RuntimeError, RuntimeEventCodec, RuntimeEventStore, ShardId, ShardState,
};
use es_store_postgres::{
    AppendOutcome, AppendRequest, CommandReplayRecord, CommandReplyPayload, CommittedAppend,
    NewEvent, RehydrationBatch, SnapshotRecord, StoreError, StoredEvent,
};
use futures::future::BoxFuture;
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::{Notify, oneshot};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CounterState {
    value: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CounterCommand {
    Add {
        stream_id: &'static str,
        amount: i64,
    },
    Reject {
        stream_id: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CounterEvent {
    Added(i64),
}

#[derive(Clone, Default)]
struct CounterCodec {
    fail_encode: bool,
}

struct CounterAggregate;

struct NoStreamCounterAggregate;

impl Aggregate for CounterAggregate {
    type State = CounterState;
    type Command = CounterCommand;
    type Event = CounterEvent;
    type Reply = i64;
    type Error = &'static str;

    fn stream_id(command: &Self::Command) -> StreamId {
        match command {
            CounterCommand::Add { stream_id, .. } => StreamId::new(*stream_id).expect("stream id"),
            CounterCommand::Reject { stream_id } => StreamId::new(*stream_id).expect("stream id"),
        }
    }

    fn partition_key(command: &Self::Command) -> es_core::PartitionKey {
        match command {
            CounterCommand::Add { stream_id, .. } => {
                es_core::PartitionKey::new(*stream_id).expect("partition key")
            }
            CounterCommand::Reject { stream_id } => {
                es_core::PartitionKey::new(*stream_id).expect("partition key")
            }
        }
    }

    fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
        ExpectedRevision::Any
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        match command {
            CounterCommand::Add { amount, .. } => Ok(Decision::new(
                vec![CounterEvent::Added(amount)],
                state.value + amount,
            )),
            CounterCommand::Reject { .. } => Err("rejected by domain"),
        }
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        match event {
            CounterEvent::Added(amount) => {
                state.value += amount;
            }
        }
    }
}

impl Aggregate for NoStreamCounterAggregate {
    type State = CounterState;
    type Command = CounterCommand;
    type Event = CounterEvent;
    type Reply = i64;
    type Error = &'static str;

    fn stream_id(command: &Self::Command) -> StreamId {
        CounterAggregate::stream_id(command)
    }

    fn partition_key(command: &Self::Command) -> es_core::PartitionKey {
        CounterAggregate::partition_key(command)
    }

    fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
        ExpectedRevision::NoStream
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        CounterAggregate::decide(state, command, metadata)
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        CounterAggregate::apply(state, event);
    }
}

impl RuntimeEventCodec<CounterAggregate> for CounterCodec {
    fn encode(
        &self,
        event: &CounterEvent,
        _metadata: &CommandMetadata,
    ) -> es_runtime::RuntimeResult<NewEvent> {
        if self.fail_encode {
            return Err(RuntimeError::Codec {
                message: "encode failed".to_owned(),
            });
        }

        match event {
            CounterEvent::Added(amount) => NewEvent::new(
                Uuid::from_u128(*amount as u128 + 100),
                "CounterAdded",
                1,
                json!({ "amount": amount }),
                json!({ "codec": "counter" }),
            )
            .map_err(RuntimeError::from_store_error),
        }
    }

    fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<CounterEvent> {
        let amount = stored
            .payload
            .get("amount")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| RuntimeError::Codec {
                message: "missing amount".to_owned(),
            })?;

        Ok(CounterEvent::Added(amount))
    }

    fn decode_snapshot(
        &self,
        snapshot: &SnapshotRecord,
    ) -> es_runtime::RuntimeResult<CounterState> {
        let value = snapshot
            .state_payload
            .get("value")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| RuntimeError::Codec {
                message: "missing snapshot value".to_owned(),
            })?;

        Ok(CounterState { value })
    }

    fn encode_reply(&self, reply: &i64) -> es_runtime::RuntimeResult<CommandReplyPayload> {
        CommandReplyPayload::new("counter_reply", 1, json!({ "value": reply }))
            .map_err(RuntimeError::from_store_error)
    }

    fn decode_reply(&self, payload: &CommandReplyPayload) -> es_runtime::RuntimeResult<i64> {
        if payload.reply_type != "counter_reply" {
            return Err(RuntimeError::Codec {
                message: format!("unexpected reply type {}", payload.reply_type),
            });
        }
        if payload.schema_version != 1 {
            return Err(RuntimeError::Codec {
                message: format!("unexpected reply schema version {}", payload.schema_version),
            });
        }

        payload
            .payload
            .get("value")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| RuntimeError::Codec {
                message: "missing counter reply value".to_owned(),
            })
    }
}

impl RuntimeEventCodec<NoStreamCounterAggregate> for CounterCodec {
    fn encode(
        &self,
        event: &CounterEvent,
        metadata: &CommandMetadata,
    ) -> es_runtime::RuntimeResult<NewEvent> {
        <Self as RuntimeEventCodec<CounterAggregate>>::encode(self, event, metadata)
    }

    fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<CounterEvent> {
        <Self as RuntimeEventCodec<CounterAggregate>>::decode(self, stored)
    }

    fn decode_snapshot(
        &self,
        snapshot: &SnapshotRecord,
    ) -> es_runtime::RuntimeResult<CounterState> {
        <Self as RuntimeEventCodec<CounterAggregate>>::decode_snapshot(self, snapshot)
    }

    fn encode_reply(&self, reply: &i64) -> es_runtime::RuntimeResult<CommandReplyPayload> {
        <Self as RuntimeEventCodec<CounterAggregate>>::encode_reply(self, reply)
    }

    fn decode_reply(&self, payload: &CommandReplyPayload) -> es_runtime::RuntimeResult<i64> {
        <Self as RuntimeEventCodec<CounterAggregate>>::decode_reply(self, payload)
    }
}

#[test]
fn command_replay_contract_round_trips_counter_reply() {
    let codec = CounterCodec::default();
    let payload = <CounterCodec as RuntimeEventCodec<CounterAggregate>>::encode_reply(&codec, &42)
        .expect("encoded reply");
    let decoded: i64 =
        <CounterCodec as RuntimeEventCodec<CounterAggregate>>::decode_reply(&codec, &payload)
            .expect("decoded reply");

    assert_eq!(42, decoded);
}

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
    rehydration_calls: Mutex<Vec<(TenantId, StreamId)>>,
    tenant_rehydration: Mutex<VecDeque<((TenantId, StreamId), RehydrationBatch)>>,
    rehydration_error: Mutex<Option<StoreError>>,
    append_gates: Mutex<VecDeque<oneshot::Receiver<()>>>,
    append_started: Notify,
}

impl FakeStore {
    fn committed() -> Self {
        Self::with_append_result(Ok(AppendOutcome::Committed(committed_append(1))))
    }

    fn duplicate() -> Self {
        Self::with_append_result(Ok(AppendOutcome::Duplicate(committed_append(1))))
    }

    fn with_append_result(result: Result<AppendOutcome, StoreError>) -> Self {
        Self {
            inner: Arc::new(FakeStoreInner {
                append_requests: Mutex::new(Vec::new()),
                append_outcomes: Mutex::new(VecDeque::from([result])),
                command_replay: Mutex::new(VecDeque::new()),
                lookup_count: Mutex::new(0),
                rehydration: Mutex::new(RehydrationBatch {
                    snapshot: None,
                    events: Vec::new(),
                }),
                rehydration_calls: Mutex::new(Vec::new()),
                tenant_rehydration: Mutex::new(VecDeque::new()),
                rehydration_error: Mutex::new(None),
                append_gates: Mutex::new(VecDeque::new()),
                append_started: Notify::new(),
            }),
        }
    }

    fn with_delayed_commit(receiver: oneshot::Receiver<()>) -> Self {
        let store = Self::committed();
        store
            .inner
            .append_gates
            .lock()
            .expect("append gates")
            .push_back(receiver);
        store
    }

    fn push_append_gate(&self, receiver: oneshot::Receiver<()>) {
        self.inner
            .append_gates
            .lock()
            .expect("append gates")
            .push_back(receiver);
    }

    fn set_rehydration(&self, rehydration: RehydrationBatch) {
        *self.inner.rehydration.lock().expect("rehydration") = rehydration;
    }

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

    fn lookup_count(&self) -> usize {
        *self.inner.lookup_count.lock().expect("lookup count")
    }

    async fn wait_for_append_start(&self) {
        self.inner.append_started.notified().await;
    }

    fn appended_len(&self) -> usize {
        self.inner
            .append_requests
            .lock()
            .expect("append requests")
            .len()
    }
}

impl RuntimeEventStore for FakeStore {
    fn append(
        &self,
        request: AppendRequest,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<AppendOutcome>> {
        let receiver = self
            .inner
            .append_gates
            .lock()
            .expect("append gates")
            .pop_front();
        self.inner
            .append_requests
            .lock()
            .expect("append requests")
            .push(request);
        let result = self
            .inner
            .append_outcomes
            .lock()
            .expect("append outcomes")
            .pop_front()
            .unwrap_or_else(|| Ok(AppendOutcome::Committed(committed_append(1))));
        self.inner.append_started.notify_waiters();

        Box::pin(async move {
            if let Some(receiver) = receiver {
                let _ = receiver.await;
            }
            result
        })
    }

    fn load_rehydration(
        &self,
        tenant_id: &TenantId,
        stream_id: &StreamId,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<RehydrationBatch>> {
        self.inner
            .rehydration_calls
            .lock()
            .expect("rehydration calls")
            .push((tenant_id.clone(), stream_id.clone()));
        let error = self
            .inner
            .rehydration_error
            .lock()
            .expect("rehydration error")
            .take();
        let tenant_batch = {
            let mut tenant_rehydration = self
                .inner
                .tenant_rehydration
                .lock()
                .expect("tenant rehydration");
            tenant_rehydration
                .iter()
                .position(|((tenant, stream), _)| tenant == tenant_id && stream == stream_id)
                .and_then(|index| tenant_rehydration.remove(index).map(|(_, batch)| batch))
        };
        let batch = tenant_batch
            .unwrap_or_else(|| self.inner.rehydration.lock().expect("rehydration").clone());

        Box::pin(async move {
            if let Some(error) = error {
                Err(error)
            } else {
                Ok(batch)
            }
        })
    }

    fn lookup_command_replay(
        &self,
        _tenant_id: &TenantId,
        _idempotency_key: &str,
    ) -> BoxFuture<'_, es_store_postgres::StoreResult<Option<CommandReplayRecord>>> {
        *self.inner.lookup_count.lock().expect("lookup count") += 1;
        let replay = self
            .inner
            .command_replay
            .lock()
            .expect("command replay")
            .pop_front()
            .unwrap_or(None);

        Box::pin(async move { Ok(replay) })
    }
}

fn tenant_id_for(value: &'static str) -> TenantId {
    TenantId::new(value).expect("tenant id")
}

fn metadata_for(tenant: &'static str) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: tenant_id_for(tenant),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn cache_key_for(tenant: &'static str, stream: &'static str) -> AggregateCacheKey {
    AggregateCacheKey {
        tenant_id: tenant_id_for(tenant),
        stream_id: StreamId::new(stream).expect("stream id"),
    }
}

fn envelope_for(
    tenant: &'static str,
    stream: &'static str,
    idempotency_key: &'static str,
    amount: i64,
) -> (
    CommandEnvelope<CounterAggregate>,
    oneshot::Receiver<es_runtime::RuntimeResult<es_runtime::CommandOutcome<i64>>>,
) {
    let (reply, receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<CounterAggregate>::new(
        CounterCommand::Add {
            stream_id: stream,
            amount,
        },
        metadata_for(tenant),
        idempotency_key,
        reply,
    )
    .expect("command envelope");

    (envelope, receiver)
}

fn tenant_id() -> TenantId {
    tenant_id_for("tenant-a")
}

fn metadata() -> CommandMetadata {
    metadata_for("tenant-a")
}

fn envelope(
    amount: i64,
) -> (
    CommandEnvelope<CounterAggregate>,
    oneshot::Receiver<es_runtime::RuntimeResult<es_runtime::CommandOutcome<i64>>>,
) {
    envelope_for("tenant-a", "counter-1", "idem-1", amount)
}

fn rejecting_envelope() -> (
    CommandEnvelope<CounterAggregate>,
    oneshot::Receiver<es_runtime::RuntimeResult<es_runtime::CommandOutcome<i64>>>,
) {
    let (reply, receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<CounterAggregate>::new(
        CounterCommand::Reject {
            stream_id: "counter-1",
        },
        metadata(),
        "idem-1",
        reply,
    )
    .expect("command envelope");

    (envelope, receiver)
}

fn no_stream_envelope(
    amount: i64,
) -> (
    CommandEnvelope<NoStreamCounterAggregate>,
    oneshot::Receiver<es_runtime::RuntimeResult<es_runtime::CommandOutcome<i64>>>,
) {
    let (reply, receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<NoStreamCounterAggregate>::new(
        CounterCommand::Add {
            stream_id: "counter-1",
            amount,
        },
        metadata(),
        "idem-no-stream",
        reply,
    )
    .expect("command envelope");

    (envelope, receiver)
}

fn record_handoff(
    state: &mut ShardState<CounterAggregate>,
    envelope: CommandEnvelope<CounterAggregate>,
) {
    state.record_released_handoff(0, envelope);
}

fn committed_append(position: i64) -> CommittedAppend {
    CommittedAppend {
        stream_id: StreamId::new("counter-1").expect("stream id"),
        first_revision: StreamRevision::new(position as u64),
        last_revision: StreamRevision::new(position as u64),
        global_positions: vec![position],
        event_ids: vec![Uuid::from_u128(position as u128)],
    }
}

fn command_replay_record(position: i64, reply: i64) -> CommandReplayRecord {
    CommandReplayRecord {
        append: committed_append(position),
        reply: CommandReplyPayload::new("counter_reply", 1, json!({ "value": reply }))
            .expect("reply payload"),
    }
}

fn dedupe_key() -> DedupeKey {
    DedupeKey {
        tenant_id: tenant_id(),
        idempotency_key: "idem-1".to_owned(),
    }
}

fn warm_cache(state: &mut ShardState<CounterAggregate>, value: i64) {
    state.cache_mut().commit_state(
        cache_key_for("tenant-a", "counter-1"),
        CounterState { value },
    );
}

fn expect_runtime_error(
    result: es_runtime::RuntimeResult<es_runtime::CommandOutcome<i64>>,
) -> RuntimeError {
    match result {
        Ok(_) => panic!("expected runtime error"),
        Err(error) => error,
    }
}

fn stored_event(position: i64, amount: i64) -> StoredEvent {
    StoredEvent {
        global_position: position,
        stream_id: StreamId::new("counter-1").expect("stream id"),
        stream_revision: StreamRevision::new(position as u64),
        event_id: Uuid::from_u128(position as u128),
        event_type: "CounterAdded".to_owned(),
        schema_version: 1,
        payload: json!({ "amount": amount }),
        metadata: json!({}),
        tenant_id: tenant_id(),
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        recorded_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

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

    assert_eq!(
        Some(&CounterState { value: 3 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn reply_stays_blocked_until_append_gate_opens() {
    let (release_append, wait_for_release) = oneshot::channel();
    let store = FakeStore::with_delayed_commit(wait_for_release);
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(5);
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
        "reply resolved before append gate opened"
    );

    release_append.send(()).expect("release append");
    let state = task.await.expect("task joined");
    assert_eq!(
        Some(&CounterState { value: 5 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn no_stream_cache_miss_skips_rehydration_before_append() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut no_stream_state = ShardState::<NoStreamCounterAggregate>::new(ShardId::new(0));
    let (no_stream_envelope, no_stream_receiver) = no_stream_envelope(7);
    no_stream_state.record_released_handoff(0, no_stream_envelope);

    assert!(
        no_stream_state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );
    no_stream_receiver.await.expect("reply").expect("success");
    assert_eq!(0, store.rehydration_calls().len());

    let fallback_store = FakeStore::committed();
    let mut regular_state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (regular_envelope, regular_receiver) = envelope(3);
    record_handoff(&mut regular_state, regular_envelope);

    assert!(
        regular_state
            .process_next_handoff(&fallback_store, &codec)
            .await
            .expect("processed")
    );
    regular_receiver.await.expect("reply").expect("success");
    assert!(
        !fallback_store.rehydration_calls().is_empty(),
        "ExpectedRevision::Any should still rehydrate on a cache miss"
    );
}

#[tokio::test]
async fn parallel_shard_workers_process_distinct_routes_concurrently() {
    let (release_first, wait_first) = oneshot::channel();
    let (release_second, wait_second) = oneshot::channel();
    let store = FakeStore::with_delayed_commit(wait_first);
    store.push_append_gate(wait_second);
    store
        .inner
        .append_outcomes
        .lock()
        .expect("append outcomes")
        .push_back(Ok(AppendOutcome::Committed(committed_append(2))));

    let codec = CounterCodec::default();
    let config = CommandEngineConfig::new(2, 8, 8).expect("config");
    let mut engine = CommandEngine::<CounterAggregate, _, _>::new(config, store.clone(), codec)
        .expect("engine");

    let (first_envelope, first_reply) = envelope_for("tenant-a", "counter-1", "idem-1", 1);
    let (second_envelope, second_reply) = envelope_for("tenant-a", "counter-2", "idem-2", 2);
    let first_gateway = engine.gateway();
    let second_gateway = first_gateway.clone();
    first_gateway.try_submit(first_envelope).expect("submit first");
    second_gateway
        .try_submit(second_envelope)
        .expect("submit second");

    let engine_task = tokio::spawn(async move {
        let first = engine.process_one().await.expect("first process");
        let second = engine.process_one().await.expect("second process");
        (engine, first, second)
    });

    store.wait_for_append_start().await;
    tokio::time::sleep(Duration::from_millis(20)).await;

    assert_eq!(
        2,
        store.appended_len(),
        "both shard routes should reach append_started before replies release"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), first_reply)
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), second_reply)
            .await
            .is_err()
    );

    release_first.send(()).expect("release first");
    release_second.send(()).expect("release second");
    let (_engine, first_processed, second_processed) = engine_task.await.expect("join");
    assert!(first_processed);
    assert!(second_processed);
}

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
    assert!(matches!(
        error,
        RuntimeError::Conflict {
            stream_id,
            expected,
            actual: Some(1),
        } if stream_id == "counter-1" && expected == "exact 99"
    ));
    assert_eq!(1, store.appended_len());
    assert_eq!(0, state.dedupe().len());
    assert_eq!(
        Some(&CounterState { value: 10 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn domain_error_does_not_append_or_mutate_cache() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    warm_cache(&mut state, 10);
    let (envelope, receiver) = rejecting_envelope();
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let error = expect_runtime_error(receiver.await.expect("reply"));
    assert!(matches!(error, RuntimeError::Domain { message } if message == "rejected by domain"));
    assert_eq!(0, store.appended_len());
    assert_eq!(0, state.dedupe().len());
    assert_eq!(
        Some(&CounterState { value: 10 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn codec_error_does_not_append_or_mutate_cache() {
    let store = FakeStore::committed();
    let codec = CounterCodec { fail_encode: true };
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
    assert!(matches!(error, RuntimeError::Codec { message } if message == "encode failed"));
    assert_eq!(0, store.appended_len());
    assert_eq!(0, state.dedupe().len());
    assert_eq!(
        Some(&CounterState { value: 10 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn rehydration_error_does_not_decide_append_or_mutate_cache() {
    let store = FakeStore::committed();
    store.set_rehydration_error(StoreError::DedupeConflict {
        tenant_id: "tenant-a".to_owned(),
        idempotency_key: "idem-1".to_owned(),
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

    let error = expect_runtime_error(receiver.await.expect("reply"));
    assert!(matches!(
        error,
        RuntimeError::Store(StoreError::DedupeConflict { .. })
    ));
    assert_eq!(0, store.appended_len());
    assert_eq!(0, state.dedupe().len());
    assert!(state.cache().is_empty());
}

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
    assert_eq!(
        Some(&CounterState { value: 8 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn same_stream_different_tenant_rehydrates_independently() {
    let store = FakeStore::with_append_result(Ok(AppendOutcome::Committed(committed_append(1))));
    store.set_tenant_rehydration(
        tenant_id_for("tenant-a"),
        StreamId::new("counter-1").expect("stream id"),
        RehydrationBatch {
            snapshot: None,
            events: vec![],
        },
    );
    store.set_tenant_rehydration(
        tenant_id_for("tenant-b"),
        StreamId::new("counter-1").expect("stream id"),
        RehydrationBatch {
            snapshot: None,
            events: vec![],
        },
    );
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));

    let (tenant_a, tenant_a_receiver) = envelope_for("tenant-a", "counter-1", "idem-a", 3);
    record_handoff(&mut state, tenant_a);
    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("tenant a processed")
    );
    assert_eq!(
        3,
        tenant_a_receiver
            .await
            .expect("reply")
            .expect("success")
            .reply
    );

    let (tenant_b, tenant_b_receiver) = envelope_for("tenant-b", "counter-1", "idem-b", 2);
    record_handoff(&mut state, tenant_b);
    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("tenant b processed")
    );
    assert_eq!(
        2,
        tenant_b_receiver
            .await
            .expect("reply")
            .expect("success")
            .reply
    );

    assert_eq!(
        vec![
            (
                tenant_id_for("tenant-a"),
                StreamId::new("counter-1").expect("stream id")
            ),
            (
                tenant_id_for("tenant-b"),
                StreamId::new("counter-1").expect("stream id")
            ),
        ],
        store.rehydration_calls()
    );
}

#[tokio::test]
async fn same_stream_different_tenant_preserves_domain_state() {
    let store = FakeStore::with_append_result(Ok(AppendOutcome::Committed(committed_append(1))));
    store.set_tenant_rehydration(
        tenant_id_for("tenant-a"),
        StreamId::new("counter-1").expect("stream id"),
        RehydrationBatch {
            snapshot: None,
            events: vec![stored_event(1, 5)],
        },
    );
    store.set_tenant_rehydration(
        tenant_id_for("tenant-b"),
        StreamId::new("counter-1").expect("stream id"),
        RehydrationBatch {
            snapshot: None,
            events: vec![stored_event(40, 40)],
        },
    );
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));

    let (tenant_a, tenant_a_receiver) = envelope_for("tenant-a", "counter-1", "idem-a", 3);
    record_handoff(&mut state, tenant_a);
    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("tenant a processed")
    );
    assert_eq!(
        8,
        tenant_a_receiver
            .await
            .expect("reply")
            .expect("success")
            .reply
    );

    let (tenant_b, tenant_b_receiver) = envelope_for("tenant-b", "counter-1", "idem-b", 2);
    record_handoff(&mut state, tenant_b);
    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("tenant b processed")
    );
    assert_eq!(
        42,
        tenant_b_receiver
            .await
            .expect("reply")
            .expect("success")
            .reply
    );
    assert_ne!(
        Some(&CounterState { value: 10 }),
        state.cache().get(&cache_key_for("tenant-b", "counter-1"))
    );
}

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
    assert_eq!(vec![7], outcome.append.global_positions);
    assert_eq!(0, store.appended_len());
    assert_eq!(0, store.lookup_count());
    assert_eq!(
        Some(&CounterState { value: 10 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn duplicate_replay_returns_original_reply_after_state_mutation() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    warm_cache(&mut state, 50);
    state.dedupe_mut().record(
        dedupe_key(),
        DedupeRecord {
            replay: command_replay_record(8, 5),
        },
    );
    let (envelope, receiver) = envelope(20);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(5, outcome.reply);
    assert_eq!(vec![8], outcome.append.global_positions);
    assert_eq!(0, store.appended_len());
    assert_eq!(
        Some(&CounterState { value: 50 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

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
    assert_eq!(vec![9], outcome.append.global_positions);
    assert_eq!(0, store.appended_len());
    assert_eq!(1, store.lookup_count());
    assert!(state.cache().is_empty());
    assert_eq!(1, state.dedupe().len());
}

#[tokio::test]
async fn duplicate_append_returns_successful_command_outcome() {
    let store = FakeStore::duplicate();
    store.set_command_replay_sequence(vec![None, Some(command_replay_record(1, 3))]);
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
    assert_eq!(3, outcome.reply);
    assert_eq!(vec![1], outcome.append.global_positions);
    assert_eq!(
        Some(&CounterState { value: 0 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
    assert_eq!(1, state.dedupe().len());
}

#[tokio::test]
async fn duplicate_append_branch_uses_stored_replay_not_fresh_decision_reply() {
    let store = FakeStore::duplicate();
    store.set_command_replay_sequence(vec![None, Some(command_replay_record(12, 44))]);
    store.set_rehydration(RehydrationBatch {
        snapshot: None,
        events: vec![stored_event(12, 40)],
    });
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

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(44, outcome.reply);
    assert_eq!(vec![12], outcome.append.global_positions);
    assert_eq!(1, store.appended_len());
    assert_eq!(2, store.lookup_count());
    assert_eq!(
        Some(&CounterState { value: 40 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn duplicate_after_warmed_cache_refreshes_from_durable_rehydration() {
    let store = FakeStore::duplicate();
    store.set_command_replay_sequence(vec![None, Some(command_replay_record(1, 13))]);
    store.set_tenant_rehydration(
        tenant_id_for("tenant-a"),
        StreamId::new("counter-1").expect("stream id"),
        RehydrationBatch {
            snapshot: None,
            events: vec![stored_event(1, 25)],
        },
    );
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

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(13, outcome.reply);
    assert_eq!(
        Some(&CounterState { value: 25 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
    assert_eq!(
        vec![(
            tenant_id_for("tenant-a"),
            StreamId::new("counter-1").expect("stream id")
        ),],
        store.rehydration_calls()
    );
}

#[tokio::test]
async fn reply_drop_after_append_still_advances_cache_and_dedupe() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(3);
    drop(receiver);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    assert_eq!(1, store.appended_len());
    assert_eq!(1, state.dedupe().len());
    assert_eq!(
        Some(&CounterState { value: 3 }),
        state.cache().get(&cache_key_for("tenant-a", "counter-1"))
    );
}

#[tokio::test]
async fn runtime_engine_processes_submitted_command_end_to_end_after_durable_commit() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut engine: CommandEngine<CounterAggregate, _, _> = CommandEngine::new(
        CommandEngineConfig::new(1, 4, 4).expect("config"),
        store,
        codec,
    )
    .expect("engine");
    let gateway = engine.gateway();
    let (envelope, receiver) = envelope(3);

    gateway.try_submit(envelope).expect("submitted");
    assert!(engine.process_one().await.expect("processed"));

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(outcome.append.global_positions, vec![1]);
}

#[tokio::test]
async fn runtime_flow_covers_overload_disruptor_handoff_conflict_and_commit_paths() {
    let store = FakeStore::committed();
    let codec = CounterCodec::default();
    let mut engine: CommandEngine<CounterAggregate, _, _> = CommandEngine::new(
        CommandEngineConfig::new(1, 1, 4).expect("config"),
        store,
        codec,
    )
    .expect("engine");
    let gateway = engine.gateway();
    let (accepted, accepted_receiver) = envelope(3);
    let (overloaded, _overloaded_receiver) = envelope(4);

    gateway.try_submit(accepted).expect("first submit accepted");
    let error = gateway
        .try_submit(overloaded)
        .expect_err("second submit overloads ingress");
    assert!(matches!(error, RuntimeError::Overloaded));

    assert!(engine.process_one().await.expect("processed"));
    let outcome = accepted_receiver.await.expect("reply").expect("success");
    assert_eq!(outcome.append.global_positions, vec![1]);

    let conflict_store = FakeStore::with_append_result(Err(StoreError::StreamConflict {
        stream_id: "counter-1".to_owned(),
        expected: "exact 99".to_owned(),
        actual: Some(1),
    }));
    let mut conflict_engine: CommandEngine<CounterAggregate, _, _> = CommandEngine::new(
        CommandEngineConfig::new(1, 1, 4).expect("config"),
        conflict_store,
        CounterCodec::default(),
    )
    .expect("conflict engine");
    let conflict_gateway = conflict_engine.gateway();
    let (conflicting, conflict_receiver) = envelope(3);

    conflict_gateway
        .try_submit(conflicting)
        .expect("conflict submit accepted");
    assert!(conflict_engine.process_one().await.expect("processed"));

    let error = expect_runtime_error(conflict_receiver.await.expect("reply"));
    assert!(matches!(
        error,
        RuntimeError::Conflict {
            stream_id,
            expected,
            actual: Some(1),
        } if stream_id == "counter-1" && expected == "exact 99"
    ));
}

//! Runtime command flow integration tests.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_kernel::{Aggregate, Decision};
use es_runtime::{
    CommandEngine, CommandEngineConfig, CommandEnvelope, RuntimeError, RuntimeEventCodec,
    RuntimeEventStore, ShardId, ShardState,
};
use es_store_postgres::{
    AppendOutcome, AppendRequest, CommittedAppend, NewEvent, RehydrationBatch, SnapshotRecord,
    StoreError, StoredEvent,
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
}

#[test]
fn command_replay_contract_round_trips_counter_reply() {
    let codec = CounterCodec::default();
    let payload = codec.encode_reply(&42).expect("encoded reply");
    let decoded = codec.decode_reply(&payload).expect("decoded reply");

    assert_eq!(42, decoded);
}

#[derive(Clone)]
struct FakeStore {
    inner: Arc<FakeStoreInner>,
}

struct FakeStoreInner {
    append_requests: Mutex<Vec<AppendRequest>>,
    append_outcomes: Mutex<VecDeque<Result<AppendOutcome, StoreError>>>,
    rehydration: Mutex<RehydrationBatch>,
    rehydration_error: Mutex<Option<StoreError>>,
    append_gate: Mutex<Option<oneshot::Receiver<()>>>,
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
                rehydration: Mutex::new(RehydrationBatch {
                    snapshot: None,
                    events: Vec::new(),
                }),
                rehydration_error: Mutex::new(None),
                append_gate: Mutex::new(None),
                append_started: Notify::new(),
            }),
        }
    }

    fn with_delayed_commit(receiver: oneshot::Receiver<()>) -> Self {
        let store = Self::committed();
        *store.inner.append_gate.lock().expect("append gate") = Some(receiver);
        store
    }

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
        let receiver = self.inner.append_gate.lock().expect("append gate").take();
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
}

fn tenant_id() -> TenantId {
    TenantId::new("tenant-a").expect("tenant id")
}

fn metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: tenant_id(),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn envelope(
    amount: i64,
) -> (
    CommandEnvelope<CounterAggregate>,
    oneshot::Receiver<es_runtime::RuntimeResult<es_runtime::CommandOutcome<i64>>>,
) {
    let (reply, receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<CounterAggregate>::new(
        CounterCommand::Add {
            stream_id: "counter-1",
            amount,
        },
        metadata(),
        "idem-1",
        reply,
    )
    .expect("command envelope");

    (envelope, receiver)
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

fn warm_cache(state: &mut ShardState<CounterAggregate>, value: i64) {
    state.cache_mut().commit_state(
        StreamId::new("counter-1").expect("stream id"),
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
    );
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
    );
}

#[tokio::test]
async fn duplicate_append_returns_successful_command_outcome() {
    let store = FakeStore::duplicate();
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
    );
    assert_eq!(1, state.dedupe().len());
}

#[tokio::test]
async fn duplicate_after_warmed_cache_does_not_apply_newly_decided_events() {
    let store = FakeStore::duplicate();
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
        Some(&CounterState { value: 10 }),
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
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
        state
            .cache()
            .get(&StreamId::new("counter-1").expect("stream id"))
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

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision};
use es_kernel::{Aggregate, Decision};
use es_runtime::{
    AggregateCache, CommandEnvelope, DedupeCache, DedupeKey, DedupeRecord, DisruptorPath,
    RoutedCommand, RuntimeError, ShardHandle, ShardId, ShardState,
};
use es_store_postgres::{CommandReplayRecord, CommandReplyPayload, CommittedAppend};
use time::OffsetDateTime;
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CounterState {
    value: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CounterCommand {
    stream_id: &'static str,
    amount: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CounterEvent {
    amount: i64,
}

struct CounterAggregate;

impl Aggregate for CounterAggregate {
    type State = CounterState;
    type Command = CounterCommand;
    type Event = CounterEvent;
    type Reply = i64;
    type Error = &'static str;

    fn stream_id(command: &Self::Command) -> StreamId {
        StreamId::new(command.stream_id).expect("stream id")
    }

    fn partition_key(command: &Self::Command) -> es_core::PartitionKey {
        es_core::PartitionKey::new(command.stream_id).expect("partition key")
    }

    fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
        ExpectedRevision::Any
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &es_core::CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        Ok(Decision::new(
            vec![CounterEvent {
                amount: command.amount,
            }],
            state.value + command.amount,
        ))
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        state.value += event.amount;
    }
}

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

fn routed_command(
    shard_id: ShardId,
    tenant: &'static str,
    stream: &'static str,
    idempotency_key: &'static str,
    amount: i64,
) -> RoutedCommand<CounterAggregate> {
    let (reply, _receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<CounterAggregate>::new(
        CounterCommand {
            stream_id: stream,
            amount,
        },
        metadata(tenant),
        idempotency_key,
        reply,
    )
    .expect("command envelope");

    RoutedCommand { shard_id, envelope }
}

fn committed_append(stream_id: StreamId) -> CommittedAppend {
    CommittedAppend {
        stream_id,
        first_revision: StreamRevision::new(1),
        last_revision: StreamRevision::new(1),
        global_positions: vec![10],
        event_ids: vec![Uuid::from_u128(10)],
    }
}

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

#[test]
fn shard_dedupe_cache_records_tenant_scoped_committed_append() {
    let mut cache = DedupeCache::new();
    let key = DedupeKey {
        tenant_id: tenant_id("tenant-a"),
        idempotency_key: "idem-1".to_owned(),
    };
    let record = DedupeRecord {
        replay: CommandReplayRecord {
            append: committed_append(stream_id("counter-1")),
            reply: CommandReplyPayload::new("counter_reply", 1, serde_json::json!({ "value": 7 }))
                .expect("reply payload"),
        },
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

#[test]
fn disruptor_path_rejects_zero_ring_size() {
    let error =
        DisruptorPath::new(ShardId::new(3), 0, || 0_u64).expect_err("zero ring size rejected");

    assert!(matches!(error, RuntimeError::InvalidRingSize));
}

#[test]
fn disruptor_path_returns_shard_overloaded_when_ring_is_full() {
    let mut path = DisruptorPath::new(ShardId::new(7), 1, || 0_u64).expect("disruptor path");

    path.try_publish(10).expect("first publish");
    let error = path.try_publish(11).expect_err("second publish overloads");

    assert!(matches!(
        error,
        RuntimeError::ShardOverloaded { shard_id: 7 }
    ));
}

#[test]
fn disruptor_path_releases_published_tokens_through_consumer_path() {
    let mut path = DisruptorPath::new(ShardId::new(1), 2, || 0_u64).expect("disruptor path");

    let sequence = path.try_publish(42).expect("publish");

    assert_eq!(
        vec![es_runtime::ReleasedHandoff {
            sequence,
            event: 42
        }],
        path.poll_released()
    );
    assert!(path.poll_released().is_empty());
}

#[test]
fn shard_state_records_ordered_handoffs() {
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(1));

    assert_eq!(1, state.shard_id().value());

    state.record_released_handoff(
        2,
        routed_command(ShardId::new(1), "tenant-a", "counter-2", "b", 2).envelope,
    );
    state.record_released_handoff(
        1,
        routed_command(ShardId::new(1), "tenant-a", "counter-1", "a", 1).envelope,
    );

    assert_eq!(2, state.pending_handoffs());
    assert_eq!(1, state.pop_handoff().expect("first handoff").sequence);
    assert_eq!(2, state.pop_handoff().expect("second handoff").sequence);
    assert!(state.pop_handoff().is_none());
}

#[test]
fn shard_handle_publishes_routed_command_before_release() {
    let mut handle = ShardHandle::<CounterAggregate>::new(ShardId::new(1), 4).expect("handle");

    let sequence = handle
        .accept_routed(routed_command(
            ShardId::new(1),
            "tenant-a",
            "counter-1",
            "idem-1",
            1,
        ))
        .expect("accepted");

    assert_eq!(0, sequence);
    assert_eq!(1, handle.pending_len());
    assert_eq!(0, handle.state().pending_handoffs());
}

#[test]
fn shard_command_cannot_be_processed_until_disruptor_release_is_drained() {
    let mut handle = ShardHandle::<CounterAggregate>::new(ShardId::new(1), 4).expect("handle");

    handle
        .accept_routed(routed_command(
            ShardId::new(1),
            "tenant-a",
            "counter-1",
            "idem-1",
            1,
        ))
        .expect("accepted");

    assert!(handle.state_mut().pop_handoff().is_none());

    assert_eq!(1, handle.drain_released_handoffs().expect("drained"));
    let handoff = handle.state_mut().pop_handoff().expect("released handoff");

    assert_eq!(0, handoff.sequence);
    assert_eq!("tenant-a", handoff.envelope.metadata.tenant_id.as_str());
    assert_eq!("counter-1", handoff.envelope.stream_id.as_str());
    assert_eq!("idem-1", handoff.envelope.idempotency_key);
}

#[test]
fn shard_pending_keeps_duplicate_inflight_stream_and_idempotency_commands_distinct() {
    let mut handle = ShardHandle::<CounterAggregate>::new(ShardId::new(1), 4).expect("handle");

    handle
        .accept_routed(routed_command(
            ShardId::new(1),
            "tenant-a",
            "counter-1",
            "same-idem",
            1,
        ))
        .expect("first accepted");
    handle
        .accept_routed(routed_command(
            ShardId::new(1),
            "tenant-a",
            "counter-1",
            "same-idem",
            2,
        ))
        .expect("second accepted");

    assert_eq!(2, handle.pending_len());

    assert_eq!(2, handle.drain_released_handoffs().expect("drained"));
    let first = handle.state_mut().pop_handoff().expect("first handoff");
    let second = handle.state_mut().pop_handoff().expect("second handoff");

    assert_eq!(0, first.sequence);
    assert_eq!(1, second.sequence);
    assert_eq!(1, first.envelope.command.amount);
    assert_eq!(2, second.envelope.command.amount);
}

#[test]
fn shard_pending_keeps_cross_tenant_same_key_commands_distinct() {
    let mut handle = ShardHandle::<CounterAggregate>::new(ShardId::new(1), 4).expect("handle");

    handle
        .accept_routed(routed_command(
            ShardId::new(1),
            "tenant-a",
            "counter-1",
            "same-idem",
            1,
        ))
        .expect("tenant a accepted");
    handle
        .accept_routed(routed_command(
            ShardId::new(1),
            "tenant-b",
            "counter-1",
            "same-idem",
            2,
        ))
        .expect("tenant b accepted");

    assert_eq!(2, handle.pending_len());

    assert_eq!(2, handle.drain_released_handoffs().expect("drained"));
    let first = handle.state_mut().pop_handoff().expect("first handoff");
    let second = handle.state_mut().pop_handoff().expect("second handoff");

    assert_eq!("tenant-a", first.envelope.metadata.tenant_id.as_str());
    assert_eq!("tenant-b", second.envelope.metadata.tenant_id.as_str());
    assert_eq!("same-idem", first.envelope.idempotency_key);
    assert_eq!("same-idem", second.envelope.idempotency_key);
}

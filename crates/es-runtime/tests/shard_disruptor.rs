use es_core::{ExpectedRevision, StreamId, StreamRevision};
use es_kernel::{Aggregate, Decision};
use es_runtime::{AggregateCache, DedupeCache, DedupeKey, DedupeRecord};
use es_store_postgres::CommittedAppend;
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
        append: committed_append(stream_id("counter-1")),
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

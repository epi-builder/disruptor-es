//! Synchronous aggregate kernel contracts for deterministic event-sourced domains.

/// Typed command decision containing events and a reply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision<E, R> {
    /// Events produced by the decision.
    pub events: Vec<E>,
    /// Reply returned to the command caller.
    pub reply: R,
}

impl<E, R> Decision<E, R> {
    /// Creates a decision from typed events and a typed reply.
    pub fn new(events: Vec<E>, reply: R) -> Self {
        Self { events, reply }
    }
}

/// Deterministic aggregate contract implemented by domain aggregate types.
pub trait Aggregate {
    /// Aggregate state type.
    type State: Default + Clone + PartialEq;
    /// Command input type.
    type Command;
    /// Event output type.
    type Event: Clone;
    /// Reply type returned after a successful decision.
    type Reply;
    /// Domain error type returned by decisions.
    type Error;

    /// Returns the stream identifier affected by the command.
    fn stream_id(command: &Self::Command) -> es_core::StreamId;

    /// Returns the ordered partition key for the command.
    fn partition_key(command: &Self::Command) -> es_core::PartitionKey;

    /// Returns the expected stream revision for optimistic concurrency.
    fn expected_revision(command: &Self::Command) -> es_core::ExpectedRevision;

    /// Decides events and reply for a command against current state.
    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &es_core::CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error>;

    /// Applies one event to aggregate state.
    fn apply(state: &mut Self::State, event: &Self::Event);
}

/// Replays events from a default aggregate state.
pub fn replay<A: Aggregate>(events: impl IntoIterator<Item = A::Event>) -> A::State {
    let mut state = A::State::default();
    for event in events {
        A::apply(&mut state, &event);
    }
    state
}

#[cfg(test)]
mod aggregate_kernel_contracts {
    use super::*;

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    struct CounterState {
        value: i32,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum CounterCommand {
        Add(i32),
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum CounterEvent {
        Added(i32),
    }

    struct CounterAggregate;

    impl Aggregate for CounterAggregate {
        type State = CounterState;
        type Command = CounterCommand;
        type Event = CounterEvent;
        type Reply = i32;
        type Error = &'static str;

        fn stream_id(_command: &Self::Command) -> es_core::StreamId {
            es_core::StreamId::new("counter-1").expect("stream id")
        }

        fn partition_key(_command: &Self::Command) -> es_core::PartitionKey {
            es_core::PartitionKey::new("counter-1").expect("partition key")
        }

        fn expected_revision(_command: &Self::Command) -> es_core::ExpectedRevision {
            es_core::ExpectedRevision::Any
        }

        fn decide(
            state: &Self::State,
            command: Self::Command,
            _metadata: &es_core::CommandMetadata,
        ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
            match command {
                CounterCommand::Add(amount) => Ok(Decision::new(
                    vec![CounterEvent::Added(amount)],
                    state.value + amount,
                )),
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

    #[test]
    fn local_aggregate_implements_associated_types() {
        let command = CounterCommand::Add(3);

        assert_eq!("counter-1", CounterAggregate::stream_id(&command).as_str());
        assert_eq!(
            "counter-1",
            CounterAggregate::partition_key(&command).as_str()
        );
        assert_eq!(
            es_core::ExpectedRevision::Any,
            CounterAggregate::expected_revision(&command)
        );
    }

    #[test]
    fn decision_preserves_typed_events_and_reply() {
        let decision = Decision::new(vec![CounterEvent::Added(5)], 5);

        assert_eq!(vec![CounterEvent::Added(5)], decision.events);
        assert_eq!(5, decision.reply);
    }

    #[test]
    fn replay_applies_events_in_order() {
        let state = replay::<CounterAggregate>([
            CounterEvent::Added(2),
            CounterEvent::Added(3),
            CounterEvent::Added(-1),
        ]);

        assert_eq!(CounterState { value: 4 }, state);
    }
}

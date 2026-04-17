use es_kernel::Aggregate;

use crate::{RuntimeError, RuntimeResult};

/// One-shot command reply used by runtime callers.
pub type CommandReply<R> = tokio::sync::oneshot::Sender<RuntimeResult<CommandOutcome<R>>>;

/// Command accepted into the runtime with precomputed routing and concurrency fields.
pub struct CommandEnvelope<A: Aggregate> {
    /// Typed aggregate command.
    pub command: A::Command,
    /// Command metadata, including tenant and trace identifiers.
    pub metadata: es_core::CommandMetadata,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
    /// Stream affected by the command.
    pub stream_id: es_core::StreamId,
    /// Ordered partition key used by the runtime router.
    pub partition_key: es_core::PartitionKey,
    /// Expected stream revision for optimistic concurrency.
    pub expected_revision: es_core::ExpectedRevision,
    /// One-shot reply channel completed after durable append.
    pub reply: CommandReply<A::Reply>,
}

impl<A: Aggregate> CommandEnvelope<A> {
    /// Builds an envelope and derives routing fields from the aggregate contract.
    pub fn new(
        command: A::Command,
        metadata: es_core::CommandMetadata,
        idempotency_key: impl Into<String>,
        reply: CommandReply<A::Reply>,
    ) -> RuntimeResult<Self> {
        let idempotency_key = idempotency_key.into();
        if idempotency_key.is_empty() {
            return Err(RuntimeError::Codec {
                message: "idempotency key cannot be empty".to_owned(),
            });
        }

        let stream_id = A::stream_id(&command);
        let partition_key = A::partition_key(&command);
        let expected_revision = A::expected_revision(&command);

        Ok(Self {
            command,
            metadata,
            idempotency_key,
            stream_id,
            partition_key,
            expected_revision,
            reply,
        })
    }
}

/// Successful command result returned only after durable append succeeds.
pub struct CommandOutcome<R> {
    /// Aggregate reply.
    pub reply: R,
    /// Durable append result assigned by the event store.
    pub append: es_store_postgres::CommittedAppend,
}

impl<R> CommandOutcome<R> {
    /// Creates a command outcome from aggregate reply data and the durable append summary.
    pub fn new(reply: R, append: es_store_postgres::CommittedAppend) -> Self {
        Self { reply, append }
    }
}

/// Runtime boundary for encoding typed aggregate events into durable store DTOs.
pub trait RuntimeEventCodec<A: Aggregate>: Clone + Send + Sync + 'static {
    /// Encodes a typed event for durable storage.
    fn encode(
        &self,
        event: &A::Event,
        metadata: &es_core::CommandMetadata,
    ) -> RuntimeResult<es_store_postgres::NewEvent>;

    /// Decodes a stored event for aggregate replay.
    fn decode(&self, stored: &es_store_postgres::StoredEvent) -> RuntimeResult<A::Event>;

    /// Decodes a stored snapshot for aggregate replay.
    fn decode_snapshot(
        &self,
        snapshot: &es_store_postgres::SnapshotRecord,
    ) -> RuntimeResult<A::State>;
}

#[cfg(test)]
mod tests {
    use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId, TenantId};
    use es_kernel::{Aggregate, Decision};
    use time::OffsetDateTime;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use super::*;

    #[derive(Clone, Debug, Default, PartialEq)]
    struct TestState;

    #[derive(Clone, Debug, PartialEq)]
    struct TestCommand {
        stream_id: &'static str,
        partition_key: &'static str,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct TestEvent;

    struct TestAggregate;

    impl Aggregate for TestAggregate {
        type State = TestState;
        type Command = TestCommand;
        type Event = TestEvent;
        type Reply = &'static str;
        type Error = &'static str;

        fn stream_id(command: &Self::Command) -> StreamId {
            StreamId::new(command.stream_id).expect("stream id")
        }

        fn partition_key(command: &Self::Command) -> PartitionKey {
            PartitionKey::new(command.partition_key).expect("partition key")
        }

        fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
            ExpectedRevision::NoStream
        }

        fn decide(
            _state: &Self::State,
            _command: Self::Command,
            _metadata: &CommandMetadata,
        ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
            Ok(Decision::new(vec![TestEvent], "ok"))
        }

        fn apply(_state: &mut Self::State, _event: &Self::Event) {}
    }

    fn metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: None,
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        }
    }

    #[test]
    fn command_envelope_new_computes_routing_fields() {
        let (reply, _rx) = oneshot::channel();
        let envelope = CommandEnvelope::<TestAggregate>::new(
            TestCommand {
                stream_id: "order-1",
                partition_key: "customer-1",
            },
            metadata(),
            "idem-1",
            reply,
        )
        .expect("envelope");

        assert_eq!("order-1", envelope.stream_id.as_str());
        assert_eq!("customer-1", envelope.partition_key.as_str());
        assert_eq!(ExpectedRevision::NoStream, envelope.expected_revision);
        assert_eq!("idem-1", envelope.idempotency_key);
    }
}

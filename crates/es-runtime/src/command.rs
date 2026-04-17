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
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
                .expect("timestamp"),
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

//! Router and gateway integration tests.

use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId, TenantId};
use es_kernel::{Aggregate, Decision};
use es_runtime::{CommandEnvelope, CommandGateway, PartitionRouter, RuntimeError};
use time::OffsetDateTime;
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq)]
struct GatewayState;

#[derive(Clone, Debug, PartialEq)]
struct GatewayCommand {
    stream_id: &'static str,
    partition_key: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
struct GatewayEvent;

struct GatewayAggregate;

impl Aggregate for GatewayAggregate {
    type State = GatewayState;
    type Command = GatewayCommand;
    type Event = GatewayEvent;
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
        Ok(Decision::new(vec![GatewayEvent], "ok"))
    }

    fn apply(_state: &mut Self::State, _event: &Self::Event) {}
}

fn metadata(tenant_id: &'static str) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: TenantId::new(tenant_id).expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn envelope(
    tenant_id: &'static str,
    stream_id: &'static str,
    partition_key: &'static str,
) -> CommandEnvelope<GatewayAggregate> {
    let (reply, _rx) = oneshot::channel();
    CommandEnvelope::<GatewayAggregate>::new(
        GatewayCommand {
            stream_id,
            partition_key,
        },
        metadata(tenant_id),
        format!("idem-{tenant_id}-{stream_id}"),
        reply,
    )
    .expect("envelope")
}

#[test]
fn partition_router_rejects_zero_shards() {
    let error = PartitionRouter::new(0).expect_err("zero shards rejected");

    assert!(matches!(error, RuntimeError::InvalidShardCount));
}

#[test]
fn same_tenant_and_key_route_to_same_shard() {
    let router = PartitionRouter::new(8).expect("router");
    let tenant = TenantId::new("tenant-a").expect("tenant");
    let partition_key = PartitionKey::new("order-123").expect("partition key");

    let first = router.route(&tenant, &partition_key);
    let second = router.route(&tenant, &partition_key);

    assert_eq!(first, second);
    assert_eq!(5, first.value());
}

#[test]
fn tenant_is_part_of_route_input() {
    let router = PartitionRouter::new(8).expect("router");
    let tenant_a = TenantId::new("tenant-a").expect("tenant a");
    let tenant_b = TenantId::new("tenant-b").expect("tenant b");
    let partition_key = PartitionKey::new("order-123").expect("partition key");

    let tenant_a_shard = router.route(&tenant_a, &partition_key);
    let tenant_b_shard = router.route(&tenant_b, &partition_key);

    assert_ne!(tenant_a_shard, tenant_b_shard);
    assert_eq!(5, tenant_a_shard.value());
    assert_eq!(2, tenant_b_shard.value());
}

#[test]
fn command_gateway_rejects_zero_ingress_capacity() {
    let router = PartitionRouter::new(8).expect("router");

    let result = CommandGateway::<GatewayAggregate>::new(router, 0);

    assert!(matches!(result, Err(RuntimeError::InvalidIngressCapacity)));
}

#[test]
fn bounded_ingress_returns_overloaded_when_full() {
    let router = PartitionRouter::new(8).expect("router");
    let (gateway, _receiver) = CommandGateway::<GatewayAggregate>::new(router, 1)
        .expect("capacity-one gateway");

    gateway
        .try_submit(envelope("tenant-a", "order-123", "order-123"))
        .expect("first submit accepted");

    let error = gateway
        .try_submit(envelope("tenant-a", "order-456", "order-456"))
        .expect_err("second submit overloads bounded ingress");

    assert!(matches!(error, RuntimeError::Overloaded));
}

#[test]
fn closed_ingress_returns_unavailable() {
    let router = PartitionRouter::new(8).expect("router");
    let (gateway, receiver) = CommandGateway::<GatewayAggregate>::new(router, 1)
        .expect("capacity-one gateway");
    drop(receiver);

    let error = gateway
        .try_submit(envelope("tenant-a", "order-123", "order-123"))
        .expect_err("closed receiver is unavailable");

    assert!(matches!(error, RuntimeError::Unavailable));
}

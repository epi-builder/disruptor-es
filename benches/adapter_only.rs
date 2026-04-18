//! Adapter-only microbenchmarks.
//!
//! These scenarios construct HTTP-shaped DTOs, decode them into runtime command
//! envelopes, and submit through bounded `CommandGateway` ingress. They do not
//! process commands through `CommandEngine` or touch PostgreSQL repositories.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId, TenantId};
use es_kernel::{Aggregate, Decision};
use es_runtime::{CommandEnvelope, CommandGateway, PartitionRouter, RuntimeError};
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq)]
struct AdapterState;

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct HttpCommandDto {
    tenant_id: String,
    order_id: String,
    idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq)]
struct AdapterCommand {
    order_id: String,
}

#[derive(Clone, Debug, PartialEq)]
struct AdapterEvent;

struct AdapterAggregate;

impl Aggregate for AdapterAggregate {
    type State = AdapterState;
    type Command = AdapterCommand;
    type Event = AdapterEvent;
    type Reply = &'static str;
    type Error = &'static str;

    fn stream_id(command: &Self::Command) -> StreamId {
        StreamId::new(format!("order-{}", command.order_id)).expect("stream id")
    }

    fn partition_key(command: &Self::Command) -> PartitionKey {
        PartitionKey::new(format!("order-{}", command.order_id)).expect("partition key")
    }

    fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
        ExpectedRevision::NoStream
    }

    fn decide(
        _state: &Self::State,
        _command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        Ok(Decision::new(vec![AdapterEvent], "accepted"))
    }

    fn apply(_state: &mut Self::State, _event: &Self::Event) {}
}

fn metadata(dto: &HttpCommandDto, seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: None,
        tenant_id: TenantId::new(&dto.tenant_id).expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn dto_json(order_id: u64) -> Vec<u8> {
    format!(
        r#"{{"tenant_id":"tenant-bench","order_id":"{order_id}","idempotency_key":"idem-{order_id}"}}"#
    )
    .into_bytes()
}

fn envelope_from_dto(dto: HttpCommandDto, seed: u128) -> CommandEnvelope<AdapterAggregate> {
    let (reply, _receiver) = oneshot::channel();
    CommandEnvelope::<AdapterAggregate>::new(
        AdapterCommand {
            order_id: dto.order_id.clone(),
        },
        metadata(&dto, seed),
        dto.idempotency_key,
        reply,
    )
    .expect("command envelope")
}

fn adapter_only_decode_envelope_submit(criterion: &mut Criterion) {
    let router = PartitionRouter::new(8).expect("router");
    let (gateway, mut receiver) =
        CommandGateway::<AdapterAggregate>::new(router, 1024).expect("gateway");
    let mut order_id = 0_u64;

    criterion.bench_function("adapter_only_decode_envelope_submit", |bench| {
        bench.iter(|| {
            order_id = order_id.wrapping_add(1);
            let dto: HttpCommandDto =
                serde_json::from_slice(&dto_json(order_id)).expect("decode DTO");
            let envelope = envelope_from_dto(dto, u128::from(order_id) + 100);
            gateway
                .try_submit(envelope)
                .expect("submit through gateway");
            let routed = receiver.try_recv().expect("drain accepted command");
            black_box(routed.shard_id);
        });
    });
}

fn adapter_only_burst_overload(criterion: &mut Criterion) {
    criterion.bench_function("adapter_only_burst_overload", |bench| {
        bench.iter(|| {
            let router = PartitionRouter::new(1).expect("router");
            let (gateway, _receiver) =
                CommandGateway::<AdapterAggregate>::new(router, 1).expect("gateway");
            let first: HttpCommandDto =
                serde_json::from_slice(&dto_json(1)).expect("decode first DTO");
            gateway
                .try_submit(envelope_from_dto(first, 200))
                .expect("first submit accepted");

            let second: HttpCommandDto =
                serde_json::from_slice(&dto_json(2)).expect("decode second DTO");
            let error = gateway
                .try_submit(envelope_from_dto(second, 300))
                .expect_err("bounded gateway overloaded");
            assert!(matches!(error, RuntimeError::Overloaded));
            black_box(error);
        });
    });
}

criterion_group!(
    adapter_only,
    adapter_only_decode_envelope_submit,
    adapter_only_burst_overload
);
criterion_main!(adapter_only);

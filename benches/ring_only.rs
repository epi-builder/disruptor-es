//! Ring-only microbenchmarks.
//!
//! These scenarios measure `DisruptorPath` publication and polling only. They
//! are not service throughput numbers and intentionally avoid domain, adapter,
//! storage, projection, and outbox imports.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use es_runtime::{DisruptorPath, ShardId};

#[derive(Clone, Debug)]
struct RingOnlyEvent {
    partition_key: &'static str,
    payload: u64,
}

fn ring_only_publish_poll(criterion: &mut Criterion) {
    let mut path = DisruptorPath::new(ShardId::new(0), 1024, || RingOnlyEvent {
        partition_key: "cold-key",
        payload: 0,
    })
    .expect("ring-only disruptor path");
    let mut payload = 0_u64;

    criterion.bench_function("ring_only_publish_poll", |bench| {
        bench.iter(|| {
            payload = payload.wrapping_add(1);
            path.try_publish(RingOnlyEvent {
                partition_key: "cold-key",
                payload,
            })
            .expect("publish to ring");
            let released = path.poll_released();
            assert_eq!(1, released.len());
            black_box(released[0].event.payload);
        });
    });
}

fn ring_only_hot_key_publish_poll(criterion: &mut Criterion) {
    let mut path = DisruptorPath::new(ShardId::new(0), 1024, || RingOnlyEvent {
        partition_key: "hot-key",
        payload: 0,
    })
    .expect("ring-only disruptor path");
    let mut payload = 0_u64;

    criterion.bench_function("ring_only_hot_key_publish_poll", |bench| {
        bench.iter(|| {
            payload = payload.wrapping_add(1);
            path.try_publish(RingOnlyEvent {
                partition_key: "hot-key",
                payload,
            })
            .expect("publish to ring");
            let released = path.poll_released();
            assert_eq!("hot-key", released[0].event.partition_key);
            black_box(released[0].sequence);
        });
    });
}

criterion_group!(
    ring_only,
    ring_only_publish_poll,
    ring_only_hot_key_publish_poll
);
criterion_main!(ring_only);

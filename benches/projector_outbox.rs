//! Projector and outbox PostgreSQL microbenchmarks.
//!
//! This benchmark owns a PostgreSQL 18 Testcontainers harness. Developer
//! database runs live in `storage_only.rs`.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_outbox::{
    DispatchBatchLimit, InMemoryPublisher, MessageKey, NewOutboxMessage, PendingSourceEventRef,
    RetryPolicy, Topic, WorkerId, dispatch_once,
};
use es_projection::{ProjectionBatchLimit, ProjectorName};
use es_store_postgres::{
    AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, PostgresOutboxStore,
    PostgresProjectionStore,
};
use example_commerce::{OrderEvent, OrderId, OrderLine, ProductId, Quantity, Sku, UserId};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use uuid::Uuid;

struct PostgresBenchHarness {
    _container: ContainerAsync<Postgres>,
    pool: PgPool,
}

async fn start_postgres() -> anyhow::Result<PostgresBenchHarness> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(PostgresBenchHarness {
        _container: container,
        pool,
    })
}

fn tenant_id() -> TenantId {
    TenantId::new("tenant-projector-outbox-bench").expect("tenant id")
}

fn stream_id(value: impl Into<String>) -> StreamId {
    StreamId::new(value).expect("stream id")
}

fn command_metadata(seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: Some(Uuid::from_u128(seed + 2)),
        tenant_id: tenant_id(),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn projector_name() -> ProjectorName {
    ProjectorName::new("bench-commerce-read-models").expect("projector name")
}

fn projection_limit() -> ProjectionBatchLimit {
    ProjectionBatchLimit::new(100).expect("projection batch limit")
}

fn worker_id() -> WorkerId {
    WorkerId::new("bench-outbox-worker").expect("worker id")
}

fn dispatch_limit() -> DispatchBatchLimit {
    DispatchBatchLimit::new(100).expect("dispatch limit")
}

fn retry_policy() -> RetryPolicy {
    RetryPolicy::new(2).expect("retry policy")
}

fn order_id(value: impl Into<String>) -> OrderId {
    OrderId::new(value).expect("order id")
}

fn user_id() -> UserId {
    UserId::new("user-bench").expect("user id")
}

fn product_id() -> ProductId {
    ProductId::new("product-bench").expect("product id")
}

fn sku() -> Sku {
    Sku::new("SKU-BENCH").expect("sku")
}

fn quantity() -> Quantity {
    Quantity::new(2).expect("quantity")
}

fn order_line() -> OrderLine {
    OrderLine {
        product_id: product_id(),
        sku: sku(),
        quantity: quantity(),
        product_available: true,
    }
}

fn order_placed_event(seed: u128, order: &str) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        "OrderPlaced",
        1,
        serde_json::to_value(OrderEvent::OrderPlaced {
            order_id: order_id(order),
            user_id: user_id(),
            lines: vec![order_line()],
        })
        .expect("serialize order event"),
        json!({ "source": "projector_outbox" }),
    )
    .expect("order placed event")
}

fn outbox_message(source_event_id: Uuid) -> NewOutboxMessage {
    NewOutboxMessage::new(
        PendingSourceEventRef::new(source_event_id),
        Topic::new("orders.placed").expect("topic"),
        MessageKey::new(format!("order-{source_event_id}")).expect("message key"),
        json!({ "source_event_id": source_event_id }),
        json!({ "source": "projector_outbox" }),
    )
}

async fn append_projectable_order(store: &PostgresEventStore, seed: u128) -> anyhow::Result<i64> {
    let order = format!("projector-order-{seed}");
    let outcome = store
        .append(AppendRequest::new(
            stream_id(format!("order-{order}")),
            ExpectedRevision::NoStream,
            command_metadata(seed),
            format!("projector-order-{seed}"),
            vec![order_placed_event(seed + 10_000, &order)],
        )?)
        .await?;
    let AppendOutcome::Committed(committed) = outcome else {
        anyhow::bail!("projector append should commit");
    };

    Ok(*committed.global_positions.first().expect("global position"))
}

async fn append_outbox_order(store: &PostgresEventStore, seed: u128) -> anyhow::Result<i64> {
    let order = format!("outbox-order-{seed}");
    let event = order_placed_event(seed + 20_000, &order);
    let source_event_id = event.event_id;
    let outcome = store
        .append(AppendRequest::new_with_outbox(
            stream_id(format!("order-{order}")),
            ExpectedRevision::NoStream,
            command_metadata(seed),
            format!("outbox-order-{seed}"),
            vec![event],
            vec![outbox_message(source_event_id)],
        )?)
        .await?;
    let AppendOutcome::Committed(committed) = outcome else {
        anyhow::bail!("outbox append should commit");
    };

    Ok(*committed.global_positions.first().expect("global position"))
}

fn projector_catch_up(criterion: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let harness = runtime
        .block_on(start_postgres())
        .expect("postgres harness");
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id();
    let projector = projector_name();
    let mut seed = 10_000_u128;

    criterion.bench_function("projector_catch_up", |bench| {
        bench.iter(|| {
            seed = seed.wrapping_add(1);
            let outcome = runtime
                .block_on(async {
                    append_projectable_order(&events, seed).await?;
                    projections
                        .catch_up(&tenant, &projector, projection_limit())
                        .await
                        .map_err(anyhow::Error::from)
                })
                .expect("projector catch-up");
            black_box(outcome);
        });
    });

    runtime.block_on(async move {
        drop(harness);
    });
}

fn outbox_claim_publish(criterion: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let harness = runtime
        .block_on(start_postgres())
        .expect("postgres harness");
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let publisher = InMemoryPublisher::default();
    let tenant = tenant_id();
    let mut seed = 20_000_u128;

    criterion.bench_function("outbox_claim_publish", |bench| {
        bench.iter(|| {
            seed = seed.wrapping_add(1);
            let outcome = runtime
                .block_on(async {
                    append_outbox_order(&events, seed).await?;
                    dispatch_once(
                        &outbox,
                        &publisher,
                        tenant.clone(),
                        worker_id(),
                        dispatch_limit(),
                        retry_policy(),
                    )
                    .await
                    .map_err(anyhow::Error::from)
                })
                .expect("outbox dispatch");
            black_box(outcome);
        });
    });

    runtime.block_on(async move {
        drop(harness);
    });
}

criterion_group!(projector_outbox, projector_catch_up, outbox_claim_publish);
criterion_main!(projector_outbox);

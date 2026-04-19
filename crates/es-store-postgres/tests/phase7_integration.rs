//! Phase 7 cross-layer PostgreSQL integration coverage.

mod common;

use std::sync::{Arc, Mutex as StdMutex};

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_outbox::{
    DispatchBatchLimit, DispatchOutcome, InMemoryPublisher, MessageKey, NewOutboxMessage,
    PendingSourceEventRef, RetryPolicy, Topic, WorkerId, dispatch_once,
};
use es_projection::{CatchUpOutcome, ProjectionBatchLimit, ProjectorName};
use es_store_postgres::{
    AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, PostgresOutboxStore,
    PostgresProjectionStore, SaveSnapshotRequest, StoreError,
};
use example_commerce::{OrderEvent, OrderId, OrderLine, ProductId, Quantity, Sku, UserId};
use metrics::{
    Counter, Gauge, GaugeFn, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit,
};
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;

static POSTGRES_TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn tenant_id(value: &str) -> TenantId {
    TenantId::new(value).expect("valid tenant id")
}

fn stream_id(value: &str) -> StreamId {
    StreamId::new(value).expect("valid stream id")
}

fn command_metadata(tenant: TenantId, seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: Some(Uuid::from_u128(seed + 2)),
        tenant_id: tenant,
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .expect("valid requested_at"),
    }
}

fn new_event(seed: u128, event_type: &str, payload: serde_json::Value) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        payload,
        json!({ "source": "phase7-integration" }),
    )
    .expect("valid event")
}

fn append_request(
    tenant: TenantId,
    stream: StreamId,
    expected_revision: ExpectedRevision,
    idempotency_key: &str,
    command_seed: u128,
    events: Vec<NewEvent>,
) -> AppendRequest {
    AppendRequest::new(
        stream,
        expected_revision,
        command_metadata(tenant, command_seed),
        idempotency_key,
        events,
    )
    .expect("valid append request")
}

fn snapshot_request(
    tenant_id: TenantId,
    stream_id: StreamId,
    revision: u64,
    version: u64,
) -> SaveSnapshotRequest {
    SaveSnapshotRequest {
        tenant_id,
        stream_id,
        stream_revision: StreamRevision::new(revision),
        state_payload: json!({ "version": version }),
        metadata: json!({ "source": "phase7-integration" }),
    }
}

fn new_outbox_message(source_event_id: Uuid, topic_value: &str) -> NewOutboxMessage {
    NewOutboxMessage::new(
        PendingSourceEventRef::new(source_event_id),
        Topic::new(topic_value).expect("valid topic"),
        MessageKey::new(format!("key-{source_event_id}")).expect("valid message key"),
        json!({ "source_event_id": source_event_id }),
        json!({ "source": "phase7-integration" }),
    )
}

fn projector_name() -> ProjectorName {
    ProjectorName::new("phase7-commerce-read-models").expect("valid projector name")
}

fn projection_limit() -> ProjectionBatchLimit {
    ProjectionBatchLimit::new(100).expect("valid projection batch limit")
}

fn worker_id(value: &str) -> WorkerId {
    WorkerId::new(value).expect("valid worker id")
}

fn dispatch_limit(value: i64) -> DispatchBatchLimit {
    DispatchBatchLimit::new(value).expect("valid dispatch limit")
}

fn retry_policy(max_attempts: i32) -> RetryPolicy {
    RetryPolicy::new(max_attempts).expect("valid retry policy")
}

fn order_id(value: &str) -> OrderId {
    OrderId::new(value).expect("valid order id")
}

fn user_id(value: &str) -> UserId {
    UserId::new(value).expect("valid user id")
}

fn product_id(value: &str) -> ProductId {
    ProductId::new(value).expect("valid product id")
}

fn sku(value: &str) -> Sku {
    Sku::new(value).expect("valid sku")
}

fn quantity(value: u32) -> Quantity {
    Quantity::new(value).expect("valid quantity")
}

fn order_line(product: &str, quantity_value: u32) -> OrderLine {
    OrderLine {
        product_id: product_id(product),
        sku: sku(&format!("SKU-{product}")),
        quantity: quantity(quantity_value),
        product_available: true,
    }
}

fn order_placed_event(seed: u128, order: &str, user: &str) -> NewEvent {
    new_event(
        seed,
        "OrderPlaced",
        serde_json::to_value(OrderEvent::OrderPlaced {
            order_id: order_id(order),
            user_id: user_id(user),
            lines: vec![order_line("product-1", 2)],
        })
        .expect("serialize order placed event"),
    )
}

async fn append_order_placed_sequence(
    store: &PostgresEventStore,
    tenant: TenantId,
    order_prefix: &str,
    user: &str,
    count: usize,
    seed: u128,
) -> anyhow::Result<()> {
    for index in 0..count {
        let event_seed = seed + u128::try_from(index).expect("usize fits u128");
        let order = format!("{order_prefix}-{index}");
        store
            .append(append_request(
                tenant.clone(),
                stream_id(&format!("order-{order}")),
                ExpectedRevision::NoStream,
                &format!("{order}-place"),
                event_seed + 10_000,
                vec![order_placed_event(event_seed, &order, user)],
            ))
            .await?;
    }

    Ok(())
}

async fn tenant_latest_position(pool: &sqlx::PgPool, tenant: &TenantId) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(max(global_position), 0) FROM events WHERE tenant_id = $1",
    )
    .bind(tenant.as_str())
    .fetch_one(pool)
    .await?)
}

#[derive(Debug)]
struct ProjectionLagGauge {
    value: StdMutex<Option<f64>>,
}

impl ProjectionLagGauge {
    fn new() -> Self {
        Self {
            value: StdMutex::new(None),
        }
    }
}

impl GaugeFn for ProjectionLagGauge {
    fn increment(&self, value: f64) {
        let mut guard = self.value.lock().expect("projection lag mutex poisoned");
        let current = guard.unwrap_or(0.0);
        *guard = Some(current + value);
    }

    fn decrement(&self, value: f64) {
        let mut guard = self.value.lock().expect("projection lag mutex poisoned");
        let current = guard.unwrap_or(0.0);
        *guard = Some(current - value);
    }

    fn set(&self, value: f64) {
        *self.value.lock().expect("projection lag mutex poisoned") = Some(value);
    }
}

#[derive(Debug)]
struct ProjectionLagRecorder {
    gauge: Arc<ProjectionLagGauge>,
}

impl ProjectionLagRecorder {
    fn new(gauge: Arc<ProjectionLagGauge>) -> Self {
        Self { gauge }
    }
}

impl Recorder for ProjectionLagRecorder {
    fn describe_counter(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_gauge(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn describe_histogram(&self, _key: KeyName, _unit: Option<Unit>, _description: SharedString) {}

    fn register_counter(&self, _key: &Key, _metadata: &Metadata<'_>) -> Counter {
        Counter::noop()
    }

    fn register_gauge(&self, key: &Key, _metadata: &Metadata<'_>) -> Gauge {
        let is_projection_lag = key.name() == "es_projection_lag";
        let is_phase7_projector = key.labels().any(|label| {
            label.key() == "projector" && label.value() == "phase7-commerce-read-models"
        });

        if is_projection_lag && is_phase7_projector {
            Gauge::from_arc(self.gauge.clone())
        } else {
            Gauge::noop()
        }
    }

    fn register_histogram(&self, _key: &Key, _metadata: &Metadata<'_>) -> Histogram {
        Histogram::noop()
    }
}

fn observed_projection_lag_value(gauge: &ProjectionLagGauge) -> f64 {
    gauge
        .value
        .lock()
        .expect("projection lag mutex poisoned")
        .expect("projection lag gauge was set")
}

#[tokio::test]
async fn phase7_append_conflict_and_global_positions() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("phase7-order-append");

    let first = store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "phase7-append-1",
            10,
            vec![
                new_event(100, "OrderPlaced", json!({ "order_id": "phase7-order" })),
                new_event(101, "OrderConfirmed", json!({ "order_id": "phase7-order" })),
            ],
        ))
        .await?;

    let AppendOutcome::Committed(committed) = first else {
        panic!("first append should commit");
    };
    assert_eq!(StreamRevision::new(1), committed.first_revision);
    assert_eq!(StreamRevision::new(2), committed.last_revision);
    assert_eq!(2, committed.global_positions.len());
    assert!(committed.global_positions[0] < committed.global_positions[1]);

    let read_back = store.read_global(&tenant, 0, 10).await?;
    assert_eq!(2, read_back.len());
    assert_eq!(committed.global_positions[0], read_back[0].global_position);
    assert_eq!(committed.global_positions[1], read_back[1].global_position);

    let error = store
        .append(append_request(
            tenant,
            stream,
            ExpectedRevision::Exact(StreamRevision::new(99)),
            "phase7-append-conflict",
            20,
            vec![new_event(
                102,
                "OrderCancelled",
                json!({ "order_id": "phase7-order" }),
            )],
        ))
        .await
        .expect_err("wrong expected revision should conflict");

    assert!(matches!(
        error,
        StoreError::StreamConflict {
            actual: Some(2),
            ..
        }
    ));

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn phase7_projection_lag_uses_tenant_durable_backlog_not_batch_size() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant_a = tenant_id("tenant-lag-a");
    let tenant_b = tenant_id("tenant-lag-b");
    let projector = projector_name();

    append_order_placed_sequence(
        &events,
        tenant_a.clone(),
        "tenant-lag-a-order",
        "lag-user-a",
        250,
        10_000,
    )
    .await?;
    append_order_placed_sequence(
        &events,
        tenant_b.clone(),
        "tenant-lag-b-order",
        "lag-user-b",
        3,
        20_000,
    )
    .await?;

    let gauge = Arc::new(ProjectionLagGauge::new());
    let recorder = ProjectionLagRecorder::new(gauge.clone());
    let _recorder_guard = metrics::set_default_local_recorder(&recorder);

    let outcome = projections
        .catch_up(
            &tenant_a,
            &projector,
            ProjectionBatchLimit::new(25).expect("valid batch limit"),
        )
        .await?;
    assert!(matches!(
        outcome,
        CatchUpOutcome::Applied {
            event_count: 25,
            ..
        }
    ));

    let offset = projections
        .projector_offset(&tenant_a, &projector)
        .await?
        .expect("projector offset");
    let tenant_latest = tenant_latest_position(&harness.pool, &tenant_a).await?;
    let expected_lag = (tenant_latest - offset.last_global_position).max(0) as f64;

    assert!(expected_lag > 25.0);
    assert_eq!(observed_projection_lag_value(&gauge), expected_lag);

    loop {
        match projections
            .catch_up(&tenant_a, &projector, projection_limit())
            .await?
        {
            CatchUpOutcome::Applied { .. } => {}
            CatchUpOutcome::Idle => break,
        }
    }

    let final_offset = projections
        .projector_offset(&tenant_a, &projector)
        .await?
        .expect("projector offset");
    assert_eq!(tenant_latest, final_offset.last_global_position);
    assert_eq!(observed_projection_lag_value(&gauge), 0.0);

    Ok(())
}

#[tokio::test]
async fn phase7_dedupe_returns_original_committed_result() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("phase7-order-dedupe");

    let first = store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "phase7-dedupe-key",
            30,
            vec![new_event(
                200,
                "OrderPlaced",
                json!({ "order_id": "phase7-dedupe" }),
            )],
        ))
        .await?;
    let duplicate = store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "phase7-dedupe-key",
            40,
            vec![new_event(
                201,
                "OrderPlaced",
                json!({ "order_id": "phase7-dedupe-retry" }),
            )],
        ))
        .await?;

    let AppendOutcome::Committed(first_committed) = first else {
        panic!("first append should commit");
    };
    let AppendOutcome::Duplicate(duplicate_committed) = duplicate else {
        panic!("duplicate append should return the original committed result");
    };

    assert_eq!(first_committed, duplicate_committed);
    assert_eq!(
        1,
        store.read_stream(&tenant, &stream, None, 10).await?.len()
    );

    Ok(())
}

#[tokio::test]
async fn phase7_snapshot_rehydration_uses_latest_snapshot() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("phase7-order-snapshot");

    store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "phase7-snapshot-events",
            50,
            vec![
                new_event(300, "OrderPlaced", json!({ "order_id": "phase7-snapshot" })),
                new_event(
                    301,
                    "OrderConfirmed",
                    json!({ "order_id": "phase7-snapshot" }),
                ),
                new_event(302, "OrderPacked", json!({ "order_id": "phase7-snapshot" })),
            ],
        ))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 1, 1))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 2, 2))
        .await?;

    let batch = store.load_rehydration(&tenant, &stream).await?;
    let snapshot = batch.snapshot.expect("latest snapshot exists");

    assert_eq!(StreamRevision::new(2), snapshot.stream_revision);
    assert_eq!(json!({ "version": 2 }), snapshot.state_payload);
    assert_eq!(1, batch.events.len());
    assert_eq!(StreamRevision::new(3), batch.events[0].stream_revision);
    assert_eq!("OrderPacked", batch.events[0].event_type);

    Ok(())
}

#[tokio::test]
async fn phase7_projector_checkpoint_and_outbox_dispatch() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();
    let event = order_placed_event(400, "phase7-projected-order", "phase7-user");
    let source_event_id = event.event_id;

    let outcome = events
        .append(AppendRequest::new_with_outbox(
            stream_id("phase7-order-projection-outbox"),
            ExpectedRevision::NoStream,
            command_metadata(tenant.clone(), 60),
            "phase7-projection-outbox",
            vec![event],
            vec![new_outbox_message(source_event_id, "orders.placed")],
        )?)
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append with outbox should commit");
    };
    let source_position = *committed
        .global_positions
        .first()
        .expect("source event global position");

    let projection = projections
        .catch_up(&tenant, &projector, projection_limit())
        .await?;
    assert_eq!(
        CatchUpOutcome::Applied {
            event_count: 1,
            last_global_position: source_position,
        },
        projection
    );
    let offset = projections
        .projector_offset(&tenant, &projector)
        .await?
        .expect("projector offset");
    assert_eq!(source_position, offset.last_global_position);
    let order = projections
        .order_summary(&tenant, "phase7-projected-order", None, None)
        .await?
        .expect("order summary");
    assert_eq!("phase7-user", order.user_id);
    assert_eq!("Placed", order.status);
    assert_eq!(source_position, order.last_applied_global_position);

    let publisher = InMemoryPublisher::default();
    let dispatch = dispatch_once(
        &outbox,
        &publisher,
        tenant.clone(),
        worker_id("phase7-worker"),
        dispatch_limit(10),
        retry_policy(2),
    )
    .await?;
    assert_eq!(DispatchOutcome::Published { published: 1 }, dispatch);
    let published = publisher.published();
    assert_eq!(1, published.len());
    assert_eq!("orders.placed", published[0].topic);
    assert_eq!(
        format!("tenant-a:orders.placed:{source_event_id}"),
        published[0].idempotency_key
    );

    Ok(())
}

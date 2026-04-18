//! Phase 7 cross-layer PostgreSQL integration coverage.

mod common;

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

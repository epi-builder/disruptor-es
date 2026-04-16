//! Durable append and optimistic-concurrency integration tests.

mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_store_postgres::{AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, StoreError};
use serde_json::json;
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

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

fn new_event(seed: u128, event_type: &str, order_id: &str) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        json!({ "order_id": order_id }),
        json!({ "source": "append_occ" }),
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

#[tokio::test]
async fn first_append_commits_event_with_no_stream_expectation() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    let outcome = store
        .append(append_request(
            tenant_id("tenant-a"),
            stream_id("order-1"),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![new_event(100, "OrderPlaced", "order-1")],
        ))
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("first append should commit new events");
    };

    assert_eq!(StreamRevision::new(1), committed.first_revision);
    assert_eq!(StreamRevision::new(1), committed.last_revision);
    assert_eq!(vec![Uuid::from_u128(100)], committed.event_ids);
    assert_eq!(1, committed.global_positions.len());
    assert!(committed.global_positions[0] >= 1);

    let stream_revision = sqlx::query_scalar::<_, i64>(
        "SELECT stream_revision FROM events WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind("tenant-a")
    .bind("order-1")
    .fetch_one(&harness.pool)
    .await?;

    assert_eq!(1, stream_revision);

    Ok(())
}

#[tokio::test]
async fn multi_event_append_assigns_sequential_revisions() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    let outcome = store
        .append(append_request(
            tenant_id("tenant-a"),
            stream_id("order-1"),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![
                new_event(100, "OrderPlaced", "order-1"),
                new_event(101, "OrderConfirmed", "order-1"),
            ],
        ))
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("multi-event append should commit");
    };

    assert_eq!(StreamRevision::new(1), committed.first_revision);
    assert_eq!(StreamRevision::new(2), committed.last_revision);
    assert_eq!(
        vec![Uuid::from_u128(100), Uuid::from_u128(101)],
        committed.event_ids
    );
    assert_eq!(2, committed.global_positions.len());
    assert!(committed.global_positions[0] < committed.global_positions[1]);

    let revisions = sqlx::query_scalar::<_, i64>(
        "SELECT stream_revision FROM events WHERE tenant_id = $1 AND stream_id = $2 ORDER BY stream_revision",
    )
    .bind("tenant-a")
    .bind("order-1")
    .fetch_all(&harness.pool)
    .await?;

    assert_eq!(vec![1, 2], revisions);

    Ok(())
}

#[tokio::test]
async fn wrong_expected_revision_returns_stream_conflict() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![new_event(100, "OrderPlaced", "order-1")],
        ))
        .await?;

    let error = store
        .append(append_request(
            tenant,
            stream,
            ExpectedRevision::Exact(StreamRevision::new(99)),
            "command-2",
            20,
            vec![new_event(101, "OrderConfirmed", "order-1")],
        ))
        .await
        .expect_err("wrong expected revision should conflict");

    assert!(matches!(
        error,
        StoreError::StreamConflict {
            actual: Some(1),
            ..
        }
    ));

    Ok(())
}

#[tokio::test]
async fn metadata_columns_are_persisted() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    store
        .append(append_request(
            tenant_id("tenant-a"),
            stream_id("order-1"),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![new_event(100, "OrderPlaced", "order-1")],
        ))
        .await?;

    let row = sqlx::query(
        r#"
        SELECT
            event_id,
            stream_id,
            stream_revision,
            global_position,
            command_id,
            causation_id,
            correlation_id,
            tenant_id,
            event_type,
            schema_version,
            payload,
            metadata,
            recorded_at
        FROM events
        WHERE tenant_id = $1 AND stream_id = $2
        "#,
    )
    .bind("tenant-a")
    .bind("order-1")
    .fetch_one(&harness.pool)
    .await?;

    assert_eq!(Uuid::from_u128(100), row.get::<Uuid, _>("event_id"));
    assert_eq!("order-1", row.get::<String, _>("stream_id"));
    assert_eq!(1, row.get::<i64, _>("stream_revision"));
    assert!(row.get::<i64, _>("global_position") >= 1);
    assert_eq!(Uuid::from_u128(10), row.get::<Uuid, _>("command_id"));
    assert_eq!(Uuid::from_u128(12), row.get::<Uuid, _>("causation_id"));
    assert_eq!(Uuid::from_u128(11), row.get::<Uuid, _>("correlation_id"));
    assert_eq!("tenant-a", row.get::<String, _>("tenant_id"));
    assert_eq!("OrderPlaced", row.get::<String, _>("event_type"));
    assert_eq!(1, row.get::<i32, _>("schema_version"));
    assert_eq!(
        json!({ "order_id": "order-1" }),
        row.get::<serde_json::Value, _>("payload")
    );
    assert_eq!(
        json!({ "source": "append_occ" }),
        row.get::<serde_json::Value, _>("metadata")
    );
    let recorded_at = row.get::<OffsetDateTime, _>("recorded_at");
    assert!(recorded_at.unix_timestamp() >= 1_700_000_000);

    Ok(())
}

#[tokio::test]
async fn conflict_rolls_back_without_extra_events() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            ExpectedRevision::NoStream,
            "command-1",
            10,
            vec![new_event(100, "OrderPlaced", "order-1")],
        ))
        .await?;

    let error = store
        .append(append_request(
            tenant,
            stream,
            ExpectedRevision::Exact(StreamRevision::new(99)),
            "command-2",
            20,
            vec![new_event(101, "OrderConfirmed", "order-1")],
        ))
        .await
        .expect_err("wrong expected revision should conflict");

    assert!(matches!(error, StoreError::StreamConflict { .. }));

    let event_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM events WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind("tenant-a")
    .bind("order-1")
    .fetch_one(&harness.pool)
    .await?;

    assert_eq!(1, event_count);

    Ok(())
}

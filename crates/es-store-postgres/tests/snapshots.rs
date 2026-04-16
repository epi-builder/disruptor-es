//! Snapshot and rehydration integration tests.

mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_store_postgres::{
    AppendRequest, NewEvent, PostgresEventStore, SaveSnapshotRequest, SnapshotRecord, StoreError,
};
use serde_json::json;
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
        json!({ "source": "snapshots" }),
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
        metadata: json!({ "source": "test" }),
    }
}

fn assert_snapshot(snapshot: &SnapshotRecord, revision: u64, version: u64) {
    assert_eq!(StreamRevision::new(revision), snapshot.stream_revision);
    assert_eq!(json!({ "version": version }), snapshot.state_payload);
    assert_eq!(json!({ "source": "test" }), snapshot.metadata);
}

#[tokio::test]
async fn load_latest_snapshot_returns_highest_revision() -> anyhow::Result<()> {
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
            vec![
                new_event(100, "OrderPlaced", "order-1"),
                new_event(101, "OrderConfirmed", "order-1"),
            ],
        ))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 1, 1))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 2, 2))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 2, 3))
        .await?;

    let latest = store
        .load_latest_snapshot(&tenant, &stream)
        .await?
        .expect("snapshot exists");

    assert_snapshot(&latest, 2, 3);

    let snapshot_rows = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM snapshots WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind(tenant.as_str())
    .bind(stream.as_str())
    .fetch_one(&harness.pool)
    .await?;

    assert_eq!(2, snapshot_rows);

    let other_tenant = store
        .load_latest_snapshot(&tenant_id("tenant-b"), &stream)
        .await?;
    let other_stream = store
        .load_latest_snapshot(&tenant, &stream_id("order-2"))
        .await?;

    assert!(other_tenant.is_none());
    assert!(other_stream.is_none());

    Ok(())
}

#[tokio::test]
async fn save_snapshot_rejects_nonexistent_stream() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    let error = store
        .save_snapshot(snapshot_request(
            tenant_id("tenant-a"),
            stream_id("order-missing"),
            1,
            1,
        ))
        .await
        .expect_err("snapshot without stream should conflict");

    assert!(matches!(
        error,
        StoreError::StreamConflict { actual: None, .. }
    ));

    Ok(())
}

#[tokio::test]
async fn save_snapshot_rejects_future_revision() -> anyhow::Result<()> {
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
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 2, 2))
        .await
        .expect_err("future snapshot should conflict");

    assert!(matches!(
        error,
        StoreError::SnapshotRevisionConflict {
            requested: 2,
            current: 1,
            ..
        }
    ));

    let snapshots = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM snapshots WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind(tenant.as_str())
    .bind(stream.as_str())
    .fetch_one(&harness.pool)
    .await?;

    assert_eq!(0, snapshots);

    Ok(())
}

#[tokio::test]
async fn rehydration_returns_latest_snapshot_plus_subsequent_events() -> anyhow::Result<()> {
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
            vec![
                new_event(100, "OrderPlaced", "order-1"),
                new_event(101, "OrderConfirmed", "order-1"),
                new_event(102, "OrderPacked", "order-1"),
            ],
        ))
        .await?;
    store
        .save_snapshot(snapshot_request(tenant.clone(), stream.clone(), 2, 2))
        .await?;

    let batch = store.load_rehydration(&tenant, &stream).await?;

    let snapshot = batch.snapshot.expect("snapshot exists");
    assert_snapshot(&snapshot, 2, 2);
    assert_eq!(vec![3], stream_revisions(&batch.events));

    Ok(())
}

#[tokio::test]
async fn rehydration_without_snapshot_returns_all_stream_events() -> anyhow::Result<()> {
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
            vec![
                new_event(100, "OrderPlaced", "order-1"),
                new_event(101, "OrderConfirmed", "order-1"),
            ],
        ))
        .await?;

    let batch = store.load_rehydration(&tenant, &stream).await?;

    assert!(batch.snapshot.is_none());
    assert_eq!(vec![1, 2], stream_revisions(&batch.events));

    Ok(())
}

fn stream_revisions(events: &[es_store_postgres::StoredEvent]) -> Vec<u64> {
    events
        .iter()
        .map(|event| event.stream_revision.value())
        .collect()
}

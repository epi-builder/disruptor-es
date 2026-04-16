//! Durable global-position read integration tests.

mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_store_postgres::{AppendOutcome, AppendRequest, NewEvent, PostgresEventStore};
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
        json!({ "source": "global_reads" }),
    )
    .expect("valid event")
}

fn append_request(
    tenant: TenantId,
    stream: StreamId,
    idempotency_key: &str,
    command_seed: u128,
    event_seed: u128,
) -> AppendRequest {
    AppendRequest::new(
        stream.clone(),
        ExpectedRevision::NoStream,
        command_metadata(tenant, command_seed),
        idempotency_key,
        vec![new_event(event_seed, "OrderPlaced", stream.as_str())],
    )
    .expect("valid append request")
}

async fn append_one(
    store: &PostgresEventStore,
    tenant: TenantId,
    stream: StreamId,
    idempotency_key: &str,
    command_seed: u128,
    event_seed: u128,
) -> anyhow::Result<i64> {
    let outcome = store
        .append(append_request(
            tenant,
            stream,
            idempotency_key,
            command_seed,
            event_seed,
        ))
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append should commit");
    };

    Ok(committed.global_positions[0])
}

#[tokio::test]
async fn global_reads_return_events_after_position() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");

    let first_position = append_one(
        &store,
        tenant.clone(),
        stream_id("order-1"),
        "command-1",
        10,
        100,
    )
    .await?;
    let second_position = append_one(
        &store,
        tenant.clone(),
        stream_id("order-2"),
        "command-2",
        20,
        200,
    )
    .await?;
    let third_position = append_one(
        &store,
        tenant.clone(),
        stream_id("order-3"),
        "command-3",
        30,
        300,
    )
    .await?;

    let all_events = store.read_global(&tenant, 0, 100).await?;
    assert_eq!(
        vec![first_position, second_position, third_position],
        global_positions(&all_events)
    );
    assert_eq!(vec!["order-1", "order-2", "order-3"], stream_ids(&all_events));

    let later_events = store.read_global(&tenant, first_position, 100).await?;
    assert_eq!(
        vec![second_position, third_position],
        global_positions(&later_events)
    );

    Ok(())
}

#[tokio::test]
async fn global_reads_respect_limit() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");

    let first_position = append_one(
        &store,
        tenant.clone(),
        stream_id("order-1"),
        "command-1",
        10,
        100,
    )
    .await?;
    append_one(
        &store,
        tenant.clone(),
        stream_id("order-2"),
        "command-2",
        20,
        200,
    )
    .await?;

    let events = store.read_global(&tenant, 0, 1).await?;

    assert_eq!(vec![first_position], global_positions(&events));

    Ok(())
}

#[tokio::test]
async fn global_reads_are_scoped_by_tenant() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant_a = tenant_id("tenant-a");
    let tenant_b = tenant_id("tenant-b");

    let tenant_a_position = append_one(
        &store,
        tenant_a.clone(),
        stream_id("order-1"),
        "command-1",
        10,
        100,
    )
    .await?;
    append_one(
        &store,
        tenant_b,
        stream_id("order-1"),
        "command-1",
        20,
        200,
    )
    .await?;

    let events = store.read_global(&tenant_a, 0, 100).await?;

    assert_eq!(vec![tenant_a_position], global_positions(&events));
    assert!(events.iter().all(|event| event.tenant_id == tenant_a));

    Ok(())
}

fn global_positions(events: &[es_store_postgres::StoredEvent]) -> Vec<i64> {
    events.iter().map(|event| event.global_position).collect()
}

fn stream_ids(events: &[es_store_postgres::StoredEvent]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event.stream_id.as_str())
        .collect()
}

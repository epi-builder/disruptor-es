//! PostgreSQL outbox integration tests.

mod common;

use std::time::Duration;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_outbox::{
    DispatchBatchLimit, MessageKey, NewOutboxMessage, PendingSourceEventRef, ProcessManagerName,
    RetryPolicy, RetryScheduleOutcome, Topic, WorkerId,
};
use es_store_postgres::{
    AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, PostgresOutboxStore,
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

fn topic(value: &str) -> Topic {
    Topic::new(value).expect("valid topic")
}

fn message_key(value: &str) -> MessageKey {
    MessageKey::new(value).expect("valid message key")
}

fn worker_id(value: &str) -> WorkerId {
    WorkerId::new(value).expect("valid worker id")
}

fn batch_limit(value: i64) -> DispatchBatchLimit {
    DispatchBatchLimit::new(value).expect("valid batch limit")
}

fn retry_policy(max_attempts: i32) -> RetryPolicy {
    RetryPolicy::new(max_attempts).expect("valid retry policy")
}

fn process_manager_name(value: &str) -> ProcessManagerName {
    ProcessManagerName::new(value).expect("valid process manager name")
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

fn new_outbox_message(source_event_id: Uuid, topic_value: &str) -> NewOutboxMessage {
    NewOutboxMessage::new(
        PendingSourceEventRef::new(source_event_id),
        topic(topic_value),
        message_key(&format!("key-{source_event_id}")),
        json!({ "source_event_id": source_event_id }),
        json!({ "kind": "integration" }),
    )
}

async fn append_source_event(
    store: &PostgresEventStore,
    tenant: TenantId,
    stream: &str,
    seed: u128,
) -> anyhow::Result<(Uuid, i64)> {
    let event_id = Uuid::from_u128(seed + 10);
    let outcome = store
        .append(AppendRequest::new(
            stream_id(stream),
            ExpectedRevision::NoStream,
            command_metadata(tenant, seed),
            format!("source-event-{seed}"),
            vec![NewEvent::new(
                event_id,
                "OutboxSourceEvent",
                1,
                json!({ "seed": seed }),
                json!({ "source": "outbox-test" }),
            )?],
        )?)
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append should commit");
    };

    Ok((
        event_id,
        *committed
            .global_positions
            .first()
            .expect("source event global position"),
    ))
}

#[tokio::test]
async fn outbox_is_idempotent_by_source_event_and_topic() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-1", 100).await?;
    let message = new_outbox_message(source_event_id, "orders.placed");

    let first = outbox
        .insert_outbox_message(&tenant, &message, source_global_position)
        .await?;
    let duplicate = outbox
        .insert_outbox_message(&tenant, &message, source_global_position)
        .await;

    assert_eq!(tenant, first.tenant_id);
    assert_eq!(source_event_id, first.source.event_id());
    assert!(duplicate.is_err());

    Ok(())
}

#[tokio::test]
async fn outbox_claims_pending_rows_with_skip_locked() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (first_event_id, first_position) =
        append_source_event(&events, tenant.clone(), "order-1", 200).await?;
    let (second_event_id, second_position) =
        append_source_event(&events, tenant.clone(), "order-2", 300).await?;
    let first = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(first_event_id, "orders.placed"),
            first_position,
        )
        .await?;
    let second = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(second_event_id, "orders.placed"),
            second_position,
        )
        .await?;

    let mut lock_tx = harness.pool.begin().await?;
    sqlx::query(
        r#"
        SELECT outbox_id
        FROM outbox_messages
        WHERE outbox_id = $1
        FOR UPDATE
        "#,
    )
    .bind(first.outbox_id)
    .fetch_one(&mut *lock_tx)
    .await?;

    let claimed = outbox
        .claim_pending(
            &tenant,
            &worker_id("worker-a"),
            batch_limit(10),
            Duration::from_secs(30),
        )
        .await?;

    assert_eq!(
        vec![second.outbox_id],
        claimed.iter().map(|row| row.outbox_id).collect::<Vec<_>>()
    );
    assert!(
        claimed
            .iter()
            .all(|row| row.locked_by.as_ref().map(WorkerId::as_str) == Some("worker-a"))
    );

    lock_tx.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn outbox_retry_and_failed_status_are_bounded() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-1", 400).await?;
    let inserted = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(source_event_id, "orders.placed"),
            source_global_position,
        )
        .await?;

    let first_claim = outbox
        .claim_pending(
            &tenant,
            &worker_id("worker-a"),
            batch_limit(1),
            Duration::from_secs(30),
        )
        .await?;
    assert_eq!(inserted.outbox_id, first_claim[0].outbox_id);
    assert_eq!(1, first_claim[0].attempts);

    let retry = outbox
        .schedule_retry(
            &tenant,
            inserted.outbox_id,
            "temporary failure",
            retry_policy(2),
        )
        .await?;
    assert_eq!(RetryScheduleOutcome::RetryScheduled, retry);

    let second_claim = outbox
        .claim_pending(
            &tenant,
            &worker_id("worker-b"),
            batch_limit(1),
            Duration::from_secs(30),
        )
        .await?;
    assert_eq!(2, second_claim[0].attempts);

    let failed = outbox
        .schedule_retry(
            &tenant,
            inserted.outbox_id,
            "permanent failure",
            retry_policy(2),
        )
        .await?;
    assert_eq!(RetryScheduleOutcome::Failed, failed);

    let remaining = outbox
        .claim_pending(
            &tenant,
            &worker_id("worker-c"),
            batch_limit(1),
            Duration::from_secs(30),
        )
        .await?;
    assert!(remaining.is_empty());

    Ok(())
}

#[tokio::test]
async fn outbox_repository_filters_by_tenant() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant_a = tenant_id("tenant-a");
    let tenant_b = tenant_id("tenant-b");
    let (tenant_a_event_id, tenant_a_position) =
        append_source_event(&events, tenant_a.clone(), "order-a", 500).await?;
    let (tenant_b_event_id, tenant_b_position) =
        append_source_event(&events, tenant_b.clone(), "order-b", 600).await?;
    let tenant_a_row = outbox
        .insert_outbox_message(
            &tenant_a,
            &new_outbox_message(tenant_a_event_id, "orders.placed"),
            tenant_a_position,
        )
        .await?;
    let tenant_b_row = outbox
        .insert_outbox_message(
            &tenant_b,
            &new_outbox_message(tenant_b_event_id, "orders.placed"),
            tenant_b_position,
        )
        .await?;

    let claimed_a = outbox
        .claim_pending(
            &tenant_a,
            &worker_id("worker-a"),
            batch_limit(10),
            Duration::from_secs(30),
        )
        .await?;

    assert_eq!(
        vec![tenant_a_row.outbox_id],
        claimed_a
            .iter()
            .map(|row| row.outbox_id)
            .collect::<Vec<_>>()
    );
    assert!(claimed_a.iter().all(|row| row.tenant_id == tenant_a));

    let claimed_b = outbox
        .claim_pending(
            &tenant_b,
            &worker_id("worker-b"),
            batch_limit(10),
            Duration::from_secs(30),
        )
        .await?;
    assert_eq!(
        vec![tenant_b_row.outbox_id],
        claimed_b
            .iter()
            .map(|row| row.outbox_id)
            .collect::<Vec<_>>()
    );
    assert!(claimed_b.iter().all(|row| row.tenant_id == tenant_b));

    Ok(())
}

#[tokio::test]
async fn outbox_process_manager_offsets_are_monotonic() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let process_manager = process_manager_name("order-fulfillment");

    assert_eq!(
        None,
        outbox
            .process_manager_offset(&tenant, &process_manager)
            .await?
    );

    outbox
        .advance_process_manager_offset(&tenant, &process_manager, 20)
        .await?;
    outbox
        .advance_process_manager_offset(&tenant, &process_manager, 10)
        .await?;

    assert_eq!(
        Some(20),
        outbox
            .process_manager_offset(&tenant, &process_manager)
            .await?
    );

    Ok(())
}

//! PostgreSQL outbox integration tests.

mod common;

use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_outbox::{
    DispatchBatchLimit, DispatchOutcome, InMemoryPublisher, MessageKey, NewOutboxMessage,
    PendingSourceEventRef, ProcessEvent, ProcessManager, ProcessManagerName, RetryPolicy,
    RetryScheduleOutcome, Topic, WorkerId, dispatch_once, process_committed_batch,
};
use es_store_postgres::{
    AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, PostgresOutboxStore, StoreError,
};
use futures::future::BoxFuture;
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

fn new_event(seed: u128, event_type: &str) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        json!({ "seed": seed }),
        json!({ "source": "append-outbox-test" }),
    )
    .expect("valid event")
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

async fn outbox_row_count(pool: &sqlx::PgPool, tenant_id: &str) -> anyhow::Result<i64> {
    let count =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM outbox_messages WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(pool)
            .await?;

    Ok(count)
}

struct ReplyingProcessManager {
    name: ProcessManagerName,
    seen: Arc<StdMutex<Vec<ProcessEvent>>>,
}

impl ReplyingProcessManager {
    fn new(name: ProcessManagerName) -> Self {
        Self {
            name,
            seen: Arc::new(StdMutex::new(Vec::new())),
        }
    }
}

impl ProcessManager for ReplyingProcessManager {
    fn name(&self) -> &ProcessManagerName {
        &self.name
    }

    fn handles(&self, event_type: &str, schema_version: i32) -> bool {
        event_type == "OrderPlaced" && schema_version == 1
    }

    fn process<'a>(
        &'a self,
        event: &'a ProcessEvent,
    ) -> BoxFuture<'a, es_outbox::OutboxResult<es_outbox::ProcessOutcome>> {
        self.seen
            .lock()
            .expect("seen process events")
            .push(event.clone());
        Box::pin(async move {
            Ok(es_outbox::ProcessOutcome::CommandsSubmitted {
                global_position: event.global_position,
                command_count: 2,
            })
        })
    }
}

#[tokio::test]
async fn append_creates_outbox_rows_atomically() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let event = new_event(700, "OrderPlaced");
    let message = new_outbox_message(event.event_id, "orders.placed");

    let outcome = store
        .append(AppendRequest::new_with_outbox(
            stream_id("order-append-outbox"),
            ExpectedRevision::NoStream,
            command_metadata(tenant.clone(), 700),
            "append-outbox-command",
            vec![event],
            vec![message],
        )?)
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append with outbox should commit");
    };
    let source_global_position = *committed
        .global_positions
        .first()
        .expect("committed global position");

    let row = sqlx::query_as::<_, (String, Uuid, i64, String, String, String)>(
        r#"
        SELECT tenant_id, source_event_id, source_global_position, topic, message_key, status
        FROM outbox_messages
        WHERE tenant_id = $1
        "#,
    )
    .bind("tenant-a")
    .fetch_one(&harness.pool)
    .await?;

    assert_eq!("tenant-a", row.0);
    assert_eq!(Uuid::from_u128(700), row.1);
    assert_eq!(source_global_position, row.2);
    assert_eq!("orders.placed", row.3);
    assert_eq!(format!("key-{}", Uuid::from_u128(700)), row.4);
    assert_eq!("pending", row.5);

    Ok(())
}

#[tokio::test]
async fn append_duplicate_replay_does_not_duplicate_outbox_rows() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let event = new_event(800, "OrderPlaced");
    let message = new_outbox_message(event.event_id, "orders.placed");

    let first = store
        .append(AppendRequest::new_with_outbox(
            stream_id("order-duplicate-outbox"),
            ExpectedRevision::NoStream,
            command_metadata(tenant.clone(), 800),
            "duplicate-outbox-command",
            vec![event.clone()],
            vec![message.clone()],
        )?)
        .await?;
    let duplicate = store
        .append(AppendRequest::new_with_outbox(
            stream_id("order-duplicate-outbox"),
            ExpectedRevision::NoStream,
            command_metadata(tenant.clone(), 801),
            "duplicate-outbox-command",
            vec![new_event(801, "OrderPlaced")],
            vec![new_outbox_message(Uuid::from_u128(801), "orders.placed")],
        )?)
        .await?;

    assert!(matches!(first, AppendOutcome::Committed(_)));
    assert!(matches!(duplicate, AppendOutcome::Duplicate(_)));
    assert_eq!(1, outbox_row_count(&harness.pool, "tenant-a").await?);

    Ok(())
}

#[tokio::test]
async fn append_conflict_rolls_back_outbox_rows() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");

    store
        .append(AppendRequest::new(
            stream_id("order-conflict-outbox"),
            ExpectedRevision::NoStream,
            command_metadata(tenant.clone(), 900),
            "seed-conflict-stream",
            vec![new_event(900, "OrderPlaced")],
        )?)
        .await?;

    let conflicting_event = new_event(901, "OrderConfirmed");
    let error = store
        .append(AppendRequest::new_with_outbox(
            stream_id("order-conflict-outbox"),
            ExpectedRevision::Exact(StreamRevision::new(99)),
            command_metadata(tenant, 901),
            "conflicting-outbox-command",
            vec![conflicting_event.clone()],
            vec![new_outbox_message(
                conflicting_event.event_id,
                "orders.confirmed",
            )],
        )?)
        .await
        .expect_err("wrong expected revision should conflict");

    assert!(matches!(error, StoreError::StreamConflict { .. }));
    assert_eq!(0, outbox_row_count(&harness.pool, "tenant-a").await?);

    Ok(())
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
async fn outbox_reclaims_expired_publishing_rows() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-1", 350).await?;
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
            Duration::from_secs(0),
        )
        .await?;
    assert_eq!(vec![inserted.outbox_id], vec![first_claim[0].outbox_id]);
    assert_eq!(1, first_claim[0].attempts);

    let second_claim = outbox
        .claim_pending(
            &tenant,
            &worker_id("worker-b"),
            batch_limit(1),
            Duration::from_secs(30),
        )
        .await?;

    assert_eq!(vec![inserted.outbox_id], vec![second_claim[0].outbox_id]);
    assert_eq!(2, second_claim[0].attempts);
    assert_eq!(
        Some("worker-b"),
        second_claim[0].locked_by.as_ref().map(WorkerId::as_str)
    );

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

#[tokio::test]
async fn process_manager_advances_postgres_offset_after_gateway_replies() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let event_store = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let name = process_manager_name("commerce-order");
    let event_id = Uuid::from_u128(9010);
    let metadata = command_metadata(tenant.clone(), 9010);
    let payload = json!({
        "OrderPlaced": {
            "order_id": "order-1",
            "user_id": "user-1",
            "lines": [{
                "product_id": "product-1",
                "sku": "SKU-1",
                "quantity": 2,
                "product_available": true
            }]
        }
    });
    let event_metadata = json!({ "source": "process-manager-integration" });

    let outcome = event_store
        .append(AppendRequest::new(
            stream_id("order-process-manager"),
            ExpectedRevision::NoStream,
            metadata.clone(),
            "process-manager-source-command",
            vec![NewEvent::new(
                event_id,
                "OrderPlaced",
                1,
                payload.clone(),
                event_metadata.clone(),
            )?],
        )?)
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append should commit");
    };
    let source_global_position = committed.global_positions[0];
    let manager = ReplyingProcessManager::new(name.clone());

    process_committed_batch(
        &manager,
        &event_store,
        &outbox,
        tenant.clone(),
        DispatchBatchLimit::new(10)?,
    )
    .await?;

    assert_eq!(
        Some(source_global_position),
        outbox.process_manager_offset(&tenant, &name).await?
    );
    let seen = manager.seen.lock().expect("seen process events");
    assert_eq!(1, seen.len());
    assert_eq!(source_global_position, seen[0].global_position);
    assert_eq!(tenant, seen[0].tenant_id);
    assert_eq!(event_id, seen[0].event_id);
    assert_eq!(metadata.command_id, seen[0].command_id);
    assert_eq!(metadata.correlation_id, seen[0].correlation_id);
    assert_eq!(metadata.causation_id, seen[0].causation_id);
    assert_eq!(payload, seen[0].payload);
    assert_eq!(event_metadata, seen[0].metadata);

    Ok(())
}

#[tokio::test]
async fn dispatcher_marks_successful_rows_published() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-dispatch-success", 1_000).await?;
    let inserted = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(source_event_id, "orders.placed"),
            source_global_position,
        )
        .await?;
    let publisher = InMemoryPublisher::default();

    let outcome = dispatch_once(
        &outbox,
        &publisher,
        tenant.clone(),
        worker_id("worker-a"),
        batch_limit(10),
        retry_policy(2),
    )
    .await?;

    assert_eq!(DispatchOutcome::Published { published: 1 }, outcome);
    let status = sqlx::query_as::<_, (String, Option<String>, Option<OffsetDateTime>)>(
        "SELECT status, locked_by, published_at FROM outbox_messages WHERE outbox_id = $1",
    )
    .bind(inserted.outbox_id)
    .fetch_one(&harness.pool)
    .await?;
    assert_eq!("published", status.0);
    assert_eq!(None, status.1);
    assert!(status.2.is_some());
    assert_eq!(1, publisher.published().len());

    Ok(())
}

#[tokio::test]
async fn dispatcher_schedules_failed_publish_for_retry() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-dispatch-retry", 1_100).await?;
    let inserted = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(source_event_id, "orders.placed"),
            source_global_position,
        )
        .await?;
    let publisher = InMemoryPublisher::default();
    publisher.push_failure("broker down");

    let outcome = dispatch_once(
        &outbox,
        &publisher,
        tenant.clone(),
        worker_id("worker-a"),
        batch_limit(10),
        retry_policy(2),
    )
    .await?;

    assert_eq!(
        DispatchOutcome::Partial {
            published: 0,
            retried: 1,
            failed: 0
        },
        outcome
    );
    let row = sqlx::query_as::<_, (String, i32, Option<String>, Option<String>)>(
        "SELECT status, attempts, locked_by, last_error FROM outbox_messages WHERE outbox_id = $1",
    )
    .bind(inserted.outbox_id)
    .fetch_one(&harness.pool)
    .await?;
    assert_eq!("pending", row.0);
    assert_eq!(1, row.1);
    assert_eq!(None, row.2);
    assert_eq!(Some("publisher error: broker down".to_owned()), row.3);

    Ok(())
}

#[tokio::test]
async fn dispatcher_reports_failed_rows_at_max_attempts() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let outbox = PostgresOutboxStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let (source_event_id, source_global_position) =
        append_source_event(&events, tenant.clone(), "order-dispatch-failed", 1_200).await?;
    let inserted = outbox
        .insert_outbox_message(
            &tenant,
            &new_outbox_message(source_event_id, "orders.placed"),
            source_global_position,
        )
        .await?;
    let publisher = InMemoryPublisher::default();
    publisher.push_failure("broker down");

    let outcome = dispatch_once(
        &outbox,
        &publisher,
        tenant.clone(),
        worker_id("worker-a"),
        batch_limit(10),
        retry_policy(1),
    )
    .await?;

    assert_eq!(
        DispatchOutcome::Partial {
            published: 0,
            retried: 0,
            failed: 1
        },
        outcome
    );
    let row = sqlx::query_as::<_, (String, i32, Option<String>, Option<String>)>(
        "SELECT status, attempts, locked_by, last_error FROM outbox_messages WHERE outbox_id = $1",
    )
    .bind(inserted.outbox_id)
    .fetch_one(&harness.pool)
    .await?;
    assert_eq!("failed", row.0);
    assert_eq!(1, row.1);
    assert_eq!(None, row.2);
    assert_eq!(Some("publisher error: broker down".to_owned()), row.3);

    Ok(())
}

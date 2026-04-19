//! Durable tenant-scoped command deduplication integration tests.

mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_store_postgres::{
    AppendOutcome, AppendRequest, CommandReplyPayload, CommittedAppend, NewEvent,
    PostgresEventStore,
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

fn new_event(seed: u128, order_id: &str) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        "OrderPlaced",
        1,
        json!({ "order_id": order_id }),
        json!({ "source": "dedupe" }),
    )
    .expect("valid event")
}

fn command_reply_payload(label: &str) -> CommandReplyPayload {
    CommandReplyPayload::new(
        "counter_added",
        1,
        json!({ "reply": label }),
    )
    .expect("valid command reply payload")
}

fn append_request(
    tenant: TenantId,
    stream: StreamId,
    idempotency_key: &str,
    command_seed: u128,
    event_seed: u128,
) -> AppendRequest {
    AppendRequest::new(
        stream,
        ExpectedRevision::NoStream,
        command_metadata(tenant, command_seed),
        idempotency_key,
        vec![new_event(event_seed, "order-1")],
    )
    .expect("valid append request")
}

fn committed(outcome: AppendOutcome) -> CommittedAppend {
    match outcome {
        AppendOutcome::Committed(committed) | AppendOutcome::Duplicate(committed) => committed,
    }
}

async fn event_count(pool: &sqlx::PgPool, tenant_id: &str, stream_id: &str) -> anyhow::Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM events WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind(tenant_id)
    .bind(stream_id)
    .fetch_one(pool)
    .await?;

    Ok(count)
}

#[tokio::test]
async fn command_replay_record_round_trips_from_response_payload() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");
    let request = append_request(
        tenant.clone(),
        stream.clone(),
        "idempotency-1",
        10,
        100,
    )
    .with_command_reply_payload(command_reply_payload("first"));

    let first = store.append(request).await?;
    let duplicate = store
        .append(append_request(tenant.clone(), stream, "idempotency-1", 20, 101))
        .await?;
    let first_committed = committed(first);
    let duplicate_committed = committed(duplicate);
    let replay = store
        .lookup_command_replay(&tenant, "idempotency-1")
        .await?
        .expect("typed replay record");

    assert_eq!(first_committed, duplicate_committed);
    assert_eq!(first_committed, replay.append);
    assert_eq!(json!({"reply":"first"}), replay.reply.payload);

    Ok(())
}

#[tokio::test]
async fn lookup_command_replay_returns_original_reply_after_store_recreation() -> anyhow::Result<()>
{
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");
    let first = store
        .append(
            append_request(
                tenant.clone(),
                stream,
                "idempotency-1",
                10,
                100,
            )
            .with_command_reply_payload(command_reply_payload("first")),
        )
        .await?;
    let first_committed = committed(first);
    let store_after_restart = PostgresEventStore::new(harness.pool.clone());

    let replay = store_after_restart
        .lookup_command_replay(&tenant, "idempotency-1")
        .await?
        .expect("typed replay record after restart");

    assert_eq!(json!({"reply":"first"}), replay.reply.payload);
    assert_eq!("counter_added", replay.reply.reply_type);
    assert_eq!(1, replay.reply.schema_version);
    assert_eq!(first_committed, replay.append);

    Ok(())
}

#[tokio::test]
async fn lookup_command_replay_is_tenant_scoped() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    store
        .append(
            append_request(
                tenant.clone(),
                stream_id("order-1"),
                "idempotency-1",
                10,
                100,
            )
            .with_command_reply_payload(command_reply_payload("first")),
        )
        .await?;

    let replay = store
        .lookup_command_replay(&tenant_id("tenant-b"), "idempotency-1")
        .await?;

    assert_eq!(None, replay);

    Ok(())
}

#[tokio::test]
async fn duplicate_idempotency_key_returns_original_result() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    let first = store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            "idempotency-1",
            10,
            100,
        ))
        .await?;
    let second = store
        .append(append_request(tenant, stream, "idempotency-1", 20, 101))
        .await?;

    let AppendOutcome::Committed(first_committed) = first else {
        panic!("first append should commit");
    };
    let AppendOutcome::Duplicate(second_committed) = second else {
        panic!("duplicate append should return original result");
    };

    assert_eq!(first_committed, second_committed);

    Ok(())
}

#[tokio::test]
async fn duplicate_idempotency_key_does_not_append_events() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let stream = stream_id("order-1");

    store
        .append(append_request(
            tenant.clone(),
            stream.clone(),
            "idempotency-1",
            10,
            100,
        ))
        .await?;
    store
        .append(append_request(tenant, stream, "idempotency-1", 20, 101))
        .await?;

    assert_eq!(1, event_count(&harness.pool, "tenant-a", "order-1").await?);

    Ok(())
}

#[tokio::test]
async fn idempotency_key_is_scoped_by_tenant() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = PostgresEventStore::new(harness.pool.clone());

    let first = store
        .append(append_request(
            tenant_id("tenant-a"),
            stream_id("order-1"),
            "idempotency-1",
            10,
            100,
        ))
        .await?;
    let second = store
        .append(append_request(
            tenant_id("tenant-b"),
            stream_id("order-1"),
            "idempotency-1",
            20,
            101,
        ))
        .await?;

    assert!(matches!(first, AppendOutcome::Committed(_)));
    assert!(matches!(second, AppendOutcome::Committed(_)));
    assert_eq!(1, event_count(&harness.pool, "tenant-a", "order-1").await?);
    assert_eq!(1, event_count(&harness.pool, "tenant-b", "order-1").await?);

    Ok(())
}

#[tokio::test]
async fn concurrent_duplicate_idempotency_key_appends_only_once() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store_a = PostgresEventStore::new(harness.pool.clone());
    let store_b = PostgresEventStore::new(harness.pool.clone());
    let request = append_request(
        tenant_id("tenant-a"),
        stream_id("order-1"),
        "idempotency-1",
        10,
        100,
    );

    let (outcome_a, outcome_b) = tokio::join!(
        store_a.append(request.clone()),
        store_b.append(request.clone())
    );
    let outcome_a = outcome_a?;
    let outcome_b = outcome_b?;

    assert!(matches!(
        (&outcome_a, &outcome_b),
        (AppendOutcome::Committed(_), AppendOutcome::Duplicate(_))
            | (AppendOutcome::Duplicate(_), AppendOutcome::Committed(_))
    ));
    assert_eq!(committed(outcome_a), committed(outcome_b));
    assert_eq!(1, event_count(&harness.pool, "tenant-a", "order-1").await?);

    Ok(())
}

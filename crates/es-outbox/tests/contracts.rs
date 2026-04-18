use es_core::TenantId;
use es_outbox::{
    DispatchBatchLimit, InMemoryPublisher, MessageKey, OutboxError, OutboxMessage, OutboxStatus,
    PendingSourceEventRef, ProcessManagerName, Publisher, RetryPolicy, SourceEventRef, Topic,
    WorkerId,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

#[test]
fn string_newtypes_reject_empty_values() {
    assert_eq!(Topic::new(""), Err(OutboxError::InvalidTopic));
    assert_eq!(MessageKey::new(""), Err(OutboxError::InvalidMessageKey));
    assert_eq!(WorkerId::new(""), Err(OutboxError::InvalidWorkerId));
    assert_eq!(
        ProcessManagerName::new(""),
        Err(OutboxError::InvalidProcessManagerName)
    );
}

#[test]
fn source_references_distinguish_pre_append_and_persisted_rows() {
    let event_id = Uuid::now_v7();
    let pending = PendingSourceEventRef::new(event_id);
    assert_eq!(pending.event_id(), event_id);

    assert_eq!(
        SourceEventRef::new(event_id, 0),
        Err(OutboxError::InvalidSourceGlobalPosition { value: 0 })
    );

    let source = SourceEventRef::new(event_id, 1).expect("valid source");
    assert_eq!(source.event_id(), event_id);
    assert_eq!(source.global_position(), 1);
}

#[test]
fn limits_and_retry_policy_are_bounded() {
    assert_eq!(
        DispatchBatchLimit::new(0),
        Err(OutboxError::InvalidBatchLimit { value: 0 })
    );
    assert_eq!(
        DispatchBatchLimit::new(1001),
        Err(OutboxError::InvalidBatchLimit { value: 1001 })
    );
    assert_eq!(DispatchBatchLimit::new(1000).expect("valid").value(), 1000);

    assert_eq!(
        RetryPolicy::new(0),
        Err(OutboxError::InvalidRetryPolicy { max_attempts: 0 })
    );
    assert_eq!(RetryPolicy::new(3).expect("valid").max_attempts(), 3);
}

#[test]
fn outbox_message_builds_deterministic_publish_envelope() {
    let tenant_id = TenantId::new("tenant-a").expect("tenant");
    let source_event_id = Uuid::now_v7();
    let topic = Topic::new("commerce.order-events").expect("topic");
    let message_key = MessageKey::new("order-123").expect("message key");
    let now = OffsetDateTime::UNIX_EPOCH;
    let message = OutboxMessage {
        outbox_id: Uuid::now_v7(),
        tenant_id,
        source: SourceEventRef::new(source_event_id, 42).expect("source"),
        topic,
        message_key,
        payload: json!({ "order_id": "order-123" }),
        metadata: json!({ "schema": 1 }),
        status: OutboxStatus::Pending,
        attempts: 0,
        available_at: now,
        locked_by: None,
        locked_until: None,
        published_at: None,
        last_error: None,
        created_at: now,
        updated_at: now,
    };

    let expected_key = format!("tenant-a:commerce.order-events:{source_event_id}");
    assert_eq!(message.idempotency_key(), expected_key);

    let envelope = message.publish_envelope();
    assert_eq!(envelope.topic, "commerce.order-events");
    assert_eq!(envelope.message_key, "order-123");
    assert_eq!(envelope.idempotency_key, expected_key);
    assert_eq!(envelope.payload, json!({ "order_id": "order-123" }));
    assert_eq!(envelope.metadata, json!({ "schema": 1 }));
}

#[test]
fn in_memory_publisher_records_one_external_effect_per_idempotency_key() {
    futures::executor::block_on(async {
        let publisher = InMemoryPublisher::default();
        let message = outbox_message();
        let envelope = message.publish_envelope();

        publisher
            .publish(envelope.clone())
            .await
            .expect("first publish succeeds");
        publisher
            .publish(envelope)
            .await
            .expect("duplicate publish succeeds");

        assert_eq!(publisher.published().len(), 1);
    });
}

#[test]
fn in_memory_publisher_can_queue_failures() {
    futures::executor::block_on(async {
        let publisher = InMemoryPublisher::default();
        publisher.push_failure("broker unavailable");

        let error = publisher
            .publish(outbox_message().publish_envelope())
            .await
            .expect_err("queued failure");
        assert_eq!(
            error,
            OutboxError::Publisher {
                message: "broker unavailable".to_owned()
            }
        );
        assert!(publisher.published().is_empty());
    });
}

fn outbox_message() -> OutboxMessage {
    let now = OffsetDateTime::UNIX_EPOCH;
    OutboxMessage {
        outbox_id: Uuid::now_v7(),
        tenant_id: TenantId::new("tenant-a").expect("tenant"),
        source: SourceEventRef::new(Uuid::now_v7(), 42).expect("source"),
        topic: Topic::new("commerce.order-events").expect("topic"),
        message_key: MessageKey::new("order-123").expect("message key"),
        payload: json!({ "order_id": "order-123" }),
        metadata: json!({ "schema": 1 }),
        status: OutboxStatus::Pending,
        attempts: 0,
        available_at: now,
        locked_by: None,
        locked_until: None,
        published_at: None,
        last_error: None,
        created_at: now,
        updated_at: now,
    }
}

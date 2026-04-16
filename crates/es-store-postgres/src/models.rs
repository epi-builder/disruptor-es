#[cfg(test)]
mod models {
    use super::*;
    use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn command_metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: Some(Uuid::from_u128(3)),
            tenant_id: TenantId::new("tenant-a").expect("valid tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
                .expect("valid timestamp"),
        }
    }

    fn valid_event() -> NewEvent {
        NewEvent::new(
            Uuid::from_u128(10),
            "OrderPlaced",
            1,
            json!({ "order_id": "order-1" }),
            json!({ "source": "test" }),
        )
        .expect("valid event")
    }

    #[test]
    fn append_request_rejects_empty_events() {
        let error = AppendRequest::new(
            StreamId::new("order-1").expect("valid stream id"),
            ExpectedRevision::NoStream,
            command_metadata(),
            "command-1",
            Vec::new(),
        )
        .expect_err("empty append rejected");

        assert!(matches!(error, StoreError::EmptyAppend));
    }

    #[test]
    fn new_event_rejects_empty_event_type() {
        let error = NewEvent::new(
            Uuid::from_u128(10),
            "",
            1,
            json!({ "order_id": "order-1" }),
            json!({ "source": "test" }),
        )
        .expect_err("empty event type rejected");

        assert!(matches!(error, StoreError::InvalidEventType));
    }

    #[test]
    fn new_event_rejects_zero_schema_version() {
        let error = NewEvent::new(
            Uuid::from_u128(10),
            "OrderPlaced",
            0,
            json!({ "order_id": "order-1" }),
            json!({ "source": "test" }),
        )
        .expect_err("zero schema version rejected");

        assert!(matches!(
            error,
            StoreError::InvalidSchemaVersion { schema_version: 0 }
        ));
    }

    #[test]
    fn new_event_rejects_payload_larger_than_limit() {
        let payload = json!({ "bytes": "x".repeat(MAX_JSON_PAYLOAD_BYTES) });
        let error = NewEvent::new(
            Uuid::from_u128(10),
            "OrderPlaced",
            1,
            payload,
            json!({ "source": "test" }),
        )
        .expect_err("oversized payload rejected");

        assert!(matches!(
            error,
            StoreError::PayloadTooLarge {
                actual_bytes,
                max_bytes: MAX_JSON_PAYLOAD_BYTES,
            } if actual_bytes > MAX_JSON_PAYLOAD_BYTES
        ));
    }

    #[test]
    fn append_request_accepts_valid_event_and_idempotency_key() {
        let request = AppendRequest::new(
            StreamId::new("order-1").expect("valid stream id"),
            ExpectedRevision::NoStream,
            command_metadata(),
            "command-1",
            vec![valid_event()],
        )
        .expect("valid append request");

        assert_eq!("order-1", request.stream_id.as_str());
        assert_eq!("command-1", request.idempotency_key);
        assert_eq!(1, request.events.len());
    }
}

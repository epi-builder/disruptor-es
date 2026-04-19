use std::collections::HashSet;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_outbox::NewOutboxMessage;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{StoreError, StoreResult};

/// Maximum accepted serialized JSON payload size for one event.
pub const MAX_JSON_PAYLOAD_BYTES: usize = 1_048_576;

/// New domain event ready to be appended to a stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NewEvent {
    /// Unique event identifier generated before persistence.
    pub event_id: Uuid,
    /// Stable event type name.
    pub event_type: String,
    /// Positive payload schema version.
    pub schema_version: i32,
    /// JSON payload stored in PostgreSQL JSONB.
    pub payload: serde_json::Value,
    /// JSON metadata stored beside the event payload.
    pub metadata: serde_json::Value,
}

impl NewEvent {
    /// Creates a validated event append DTO.
    pub fn new(
        event_id: Uuid,
        event_type: impl Into<String>,
        schema_version: i32,
        payload: serde_json::Value,
        metadata: serde_json::Value,
    ) -> StoreResult<Self> {
        let event_type = event_type.into();
        if event_type.is_empty() {
            return Err(StoreError::InvalidEventType);
        }
        if schema_version <= 0 {
            return Err(StoreError::InvalidSchemaVersion { schema_version });
        }

        let actual_bytes = serde_json::to_vec(&payload)
            .expect("serializing serde_json::Value to bytes cannot fail")
            .len();
        if actual_bytes > MAX_JSON_PAYLOAD_BYTES {
            return Err(StoreError::PayloadTooLarge {
                actual_bytes,
                max_bytes: MAX_JSON_PAYLOAD_BYTES,
            });
        }

        Ok(Self {
            event_id,
            event_type,
            schema_version,
            payload,
            metadata,
        })
    }
}

/// Request to append one or more events to a stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AppendRequest {
    /// Stream receiving the events.
    pub stream_id: StreamId,
    /// Optimistic-concurrency expectation for the stream.
    pub expected_revision: ExpectedRevision,
    /// Command metadata, including the tenant that owns the append.
    pub command_metadata: CommandMetadata,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
    /// Events to append atomically.
    pub events: Vec<NewEvent>,
    /// Outbox messages derived from the appended events.
    pub outbox_messages: Vec<NewOutboxMessage>,
}

impl AppendRequest {
    /// Creates a validated append request.
    pub fn new(
        stream_id: StreamId,
        expected_revision: ExpectedRevision,
        command_metadata: CommandMetadata,
        idempotency_key: impl Into<String>,
        events: Vec<NewEvent>,
    ) -> StoreResult<Self> {
        Self::new_with_outbox(
            stream_id,
            expected_revision,
            command_metadata,
            idempotency_key,
            events,
            Vec::new(),
        )
    }

    /// Creates a validated append request with derived outbox messages.
    pub fn new_with_outbox(
        stream_id: StreamId,
        expected_revision: ExpectedRevision,
        command_metadata: CommandMetadata,
        idempotency_key: impl Into<String>,
        events: Vec<NewEvent>,
        outbox_messages: Vec<NewOutboxMessage>,
    ) -> StoreResult<Self> {
        if events.is_empty() {
            return Err(StoreError::EmptyAppend);
        }

        let idempotency_key = idempotency_key.into();
        if idempotency_key.is_empty() {
            return Err(StoreError::InvalidIdempotencyKey);
        }

        let event_ids = events
            .iter()
            .map(|event| event.event_id)
            .collect::<HashSet<_>>();
        for message in &outbox_messages {
            let source_event_id = message.source.event_id();
            if !event_ids.contains(&source_event_id) {
                return Err(StoreError::InvalidOutboxSourceEvent { source_event_id });
            }
        }

        Ok(Self {
            stream_id,
            expected_revision,
            command_metadata,
            idempotency_key,
            events,
            outbox_messages,
        })
    }
}

/// Result data for a committed append.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CommittedAppend {
    /// Stream that received the append.
    pub stream_id: StreamId,
    /// First stream revision written by the append.
    pub first_revision: StreamRevision,
    /// Last stream revision written by the append.
    pub last_revision: StreamRevision,
    /// Durable global positions assigned to the appended events.
    pub global_positions: Vec<i64>,
    /// Event identifiers committed by the append.
    pub event_ids: Vec<Uuid>,
}

/// Outcome of an append request after idempotency handling.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum AppendOutcome {
    /// New events were committed.
    Committed(CommittedAppend),
    /// A prior result was returned for the same idempotency key.
    Duplicate(CommittedAppend),
}

/// Event row read from durable storage.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StoredEvent {
    /// Durable global position assigned by PostgreSQL.
    pub global_position: i64,
    /// Stream that owns the event.
    pub stream_id: StreamId,
    /// Stream revision for this event.
    pub stream_revision: StreamRevision,
    /// Unique event identifier.
    pub event_id: Uuid,
    /// Stable event type name.
    pub event_type: String,
    /// Positive payload schema version.
    pub schema_version: i32,
    /// JSON payload stored in PostgreSQL JSONB.
    pub payload: serde_json::Value,
    /// JSON metadata stored beside the event payload.
    pub metadata: serde_json::Value,
    /// Tenant that owns the event.
    pub tenant_id: TenantId,
    /// Command that produced the event.
    pub command_id: Uuid,
    /// Correlation identifier shared by related processing.
    pub correlation_id: Uuid,
    /// Optional command or event that caused this event.
    pub causation_id: Option<Uuid>,
    /// Time the event was durably recorded.
    pub recorded_at: OffsetDateTime,
}

/// Latest durable aggregate snapshot for a stream.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SnapshotRecord {
    /// Tenant that owns the snapshot.
    pub tenant_id: TenantId,
    /// Stream that owns the snapshot.
    pub stream_id: StreamId,
    /// Stream revision captured by the snapshot.
    pub stream_revision: StreamRevision,
    /// Snapshot state payload stored in PostgreSQL JSONB.
    pub state_payload: serde_json::Value,
    /// Snapshot metadata stored beside the state payload.
    pub metadata: serde_json::Value,
    /// Time the snapshot was saved.
    pub recorded_at: OffsetDateTime,
}

/// Request to save a stream snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SaveSnapshotRequest {
    /// Tenant that owns the snapshot.
    pub tenant_id: TenantId,
    /// Stream that owns the snapshot.
    pub stream_id: StreamId,
    /// Stream revision captured by the snapshot.
    pub stream_revision: StreamRevision,
    /// Snapshot state payload stored in PostgreSQL JSONB.
    pub state_payload: serde_json::Value,
    /// Snapshot metadata stored beside the state payload.
    pub metadata: serde_json::Value,
}

/// Latest snapshot plus subsequent stream events for aggregate rehydration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RehydrationBatch {
    /// Latest snapshot, if one exists.
    pub snapshot: Option<SnapshotRecord>,
    /// Events after the snapshot revision, or all events if no snapshot exists.
    pub events: Vec<StoredEvent>,
}

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
    fn command_reply_payload_rejects_empty_reply_type() {
        let error = CommandReplyPayload::new("", 1, json!({}))
            .expect_err("empty reply type rejected");

        assert!(matches!(error, StoreError::InvalidReplyType));
    }

    #[test]
    fn command_reply_payload_rejects_non_positive_schema_version() {
        let error = CommandReplyPayload::new("order_placed", 0, json!({}))
            .expect_err("non-positive schema version rejected");

        assert!(matches!(
            error,
            StoreError::InvalidSchemaVersion { schema_version: 0 }
        ));
    }

    #[test]
    fn append_request_can_attach_command_reply_payload() {
        let reply = CommandReplyPayload::new(
            "order_placed",
            1,
            json!({ "order_id": "order-1" }),
        )
        .expect("valid reply payload");
        let request = AppendRequest::new(
            StreamId::new("order-1").expect("valid stream id"),
            ExpectedRevision::NoStream,
            command_metadata(),
            "command-1",
            vec![valid_event()],
        )
        .expect("valid append request")
        .with_command_reply_payload(reply.clone());

        assert_eq!(Some(reply), request.command_reply_payload);
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
        assert!(request.outbox_messages.is_empty());
    }

    #[test]
    fn append_request_rejects_unknown_outbox_source_event() {
        let source_event_id = Uuid::from_u128(99);
        let message = es_outbox::NewOutboxMessage::new(
            es_outbox::PendingSourceEventRef::new(source_event_id),
            es_outbox::Topic::new("orders.placed").expect("valid topic"),
            es_outbox::MessageKey::new("order-1").expect("valid message key"),
            json!({ "order_id": "order-1" }),
            json!({ "source": "test" }),
        );
        let error = AppendRequest::new_with_outbox(
            StreamId::new("order-1").expect("valid stream id"),
            ExpectedRevision::NoStream,
            command_metadata(),
            "command-1",
            vec![valid_event()],
            vec![message],
        )
        .expect_err("unknown outbox source event rejected");

        assert!(matches!(
            error,
            StoreError::InvalidOutboxSourceEvent { source_event_id: id } if id == source_event_id
        ));
    }
}

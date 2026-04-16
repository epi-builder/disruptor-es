//! Core event-sourcing identity, revision, and metadata contracts.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Errors returned by core value constructors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CoreError {
    /// A required string-backed value was empty.
    #[error("{type_name} cannot be empty")]
    EmptyValue {
        /// Name of the value type that rejected the empty input.
        type_name: &'static str,
    },
}

/// Durable event stream identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct StreamId(String);

impl StreamId {
    /// Creates a stream identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "StreamId").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Ordered partition routing key.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PartitionKey(String);

impl PartitionKey {
    /// Creates a partition key.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "PartitionKey").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Tenant identity attached to command and event metadata.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct TenantId(String);

impl TenantId {
    /// Creates a tenant identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "TenantId").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

fn string_value(value: impl Into<String>, type_name: &'static str) -> Result<String, CoreError> {
    let value = value.into();
    if value.is_empty() {
        return Err(CoreError::EmptyValue { type_name });
    }
    Ok(value)
}

/// Ordered stream revision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StreamRevision(u64);

impl StreamRevision {
    /// Creates a revision from its numeric value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric revision value.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Optimistic concurrency expectation for appending to a stream.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ExpectedRevision {
    /// Append regardless of current stream state.
    Any,
    /// Append only if the stream does not exist.
    NoStream,
    /// Append only if the stream is at the exact revision.
    Exact(StreamRevision),
}

/// Metadata supplied with a command before event recording.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandMetadata {
    /// Unique command identifier for idempotency and tracing.
    pub command_id: Uuid,
    /// Correlation identifier shared by related command processing.
    pub correlation_id: Uuid,
    /// Optional identifier for the command or event that caused this command.
    pub causation_id: Option<Uuid>,
    /// Tenant that owns the command.
    pub tenant_id: TenantId,
    /// Time the command was requested by the caller.
    pub requested_at: OffsetDateTime,
}

/// Metadata committed with a recorded event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventMetadata {
    /// Unique event identifier.
    pub event_id: Uuid,
    /// Command that produced the event.
    pub command_id: Uuid,
    /// Correlation identifier shared by related processing.
    pub correlation_id: Uuid,
    /// Optional command or event that caused this event.
    pub causation_id: Option<Uuid>,
    /// Tenant that owns the event.
    pub tenant_id: TenantId,
    /// Time the event was durably recorded.
    pub recorded_at: OffsetDateTime,
}

#[cfg(test)]
mod metadata_contracts {
    use super::*;

    #[test]
    fn constructors_return_valid_opaque_newtypes() {
        let stream_id = StreamId::new("order-1").expect("valid stream id");
        let partition_key = PartitionKey::new("order-1").expect("valid partition key");
        let tenant_id = TenantId::new("tenant-a").expect("valid tenant id");

        assert_eq!("order-1", stream_id.as_str());
        assert_eq!("order-1", partition_key.as_str());
        assert_eq!("tenant-a", tenant_id.as_str());
    }

    #[test]
    fn empty_strings_return_typed_errors() {
        assert_eq!(
            CoreError::EmptyValue {
                type_name: "StreamId",
            },
            StreamId::new("").expect_err("empty stream id")
        );
        assert_eq!(
            CoreError::EmptyValue {
                type_name: "PartitionKey",
            },
            PartitionKey::new("").expect_err("empty partition key")
        );
        assert_eq!(
            CoreError::EmptyValue {
                type_name: "TenantId",
            },
            TenantId::new("").expect_err("empty tenant id")
        );
    }

    #[test]
    fn exact_expected_revision_preserves_numeric_revision() {
        let expected = ExpectedRevision::Exact(StreamRevision::new(7));

        match expected {
            ExpectedRevision::Exact(revision) => assert_eq!(7, revision.value()),
            ExpectedRevision::Any | ExpectedRevision::NoStream => panic!("wrong revision variant"),
        }
    }

    #[test]
    fn command_metadata_requires_tenant_and_roundtrips_through_serde() {
        let metadata = CommandMetadata {
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: Some(Uuid::from_u128(3)),
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        };

        let encoded = serde_json::to_string(&metadata).expect("serialize metadata");
        let decoded: CommandMetadata =
            serde_json::from_str(&encoded).expect("deserialize metadata");

        assert_eq!(metadata, decoded);
    }
}

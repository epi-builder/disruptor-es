use es_core::TenantId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{OutboxError, OutboxResult, PublishEnvelope};

/// Validated external integration topic.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Topic(String);

impl Topic {
    /// Creates a topic.
    pub fn new(value: impl Into<String>) -> OutboxResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(OutboxError::InvalidTopic);
        }

        Ok(Self(value))
    }

    /// Returns the borrowed topic.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated message partition key for external publication.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct MessageKey(String);

impl MessageKey {
    /// Creates a message key.
    pub fn new(value: impl Into<String>) -> OutboxResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(OutboxError::InvalidMessageKey);
        }

        Ok(Self(value))
    }

    /// Returns the borrowed message key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated dispatcher worker identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct WorkerId(String);

impl WorkerId {
    /// Creates a worker identity.
    pub fn new(value: impl Into<String>) -> OutboxResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(OutboxError::InvalidWorkerId);
        }

        Ok(Self(value))
    }

    /// Returns the borrowed worker identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated process-manager identity for replayable workflow keys.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ProcessManagerName(String);

impl ProcessManagerName {
    /// Creates a process-manager name.
    pub fn new(value: impl Into<String>) -> OutboxResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(OutboxError::InvalidProcessManagerName);
        }

        Ok(Self(value))
    }

    /// Returns the borrowed process-manager name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Source event reference before storage knows the committed global position.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PendingSourceEventRef {
    /// Source event identifier.
    pub event_id: Uuid,
}

impl PendingSourceEventRef {
    /// Creates a pending source event reference.
    pub const fn new(event_id: Uuid) -> Self {
        Self { event_id }
    }

    /// Returns the source event identifier.
    pub const fn event_id(self) -> Uuid {
        self.event_id
    }
}

/// Source event reference for a persisted outbox row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SourceEventRef {
    /// Source event identifier.
    pub event_id: Uuid,
    /// Committed source event global position.
    pub global_position: i64,
}

impl SourceEventRef {
    /// Creates a persisted source event reference.
    pub fn new(event_id: Uuid, global_position: i64) -> OutboxResult<Self> {
        if global_position <= 0 {
            return Err(OutboxError::InvalidSourceGlobalPosition {
                value: global_position,
            });
        }

        Ok(Self {
            event_id,
            global_position,
        })
    }

    /// Returns the source event identifier.
    pub const fn event_id(self) -> Uuid {
        self.event_id
    }

    /// Returns the committed source global position.
    pub const fn global_position(self) -> i64 {
        self.global_position
    }
}

/// Bounded dispatch batch size.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DispatchBatchLimit(i64);

impl DispatchBatchLimit {
    /// Creates a dispatch batch limit.
    pub fn new(value: i64) -> OutboxResult<Self> {
        if !(1..=1000).contains(&value) {
            return Err(OutboxError::InvalidBatchLimit { value });
        }

        Ok(Self(value))
    }

    /// Returns the numeric batch limit.
    pub const fn value(self) -> i64 {
        self.0
    }
}

/// Retry policy for publisher failures.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetryPolicy {
    /// Maximum publish attempts before final failure.
    pub max_attempts: i32,
}

impl RetryPolicy {
    /// Creates a retry policy.
    pub fn new(max_attempts: i32) -> OutboxResult<Self> {
        if max_attempts < 1 {
            return Err(OutboxError::InvalidRetryPolicy { max_attempts });
        }

        Ok(Self { max_attempts })
    }

    /// Returns the maximum publish attempts.
    pub const fn max_attempts(self) -> i32 {
        self.max_attempts
    }
}

/// Durable outbox message status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum OutboxStatus {
    /// Message is ready to be claimed.
    Pending,
    /// Message has been claimed by a dispatcher.
    Publishing,
    /// Message has been published successfully.
    Published,
    /// Message exhausted retry attempts.
    Failed,
}

impl OutboxStatus {
    /// Returns the storage representation of the status.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Publishing => "publishing",
            Self::Published => "published",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for OutboxStatus {
    type Error = OutboxError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "pending" => Ok(Self::Pending),
            "publishing" => Ok(Self::Publishing),
            "published" => Ok(Self::Published),
            "failed" => Ok(Self::Failed),
            other => Err(OutboxError::InvalidStatus {
                status: other.to_owned(),
            }),
        }
    }
}

/// Storage outcome when scheduling a failed publish attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RetryScheduleOutcome {
    /// The row was scheduled for another publish attempt.
    RetryScheduled,
    /// The row transitioned to final failed status.
    Failed,
}

/// New outbox row requested before append storage fills the committed source position.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NewOutboxMessage {
    /// Source event before append persistence.
    pub source: PendingSourceEventRef,
    /// External integration topic.
    pub topic: Topic,
    /// External message key.
    pub message_key: MessageKey,
    /// External payload.
    pub payload: serde_json::Value,
    /// External metadata.
    pub metadata: serde_json::Value,
}

impl NewOutboxMessage {
    /// Creates a new outbox message request.
    pub const fn new(
        source: PendingSourceEventRef,
        topic: Topic,
        message_key: MessageKey,
        payload: serde_json::Value,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            source,
            topic,
            message_key,
            payload,
            metadata,
        }
    }
}

/// Persisted outbox message ready for dispatch or status transitions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OutboxMessage {
    /// Durable outbox row identifier.
    pub outbox_id: Uuid,
    /// Tenant that owns this outbox message.
    pub tenant_id: TenantId,
    /// Persisted source event reference.
    pub source: SourceEventRef,
    /// External integration topic.
    pub topic: Topic,
    /// External message key.
    pub message_key: MessageKey,
    /// External payload.
    pub payload: serde_json::Value,
    /// External metadata.
    pub metadata: serde_json::Value,
    /// Durable dispatch status.
    pub status: OutboxStatus,
    /// Number of publish attempts.
    pub attempts: i32,
    /// Earliest time this row is available for dispatch.
    pub available_at: OffsetDateTime,
    /// Dispatcher worker currently owning the row.
    pub locked_by: Option<WorkerId>,
    /// Lock expiration time.
    pub locked_until: Option<OffsetDateTime>,
    /// Successful publication time.
    pub published_at: Option<OffsetDateTime>,
    /// Last publisher error message.
    pub last_error: Option<String>,
    /// Row creation time.
    pub created_at: OffsetDateTime,
    /// Row update time.
    pub updated_at: OffsetDateTime,
}

impl OutboxMessage {
    /// Returns the deterministic external idempotency key for this message.
    pub fn idempotency_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.tenant_id.as_str(),
            self.topic.as_str(),
            self.source.event_id()
        )
    }

    /// Creates the publisher envelope for this message.
    pub fn publish_envelope(&self) -> PublishEnvelope {
        PublishEnvelope {
            topic: self.topic.as_str().to_owned(),
            message_key: self.message_key.as_str().to_owned(),
            idempotency_key: self.idempotency_key(),
            payload: self.payload.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

/// Outcome from one dispatcher pass.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DispatchOutcome {
    /// No messages were available.
    Idle,
    /// All claimed messages were published.
    Published {
        /// Number of messages published.
        published: usize,
    },
    /// Claimed messages had mixed publish/retry/failure outcomes.
    Partial {
        /// Number of messages published.
        published: usize,
        /// Number of messages scheduled for retry.
        retried: usize,
        /// Number of messages moved to final failed status.
        failed: usize,
    },
}

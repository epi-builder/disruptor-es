/// Result alias for outbox, publisher, dispatcher, and process-manager operations.
pub type OutboxResult<T> = Result<T, OutboxError>;

/// Errors returned by outbox contracts.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OutboxError {
    /// Topic was invalid.
    #[error("topic cannot be empty")]
    InvalidTopic,
    /// Message key was invalid.
    #[error("message key cannot be empty")]
    InvalidMessageKey,
    /// Worker identity was invalid.
    #[error("worker id cannot be empty")]
    InvalidWorkerId,
    /// Process-manager identity was invalid.
    #[error("process manager name cannot be empty")]
    InvalidProcessManagerName,
    /// Source event global position was invalid.
    #[error("source global position must be positive: {value}")]
    InvalidSourceGlobalPosition {
        /// Rejected source global position.
        value: i64,
    },
    /// Dispatch batch limit was invalid.
    #[error("dispatch batch limit must be between 1 and 1000: {value}")]
    InvalidBatchLimit {
        /// Rejected batch limit.
        value: i64,
    },
    /// Retry policy was invalid.
    #[error("retry policy max attempts must be positive: {max_attempts}")]
    InvalidRetryPolicy {
        /// Rejected retry max attempts.
        max_attempts: i32,
    },
    /// Stored status was invalid.
    #[error("invalid outbox status: {status}")]
    InvalidStatus {
        /// Rejected status value.
        status: String,
    },
    /// Publisher returned an infrastructure error.
    #[error("publisher error: {message}")]
    Publisher {
        /// Publisher error message.
        message: String,
    },
    /// Storage returned an infrastructure error.
    #[error("store error: {message}")]
    Store {
        /// Store error message.
        message: String,
    },
    /// Process-manager command submission failed.
    #[error("command submit error: {message}")]
    CommandSubmit {
        /// Command submit error message.
        message: String,
    },
    /// Process-manager command reply channel was dropped.
    #[error("command reply dropped")]
    CommandReplyDropped,
    /// Event payload could not be decoded.
    #[error("failed to decode payload for {event_type} schema version {schema_version}")]
    PayloadDecode {
        /// Event type being decoded.
        event_type: String,
        /// Event schema version being decoded.
        schema_version: i32,
    },
}

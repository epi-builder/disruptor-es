/// Result alias for PostgreSQL event-store operations.
pub type StoreResult<T> = Result<T, StoreError>;

/// Errors returned by the PostgreSQL event-store API.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Append request contained no events.
    #[error("append request must contain at least one event")]
    EmptyAppend,
    /// Event type was empty.
    #[error("event type cannot be empty")]
    InvalidEventType,
    /// Idempotency key was empty.
    #[error("idempotency key cannot be empty")]
    InvalidIdempotencyKey,
    /// Schema version was not positive.
    #[error("schema version must be positive, got {schema_version}")]
    InvalidSchemaVersion {
        /// Rejected schema version.
        schema_version: i32,
    },
    /// Serialized event payload exceeded the configured limit.
    #[error("payload is too large: {actual_bytes} bytes exceeds {max_bytes} bytes")]
    PayloadTooLarge {
        /// Serialized payload byte count.
        actual_bytes: usize,
        /// Maximum accepted serialized payload byte count.
        max_bytes: usize,
    },
    /// Stream revision did not match the requested optimistic-concurrency expectation.
    #[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
    StreamConflict {
        /// Conflicting stream identifier.
        stream_id: String,
        /// Expected revision description.
        expected: String,
        /// Actual stream revision, or `None` when the stream does not exist.
        actual: Option<u64>,
    },
    /// Stored revision from PostgreSQL could not be represented by core revision types.
    #[error("invalid stored stream revision {value}")]
    InvalidStoredRevision {
        /// Rejected revision value.
        value: i64,
    },
    /// Global event position from PostgreSQL was invalid.
    #[error("invalid global event position {value}")]
    InvalidGlobalPosition {
        /// Rejected global position value.
        value: i64,
    },
    /// Idempotency key exists for a different command result.
    #[error("dedupe conflict for tenant {tenant_id} and idempotency key {idempotency_key}")]
    DedupeConflict {
        /// Tenant that owns the idempotency key.
        tenant_id: String,
        /// Conflicting idempotency key.
        idempotency_key: String,
    },
    /// Stored command dedupe response could not be decoded.
    #[error("stored dedupe result could not be decoded")]
    DedupeResultDecode {
        /// JSON decode error.
        #[source]
        source: serde_json::Error,
    },
    /// SQLx returned a database error.
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_result_alias_accepts_store_error() {
        let result: StoreResult<()> = Err(StoreError::EmptyAppend);

        assert!(matches!(result, Err(StoreError::EmptyAppend)));
    }
}

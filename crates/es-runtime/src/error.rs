/// Result alias for runtime command execution.
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Errors returned by the local command runtime.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// Runtime ingress capacity is full.
    #[error("runtime is overloaded")]
    Overloaded,
    /// Runtime background workers are unavailable.
    #[error("runtime is unavailable")]
    Unavailable,
    /// A shard-local ring or mailbox is full.
    #[error("shard {shard_id} is overloaded")]
    #[allow(missing_docs)]
    ShardOverloaded { shard_id: usize },
    /// Runtime was configured with zero shards.
    #[error("shard count must be greater than zero")]
    InvalidShardCount,
    /// Runtime ingress capacity was invalid.
    #[error("ingress capacity must be greater than zero")]
    InvalidIngressCapacity,
    /// Runtime disruptor ring size was invalid.
    #[error("ring size must be greater than zero")]
    InvalidRingSize,
    /// Storage reported an optimistic-concurrency conflict.
    #[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
    #[allow(missing_docs)]
    Conflict {
        stream_id: String,
        expected: String,
        actual: Option<u64>,
    },
    /// Aggregate decision rejected the command.
    #[error("domain error: {message}")]
    Domain {
        /// Domain error message.
        message: String,
    },
    /// Event codec failed to encode or decode an event.
    #[error("codec error: {message}")]
    Codec {
        /// Codec error message.
        message: String,
    },
    /// Durable event store returned an infrastructure error.
    #[error("store error")]
    Store(#[from] es_store_postgres::StoreError),
}

impl RuntimeError {
    /// Converts store errors into runtime-visible errors, preserving conflicts as structured data.
    pub fn from_store_error(error: es_store_postgres::StoreError) -> Self {
        match error {
            es_store_postgres::StoreError::StreamConflict {
                stream_id,
                expected,
                actual,
            } => Self::Conflict {
                stream_id,
                expected,
                actual,
            },
            error => Self::Store(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use es_store_postgres::StoreError;

    use super::*;

    #[test]
    fn runtime_error_formats_typed_capacity_errors() {
        assert_eq!(
            "runtime is overloaded",
            RuntimeError::Overloaded.to_string()
        );
        assert_eq!(
            "runtime is unavailable",
            RuntimeError::Unavailable.to_string()
        );
        assert_eq!(
            "shard 2 is overloaded",
            RuntimeError::ShardOverloaded { shard_id: 2 }.to_string()
        );
    }

    #[test]
    fn runtime_error_maps_store_conflict_to_structured_conflict() {
        let error = RuntimeError::from_store_error(StoreError::StreamConflict {
            stream_id: "order-1".to_owned(),
            expected: "no stream".to_owned(),
            actual: Some(7),
        });

        assert!(matches!(
            error,
            RuntimeError::Conflict {
                stream_id,
                expected,
                actual: Some(7),
            } if stream_id == "order-1" && expected == "no stream"
        ));
    }
}

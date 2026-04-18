/// Result alias for projection and query-side catch-up operations.
pub type ProjectionResult<T> = Result<T, ProjectionError>;

/// Errors returned by projection contracts.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectionError {
    /// Projector name was invalid.
    #[error("projector name cannot be empty")]
    InvalidProjectorName,
    /// A global position was invalid.
    #[error("global position must be nonnegative: {value}")]
    InvalidGlobalPosition {
        /// Rejected global position value.
        value: i64,
    },
    /// A projection batch limit was invalid.
    #[error("batch limit must be between 1 and 1000: {value}")]
    InvalidBatchLimit {
        /// Rejected batch limit value.
        value: i64,
    },
    /// Projection had not caught up to the required global position before the wait deadline.
    #[error("projection lag: required {required}, actual {actual}")]
    ProjectionLag {
        /// Required global position.
        required: i64,
        /// Actual global position observed.
        actual: i64,
    },
    /// Event payload could not be decoded for a projector.
    #[error("failed to decode payload for {event_type} schema version {schema_version}")]
    PayloadDecode {
        /// Event type being decoded.
        event_type: String,
        /// Event schema version being decoded.
        schema_version: i32,
    },
    /// Projection storage returned an infrastructure error.
    #[error("store error: {message}")]
    Store {
        /// Storage error message.
        message: String,
    },
}

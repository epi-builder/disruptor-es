use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use es_runtime::RuntimeError;
use serde::Serialize;

/// JSON API error returned by HTTP command handlers.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Request payload or derived command data was invalid.
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Human-readable validation message.
        message: String,
    },
    /// Runtime or shard ingress was overloaded.
    #[error("{0}")]
    Overloaded(String),
    /// Runtime was unavailable or the command reply was dropped.
    #[error("{0}")]
    Unavailable(String),
    /// Runtime reported an optimistic-concurrency conflict.
    #[error("stream conflict for {stream_id}: expected {expected}, actual {actual:?}")]
    Conflict {
        /// Conflicting stream identifier.
        stream_id: String,
        /// Expected stream revision.
        expected: String,
        /// Actual stream revision, when known.
        actual: Option<u64>,
    },
    /// Domain command was rejected.
    #[error("domain error: {message}")]
    Domain {
        /// Domain error message.
        message: String,
    },
    /// Internal codec or storage failure.
    #[error("internal error: {message}")]
    Internal {
        /// Internal error message.
        message: String,
    },
    /// Command reply channel closed before the runtime completed the command.
    #[error("command reply dropped")]
    ReplyDropped,
}

/// Outer JSON body for API errors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiErrorBody {
    /// Error details.
    pub error: ApiErrorPayload,
}

/// Stable API error payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiErrorPayload {
    /// Stable machine-readable error code.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// Conflicting stream identifier for conflict responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
    /// Expected revision for conflict responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Actual revision for conflict responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<u64>,
}

impl ApiError {
    pub(crate) fn invalid_request(error: impl std::fmt::Display) -> Self {
        Self::InvalidRequest {
            message: error.to_string(),
        }
    }

    fn status_and_payload(&self) -> (StatusCode, ApiErrorPayload) {
        match self {
            Self::InvalidRequest { message } => (
                StatusCode::BAD_REQUEST,
                ApiErrorPayload::new("invalid_request", message.clone()),
            ),
            Self::Overloaded(message) => (
                StatusCode::TOO_MANY_REQUESTS,
                ApiErrorPayload::new("overloaded", message.clone()),
            ),
            Self::Unavailable(message) => (
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorPayload::new("unavailable", message.clone()),
            ),
            Self::Conflict {
                stream_id,
                expected,
                actual,
            } => (
                StatusCode::CONFLICT,
                ApiErrorPayload {
                    code: "conflict",
                    message: self.to_string(),
                    stream_id: Some(stream_id.clone()),
                    expected: Some(expected.clone()),
                    actual: *actual,
                },
            ),
            Self::Domain { message } => (
                StatusCode::BAD_REQUEST,
                ApiErrorPayload::new("domain", message.clone()),
            ),
            Self::Internal { message } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorPayload::new("internal", message.clone()),
            ),
            Self::ReplyDropped => (
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorPayload::new("unavailable", self.to_string()),
            ),
        }
    }
}

impl ApiErrorPayload {
    fn new(code: &'static str, message: String) -> Self {
        Self {
            code,
            message,
            stream_id: None,
            expected: None,
            actual: None,
        }
    }
}

impl From<RuntimeError> for ApiError {
    fn from(error: RuntimeError) -> Self {
        match error {
            RuntimeError::Overloaded | RuntimeError::ShardOverloaded { .. } => {
                Self::Overloaded(error.to_string())
            }
            RuntimeError::Unavailable => Self::Unavailable(error.to_string()),
            RuntimeError::Conflict {
                stream_id,
                expected,
                actual,
            } => Self::Conflict {
                stream_id,
                expected,
                actual,
            },
            RuntimeError::Domain { message } => Self::Domain { message },
            RuntimeError::Codec { message } => Self::InvalidRequest { message },
            RuntimeError::InvalidShardCount
            | RuntimeError::InvalidIngressCapacity
            | RuntimeError::InvalidRingSize
            | RuntimeError::Store(_) => Self::Internal {
                message: error.to_string(),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, payload) = self.status_and_payload();
        (status, Json(ApiErrorBody { error: payload })).into_response()
    }
}

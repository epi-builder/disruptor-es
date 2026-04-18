use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::{OutboxError, OutboxResult};

/// Message passed from the dispatcher to an external publisher.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PublishEnvelope {
    /// External topic.
    pub topic: String,
    /// External message key.
    pub message_key: String,
    /// Deterministic idempotency key.
    pub idempotency_key: String,
    /// External payload.
    pub payload: serde_json::Value,
    /// External metadata.
    pub metadata: serde_json::Value,
}

/// Storage-neutral external publisher boundary.
pub trait Publisher: Clone + Send + Sync + 'static {
    /// Publishes one envelope.
    fn publish(&self, envelope: PublishEnvelope) -> BoxFuture<'_, OutboxResult<()>>;
}

/// In-memory publisher for tests and contract verification.
#[derive(Clone, Debug, Default)]
pub struct InMemoryPublisher {
    inner: Arc<Mutex<InMemoryPublisherInner>>,
}

#[derive(Debug, Default)]
struct InMemoryPublisherInner {
    published: Vec<PublishEnvelope>,
    idempotency_keys: HashSet<String>,
    failures: VecDeque<String>,
}

impl InMemoryPublisher {
    /// Returns the recorded external effects.
    pub fn published(&self) -> Vec<PublishEnvelope> {
        self.inner
            .lock()
            .expect("in-memory publisher mutex poisoned")
            .published
            .clone()
    }

    /// Queues a publisher failure for the next publish attempt.
    pub fn push_failure(&self, message: impl Into<String>) {
        self.inner
            .lock()
            .expect("in-memory publisher mutex poisoned")
            .failures
            .push_back(message.into());
    }
}

impl Publisher for InMemoryPublisher {
    fn publish(&self, envelope: PublishEnvelope) -> BoxFuture<'_, OutboxResult<()>> {
        Box::pin(async move {
            let mut inner = self
                .inner
                .lock()
                .expect("in-memory publisher mutex poisoned");

            if let Some(message) = inner.failures.pop_front() {
                return Err(OutboxError::Publisher { message });
            }

            if inner
                .idempotency_keys
                .insert(envelope.idempotency_key.clone())
            {
                inner.published.push(envelope);
            }

            Ok(())
        })
    }
}

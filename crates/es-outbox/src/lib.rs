//! Outbox dispatch and process-manager integration boundary.

mod dispatcher;
mod error;
mod models;
mod publisher;

pub use dispatcher::{OutboxStore, dispatch_once};
pub use error::{OutboxError, OutboxResult};
pub use models::{
    DispatchBatchLimit, DispatchOutcome, MessageKey, NewOutboxMessage, OutboxMessage, OutboxStatus,
    PendingSourceEventRef, ProcessManagerName, RetryPolicy, RetryScheduleOutcome, SourceEventRef,
    Topic, WorkerId,
};
pub use publisher::{InMemoryPublisher, PublishEnvelope, Publisher};

/// Phase ownership marker for the outbox crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 6 owns outbox dispatch and process-manager integration contracts.";

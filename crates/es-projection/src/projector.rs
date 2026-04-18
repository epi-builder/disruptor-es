use std::{future::Future, pin::Pin};

use es_core::TenantId;
use serde::{Deserialize, Serialize};

use crate::{ProjectionResult, ProjectorName};

/// Storage-neutral event input for projection handlers.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectionEvent {
    /// Durable event-store global position.
    pub global_position: i64,
    /// Stored event type.
    pub event_type: String,
    /// Stored event schema version.
    pub schema_version: i32,
    /// Event payload.
    pub payload: serde_json::Value,
    /// Event metadata.
    pub metadata: serde_json::Value,
    /// Tenant that owns the event.
    pub tenant_id: TenantId,
}

/// Outcome of one projector catch-up attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatchUpOutcome {
    /// No committed events were available to apply.
    Idle,
    /// One or more committed events were applied.
    Applied {
        /// Number of events applied by the catch-up attempt.
        event_count: usize,
        /// Last durable global position applied.
        last_global_position: i64,
    },
}

/// Projection handler for storage-neutral committed events.
pub trait Projector: Send + Sync {
    /// Returns this projector's validated name.
    fn name(&self) -> &ProjectorName;

    /// Returns whether this projector handles an event type and schema version.
    fn handles(&self, event_type: &str, schema_version: i32) -> bool;

    /// Applies one projection event.
    fn apply<'a>(
        &'a self,
        event: &'a ProjectionEvent,
    ) -> Pin<Box<dyn Future<Output = ProjectionResult<()>> + Send + 'a>>;
}

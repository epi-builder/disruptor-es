use es_core::TenantId;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DispatchBatchLimit, OutboxResult, ProcessManagerName};

/// Committed event shape consumed by storage-neutral process managers.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProcessEvent {
    /// Durable global position assigned by the event store.
    pub global_position: i64,
    /// Unique source event identifier.
    pub event_id: Uuid,
    /// Stable source event type.
    pub event_type: String,
    /// Source event payload schema version.
    pub schema_version: i32,
    /// Source event JSON payload.
    pub payload: serde_json::Value,
    /// Source event JSON metadata.
    pub metadata: serde_json::Value,
    /// Tenant that owns the source event and follow-up commands.
    pub tenant_id: TenantId,
    /// Command that produced the source event.
    pub command_id: Uuid,
    /// Correlation identifier copied into follow-up commands.
    pub correlation_id: Uuid,
    /// Optional command or event that caused the source event.
    pub causation_id: Option<Uuid>,
}

/// Outcome of processing a committed event batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessOutcome {
    /// No committed events were available.
    Idle,
    /// Event was intentionally ignored.
    Skipped {
        /// Skipped event global position.
        global_position: i64,
    },
    /// Follow-up commands were submitted and replied.
    CommandsSubmitted {
        /// Processed source event global position.
        global_position: i64,
        /// Number of commands submitted for the source event.
        command_count: usize,
    },
}

/// Storage-neutral process-manager behavior.
pub trait ProcessManager: Send + Sync {
    /// Returns the durable process-manager identity.
    fn name(&self) -> &ProcessManagerName;

    /// Returns whether this process manager handles an event type/schema version pair.
    fn handles(&self, event_type: &str, schema_version: i32) -> bool;

    /// Processes one committed event.
    fn process<'a>(
        &'a self,
        event: &'a ProcessEvent,
    ) -> BoxFuture<'a, OutboxResult<ProcessOutcome>>;
}

/// Port for reading committed events by durable global position.
pub trait CommittedEventReader: Send + Sync {
    /// Reads committed events after a saved global position.
    fn read_global(
        &self,
        tenant_id: TenantId,
        after_global_position: i64,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<ProcessEvent>>>;
}

/// Port for tenant-scoped durable process-manager offsets.
pub trait ProcessManagerOffsetStore: Send + Sync {
    /// Loads the last completed global position.
    fn process_manager_offset(
        &self,
        tenant_id: TenantId,
        name: ProcessManagerName,
    ) -> BoxFuture<'_, OutboxResult<Option<i64>>>;

    /// Advances the last completed global position.
    fn advance_process_manager_offset(
        &self,
        tenant_id: TenantId,
        name: ProcessManagerName,
        last_global_position: i64,
    ) -> BoxFuture<'_, OutboxResult<()>>;
}

/// Processes caller-supplied committed events after loading the durable offset.
pub async fn process_batch<M, O>(
    manager: &M,
    offset_store: &O,
    tenant_id: TenantId,
    events: Vec<ProcessEvent>,
) -> OutboxResult<ProcessOutcome>
where
    M: ProcessManager,
    O: ProcessManagerOffsetStore,
{
    let current_offset = offset_store
        .process_manager_offset(tenant_id.clone(), manager.name().clone())
        .await?
        .unwrap_or(0);
    let mut outcome = ProcessOutcome::Idle;

    for event in events
        .into_iter()
        .filter(|event| event.global_position > current_offset)
    {
        outcome = if manager.handles(&event.event_type, event.schema_version) {
            manager.process(&event).await?
        } else {
            ProcessOutcome::Skipped {
                global_position: event.global_position,
            }
        };
        offset_store
            .advance_process_manager_offset(
                tenant_id.clone(),
                manager.name().clone(),
                event.global_position,
            )
            .await?;
    }

    Ok(outcome)
}

/// Reads committed events from storage and processes one batch.
pub async fn process_committed_batch<M, R, O>(
    manager: &M,
    reader: &R,
    offset_store: &O,
    tenant_id: TenantId,
    limit: DispatchBatchLimit,
) -> OutboxResult<ProcessOutcome>
where
    M: ProcessManager,
    R: CommittedEventReader,
    O: ProcessManagerOffsetStore,
{
    let offset = offset_store
        .process_manager_offset(tenant_id.clone(), manager.name().clone())
        .await?
        .unwrap_or(0);
    let events = reader.read_global(tenant_id.clone(), offset, limit).await?;
    if events.is_empty() {
        return Ok(ProcessOutcome::Idle);
    }

    process_batch(manager, offset_store, tenant_id, events).await
}

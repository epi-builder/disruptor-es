//! Process-manager contract tests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use es_core::TenantId;
use es_outbox::{
    CommittedEventReader, DispatchBatchLimit, OutboxResult, ProcessEvent, ProcessManager,
    ProcessManagerName, ProcessManagerOffsetStore, ProcessOutcome, process_batch,
    process_committed_batch,
};
use futures::future::BoxFuture;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecordingOffsets {
    current: Arc<Mutex<Option<i64>>>,
    advances: Arc<Mutex<Vec<i64>>>,
}

impl ProcessManagerOffsetStore for RecordingOffsets {
    fn process_manager_offset(
        &self,
        _tenant_id: TenantId,
        _name: ProcessManagerName,
    ) -> BoxFuture<'_, OutboxResult<Option<i64>>> {
        let current = *self.current.lock().expect("current offset");
        Box::pin(async move { Ok(current) })
    }

    fn advance_process_manager_offset(
        &self,
        _tenant_id: TenantId,
        _name: ProcessManagerName,
        last_global_position: i64,
    ) -> BoxFuture<'_, OutboxResult<()>> {
        self.advances
            .lock()
            .expect("offset advances")
            .push(last_global_position);
        Box::pin(async move { Ok(()) })
    }
}

struct RecordingManager {
    name: ProcessManagerName,
    processed: Arc<Mutex<Vec<i64>>>,
}

impl RecordingManager {
    fn new() -> Self {
        Self {
            name: ProcessManagerName::new("recording-manager").expect("process manager name"),
            processed: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ProcessManager for RecordingManager {
    fn name(&self) -> &ProcessManagerName {
        &self.name
    }

    fn handles(&self, event_type: &str, schema_version: i32) -> bool {
        event_type == "OrderPlaced" && schema_version == 1
    }

    fn process<'a>(
        &'a self,
        event: &'a ProcessEvent,
    ) -> BoxFuture<'a, OutboxResult<ProcessOutcome>> {
        self.processed
            .lock()
            .expect("processed events")
            .push(event.global_position);
        Box::pin(async move {
            Ok(ProcessOutcome::CommandsSubmitted {
                global_position: event.global_position,
                command_count: 1,
            })
        })
    }
}

#[derive(Clone)]
struct RecordingReader {
    events: Arc<Mutex<VecDeque<ProcessEvent>>>,
    calls: Arc<Mutex<Vec<(TenantId, i64, i64)>>>,
}

impl RecordingReader {
    fn new(events: Vec<ProcessEvent>) -> Self {
        Self {
            events: Arc::new(Mutex::new(events.into())),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CommittedEventReader for RecordingReader {
    fn read_global(
        &self,
        tenant_id: TenantId,
        after_global_position: i64,
        limit: DispatchBatchLimit,
    ) -> BoxFuture<'_, OutboxResult<Vec<ProcessEvent>>> {
        self.calls.lock().expect("reader calls").push((
            tenant_id,
            after_global_position,
            limit.value(),
        ));
        let mut events = self.events.lock().expect("events");
        let batch = events.drain(..).collect::<Vec<_>>();
        Box::pin(async move { Ok(batch) })
    }
}

fn tenant() -> TenantId {
    TenantId::new("tenant-a").expect("tenant id")
}

fn process_event(global_position: i64, event_type: &str) -> ProcessEvent {
    ProcessEvent {
        global_position,
        event_id: Uuid::from_u128(global_position as u128),
        event_type: event_type.to_owned(),
        schema_version: 1,
        payload: json!({ "global_position": global_position }),
        metadata: json!({ "source": "process-manager-test" }),
        tenant_id: tenant(),
        command_id: Uuid::from_u128(100 + global_position as u128),
        correlation_id: Uuid::from_u128(200 + global_position as u128),
        causation_id: Some(Uuid::from_u128(300 + global_position as u128)),
    }
}

#[tokio::test]
async fn process_manager_process_batch_skips_events_at_or_below_saved_offset() -> OutboxResult<()> {
    let manager = RecordingManager::new();
    let offsets = RecordingOffsets::default();
    *offsets.current.lock().expect("current offset") = Some(10);

    let outcome = process_batch(
        &manager,
        &offsets,
        tenant(),
        vec![
            process_event(9, "OrderPlaced"),
            process_event(11, "OrderPlaced"),
        ],
    )
    .await?;

    assert_eq!(
        vec![11],
        *manager.processed.lock().expect("processed events")
    );
    assert_eq!(vec![11], *offsets.advances.lock().expect("advances"));
    assert_eq!(
        ProcessOutcome::CommandsSubmitted {
            global_position: 11,
            command_count: 1
        },
        outcome
    );

    Ok(())
}

#[tokio::test]
async fn process_manager_process_batch_advances_skipped_events_after_manager_returns()
-> OutboxResult<()> {
    let manager = RecordingManager::new();
    let offsets = RecordingOffsets::default();

    let outcome = process_batch(
        &manager,
        &offsets,
        tenant(),
        vec![process_event(12, "InventoryReserved")],
    )
    .await?;

    assert_eq!(
        Vec::<i64>::new(),
        *manager.processed.lock().expect("processed")
    );
    assert_eq!(vec![12], *offsets.advances.lock().expect("advances"));
    assert_eq!(
        ProcessOutcome::Skipped {
            global_position: 12
        },
        outcome
    );

    Ok(())
}

#[tokio::test]
async fn process_manager_process_committed_batch_reads_from_saved_offset() -> OutboxResult<()> {
    let manager = RecordingManager::new();
    let offsets = RecordingOffsets::default();
    *offsets.current.lock().expect("current offset") = Some(20);
    let reader = RecordingReader::new(vec![process_event(21, "OrderPlaced")]);
    let limit = DispatchBatchLimit::new(25)?;

    process_committed_batch(&manager, &reader, &offsets, tenant(), limit).await?;

    assert_eq!(
        vec![(tenant(), 20, 25)],
        *reader.calls.lock().expect("reader calls")
    );
    assert_eq!(vec![21], *offsets.advances.lock().expect("advances"));

    Ok(())
}

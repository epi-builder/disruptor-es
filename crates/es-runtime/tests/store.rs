mod common;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_runtime::{PostgresRuntimeEventStore, RuntimeEventStore};
use es_store_postgres::{AppendOutcome, AppendRequest, CommittedAppend, NewEvent};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

fn metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: TenantId::new("tenant-a").expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn append_request() -> AppendRequest {
    AppendRequest::new(
        StreamId::new("order-1").expect("stream id"),
        ExpectedRevision::NoStream,
        metadata(),
        "idem-1",
        vec![
            NewEvent::new(
                Uuid::from_u128(10),
                "OrderPlaced",
                1,
                json!({ "order_id": "order-1" }),
                json!({ "source": "runtime-test" }),
            )
            .expect("event"),
        ],
    )
    .expect("append request")
}

fn committed_append() -> CommittedAppend {
    CommittedAppend {
        stream_id: StreamId::new("order-1").expect("stream id"),
        first_revision: StreamRevision::new(1),
        last_revision: StreamRevision::new(1),
        global_positions: vec![42],
        event_ids: vec![Uuid::from_u128(10)],
    }
}

#[tokio::test]
async fn store_fake_records_append_requests_and_returns_committed() {
    let store =
        common::FakeRuntimeEventStore::with_outcome(AppendOutcome::Committed(committed_append()));
    let request = append_request();

    let outcome = store.append(request.clone()).await.expect("append");

    assert!(matches!(outcome, AppendOutcome::Committed(_)));
    assert_eq!(1, store.appended_len());
    assert_eq!(vec![request], store.append_requests());
}

#[test]
fn store_postgres_adapter_implements_runtime_event_store() {
    fn assert_runtime_store<S: RuntimeEventStore>() {}

    assert_runtime_store::<PostgresRuntimeEventStore>();
}

#[test]
fn store_validation_metadata_records_wave_0_contract_files() {
    let root = env!("CARGO_MANIFEST_DIR");
    let validation = std::fs::read_to_string(format!(
        "{root}/../../.planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md"
    ))
    .expect("validation file");

    assert!(validation.contains("crates/es-runtime/src/error.rs"));
    assert!(validation.contains("crates/es-runtime/src/command.rs"));
    assert!(validation.contains("crates/es-runtime/src/store.rs"));
}

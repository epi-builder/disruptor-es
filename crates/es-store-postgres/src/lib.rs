//! PostgreSQL-backed durable event-store boundary.
//!
//! This crate owns durable PostgreSQL storage for event appends, stream/global
//! reads, command deduplication results, snapshots, and rehydration DTOs. It is
//! not a runtime, adapter, projection worker, outbox dispatcher, broker client,
//! or disruptor execution crate.

mod error;
mod event_store;
/// Identifier generation helpers.
pub mod ids;
mod models;
mod projection;
mod rehydrate;
mod sql;

pub use error::{StoreError, StoreResult};
pub use event_store::PostgresEventStore;
pub use ids::{IdGenerator, UuidV7Generator};
pub use models::{
    AppendOutcome, AppendRequest, CommittedAppend, MAX_JSON_PAYLOAD_BYTES, NewEvent,
    RehydrationBatch, SaveSnapshotRequest, SnapshotRecord, StoredEvent,
};
pub use projection::{OrderSummaryReadModel, PostgresProjectionStore, ProductInventoryReadModel};

//! Local command routing, shard ownership, and in-process execution boundary.

mod cache;
mod command;
mod disruptor_path;
mod engine;
mod error;
mod gateway;
mod router;
mod shard;
mod store;

pub use cache::{AggregateCache, DedupeCache, DedupeKey, DedupeRecord};
pub use command::{CommandEnvelope, CommandOutcome, CommandReply, RuntimeEventCodec};
pub use disruptor_path::{DisruptorPath, ReleasedHandoff};
pub use engine::{CommandEngine, CommandEngineConfig};
pub use error::{RuntimeError, RuntimeResult};
pub use es_kernel::Aggregate;
pub use es_store_postgres::CommittedAppend;
pub use gateway::{CommandGateway, RoutedCommand};
pub use router::{PartitionRouter, ROUTING_HASH_SEED, ShardId};
pub use shard::{LocalHandoffId, ShardHandle, ShardHandoff, ShardHandoffToken, ShardState};
pub use store::{PostgresRuntimeEventStore, RuntimeEventStore};

/// Phase ownership marker for the runtime crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 3 owns local command routing, shard ownership, and in-process execution.";

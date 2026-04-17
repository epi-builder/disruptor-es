//! Local command routing, shard ownership, and in-process execution boundary.

mod command;
mod error;
mod gateway;
mod router;
mod store;

pub use command::{CommandEnvelope, CommandOutcome, CommandReply, RuntimeEventCodec};
pub use error::{RuntimeError, RuntimeResult};
pub use gateway::{CommandGateway, RoutedCommand};
pub use router::{PartitionRouter, ROUTING_HASH_SEED, ShardId};
pub use store::{PostgresRuntimeEventStore, RuntimeEventStore};

/// Phase ownership marker for the runtime crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 3 owns local command routing, shard ownership, and in-process execution.";

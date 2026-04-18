//! Projector and read-model catch-up boundary.

mod checkpoint;
mod error;
mod projector;
mod query;

pub use checkpoint::{
    MinimumGlobalPosition, ProjectionBatchLimit, ProjectorName, ProjectorOffset,
};
pub use error::{ProjectionError, ProjectionResult};
pub use projector::{CatchUpOutcome, ProjectionEvent, Projector};
pub use query::{FreshnessCheck, WaitPolicy, wait_for_minimum_position};

/// Phase ownership marker for the projection crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 5 owns query-side projection catch-up contracts and must not gate command success.";

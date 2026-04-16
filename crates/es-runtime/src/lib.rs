//! Local command routing, shard ownership, and in-process execution boundary.

/// Phase ownership marker for the runtime crate.
pub const PHASE_BOUNDARY: &str =
    "Phase 3 owns local command routing, shard ownership, and in-process execution.";

//! Durable event append and event-store transaction boundary.

/// Phase ownership marker for the durable event-store crate.
pub const PHASE_BOUNDARY: &str = "Phase 2 owns durable event append and transaction contracts.";

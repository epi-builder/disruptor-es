//! Request decoding boundary for the future HTTP adapter.

/// Phase ownership marker for the HTTP adapter crate.
pub const PHASE_BOUNDARY: &str =
    "Future phases decode HTTP requests here without owning aggregate state.";

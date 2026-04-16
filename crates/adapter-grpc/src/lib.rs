//! Request decoding boundary for the future gRPC adapter.

/// Phase ownership marker for the gRPC adapter crate.
pub const PHASE_BOUNDARY: &str =
    "Future phases decode gRPC requests here without owning aggregate state.";

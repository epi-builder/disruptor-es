//! Application composition library.

/// Commerce process-manager workflow composition.
pub mod commerce_process_manager;
/// External-process HTTP stress runner and canonical request fixtures.
pub mod http_stress;
/// Application-level tracing and metrics bootstrap.
pub mod observability;
/// Official HTTP service composition/bootstrap.
pub mod serve;
/// Single-service integrated stress runner.
pub mod stress;

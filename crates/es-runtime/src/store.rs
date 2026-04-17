/// Placeholder for the runtime event-store adapter implemented by Plan 03-01 Task 3.
#[derive(Clone, Debug)]
pub struct PostgresRuntimeEventStore;

/// Placeholder for the runtime event-store trait implemented by Plan 03-01 Task 3.
pub trait RuntimeEventStore: Clone + Send + Sync + 'static {}

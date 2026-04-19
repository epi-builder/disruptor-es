use std::collections::HashMap;

use es_core::{StreamId, TenantId};
use es_kernel::Aggregate;

/// Shard-local aggregate state cache owned by a single shard runtime.
pub struct AggregateCache<A: Aggregate> {
    states: HashMap<AggregateCacheKey, A::State>,
}

impl<A: Aggregate> Default for AggregateCache<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Aggregate> AggregateCache<A> {
    /// Creates an empty shard-local aggregate cache.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Returns cached state, inserting a default aggregate state when the key is absent.
    pub fn get_or_default(&mut self, key: &AggregateCacheKey) -> A::State {
        self.states.entry(key.clone()).or_default().clone()
    }

    /// Replaces the cached state after the caller has committed the staged state.
    pub fn commit_state(&mut self, key: AggregateCacheKey, state: A::State) {
        self.states.insert(key, state);
    }

    /// Returns cached state without creating a default entry.
    pub fn get(&self, key: &AggregateCacheKey) -> Option<&A::State> {
        self.states.get(key)
    }

    /// Returns the number of cached stream states.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Returns true when the cache has no stream states.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

/// Tenant-scoped aggregate cache key for shard-local hot state.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AggregateCacheKey {
    /// Tenant that owns the stream state.
    pub tenant_id: TenantId,
    /// Stream whose aggregate state is cached.
    pub stream_id: StreamId,
}

/// Tenant-scoped dedupe cache key for a shard-local optimization.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DedupeKey {
    /// Tenant that owns the idempotency key.
    pub tenant_id: TenantId,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
}

/// Cached replay record returned for a duplicate command.
#[derive(Clone, Debug, PartialEq)]
pub struct DedupeRecord {
    /// Durable append and typed reply originally returned by PostgreSQL.
    pub replay: es_store_postgres::CommandReplayRecord,
}

/// Shard-local dedupe cache. PostgreSQL remains authoritative for command dedupe.
#[derive(Default)]
pub struct DedupeCache {
    records: HashMap<DedupeKey, DedupeRecord>,
}

impl DedupeCache {
    /// Creates an empty shard-local dedupe cache.
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    /// Returns a cached dedupe record for a tenant-scoped idempotency key.
    pub fn get(&self, key: &DedupeKey) -> Option<&DedupeRecord> {
        self.records.get(key)
    }

    /// Records a committed append summary in the shard-local dedupe cache.
    pub fn record(&mut self, key: DedupeKey, record: DedupeRecord) {
        self.records.insert(key, record);
    }

    /// Returns the number of cached dedupe records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true when the cache has no dedupe records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

use std::hash::Hasher;

use crate::{RuntimeError, RuntimeResult};

/// Fixed seed for stable tenant-aware routing.
pub const ROUTING_HASH_SEED: u64 = 0x4553_5255_4e54494d;

/// Local shard identifier selected by partition routing.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShardId(usize);

impl ShardId {
    /// Creates a shard identifier from its numeric value.
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the numeric shard identifier.
    pub const fn value(self) -> usize {
        self.0
    }
}

/// Stable router from tenant-scoped partition keys to local shard ownership.
#[derive(Clone, Debug)]
pub struct PartitionRouter {
    shard_count: usize,
}

impl PartitionRouter {
    /// Creates a router for a fixed local shard count.
    pub fn new(shard_count: usize) -> RuntimeResult<Self> {
        if shard_count == 0 {
            return Err(RuntimeError::InvalidShardCount);
        }

        Ok(Self { shard_count })
    }

    /// Returns the configured local shard count.
    pub const fn shard_count(&self) -> usize {
        self.shard_count
    }

    /// Routes a tenant-scoped partition key to a local shard.
    pub fn route(
        &self,
        tenant_id: &es_core::TenantId,
        partition_key: &es_core::PartitionKey,
    ) -> ShardId {
        let mut hasher = twox_hash::XxHash64::with_seed(ROUTING_HASH_SEED);
        hasher.write(tenant_id.as_str().as_bytes());
        hasher.write_u8(0);
        hasher.write(partition_key.as_str().as_bytes());

        ShardId::new((hasher.finish() as usize) % self.shard_count)
    }
}

use es_kernel::Aggregate;
use tracing::info_span;

use crate::{
    CommandGateway, PartitionRouter, RoutedCommand, RuntimeError, RuntimeEventCodec,
    RuntimeEventStore, RuntimeResult, ShardHandle, ShardId,
};

/// Configuration for the local command engine.
#[derive(Clone, Debug)]
pub struct CommandEngineConfig {
    /// Number of local shard owners.
    pub shard_count: usize,
    /// Bounded adapter-facing ingress capacity.
    pub ingress_capacity: usize,
    /// Per-shard disruptor ring size.
    pub ring_size: usize,
}

impl CommandEngineConfig {
    /// Creates validated command engine configuration.
    pub fn new(
        shard_count: usize,
        ingress_capacity: usize,
        ring_size: usize,
    ) -> RuntimeResult<Self> {
        if shard_count == 0 {
            return Err(RuntimeError::InvalidShardCount);
        }
        if ingress_capacity == 0 {
            return Err(RuntimeError::InvalidIngressCapacity);
        }
        if ring_size == 0 {
            return Err(RuntimeError::InvalidRingSize);
        }

        Ok(Self {
            shard_count,
            ingress_capacity,
            ring_size,
        })
    }
}

/// Production local command engine connecting ingress, shards, storage, codec, and replies.
pub struct CommandEngine<A, S, C>
where
    A: Aggregate,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A>,
{
    gateway: CommandGateway<A>,
    receiver: tokio::sync::mpsc::Receiver<RoutedCommand<A>>,
    shards: Vec<ShardHandle<A>>,
    store: S,
    codec: C,
}

impl<A, S, C> CommandEngine<A, S, C>
where
    A: Aggregate,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A>,
{
    /// Creates a command engine with bounded ingress and one shard handle per shard.
    pub fn new(config: CommandEngineConfig, store: S, codec: C) -> RuntimeResult<Self> {
        let router = PartitionRouter::new(config.shard_count)?;
        let (gateway, receiver) = CommandGateway::new(router, config.ingress_capacity)?;
        let mut shards = Vec::with_capacity(config.shard_count);

        for index in 0..config.shard_count {
            shards.push(ShardHandle::new(ShardId::new(index), config.ring_size)?);
        }

        Ok(Self {
            gateway,
            receiver,
            shards,
            store,
            codec,
        })
    }

    /// Returns a cloneable adapter-facing command gateway.
    pub fn gateway(&self) -> CommandGateway<A> {
        self.gateway.clone()
    }

    /// Processes one accepted command through its owning shard, if one is available.
    pub async fn process_one(&mut self) -> RuntimeResult<bool>
    where
        A::Error: std::fmt::Display,
    {
        let Some(routed) = self.receiver.recv().await else {
            return Ok(false);
        };

        let shard_index = routed.shard_id.value();
        let span = info_span!(
            "command_engine.process_one",
            command_id = %routed.envelope.metadata.command_id,
            correlation_id = %routed.envelope.metadata.correlation_id,
            causation_id = ?routed.envelope.metadata.causation_id,
            tenant_id = %routed.envelope.metadata.tenant_id.as_str(),
            stream_id = %routed.envelope.stream_id.as_str(),
            shard_id = shard_index,
        );
        let _entered = span.enter();
        let Some(shard) = self.shards.get_mut(shard_index) else {
            let _ = routed.envelope.reply.send(Err(RuntimeError::Unavailable));
            return Ok(true);
        };

        shard.accept_routed(routed)?;
        shard.drain_released_handoffs()?;

        while shard
            .state_mut()
            .process_next_handoff(&self.store, &self.codec)
            .await?
        {}

        Ok(true)
    }
}

use es_kernel::Aggregate;
use tokio::sync::mpsc;

use crate::{CommandEnvelope, PartitionRouter, RuntimeError, RuntimeResult, ShardId};

/// Command envelope after deterministic local shard routing.
pub struct RoutedCommand<A: Aggregate> {
    /// Local shard selected by the partition router.
    pub shard_id: ShardId,
    /// Original command envelope accepted from the adapter boundary.
    pub envelope: CommandEnvelope<A>,
}

/// Bounded adapter-facing command ingress.
pub struct CommandGateway<A: Aggregate> {
    router: PartitionRouter,
    sender: mpsc::Sender<RoutedCommand<A>>,
}

impl<A: Aggregate> Clone for CommandGateway<A> {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl<A: Aggregate> CommandGateway<A> {
    /// Creates a bounded gateway and returns its receiving side for the runtime engine.
    pub fn new(
        router: PartitionRouter,
        ingress_capacity: usize,
    ) -> RuntimeResult<(Self, mpsc::Receiver<RoutedCommand<A>>)> {
        if ingress_capacity == 0 {
            return Err(RuntimeError::InvalidIngressCapacity);
        }

        let (sender, receiver) = tokio::sync::mpsc::channel(ingress_capacity);
        Ok((Self { router, sender }, receiver))
    }

    /// Attempts to submit a command without waiting for ingress capacity.
    pub fn try_submit(&self, envelope: CommandEnvelope<A>) -> RuntimeResult<()> {
        let shard_id = self
            .router
            .route(&envelope.metadata.tenant_id, &envelope.partition_key);
        let routed = RoutedCommand { shard_id, envelope };

        self.sender.try_send(routed).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => RuntimeError::Overloaded,
            mpsc::error::TrySendError::Closed(_) => RuntimeError::Unavailable,
        })
    }
}

use es_kernel::Aggregate;
use metrics::{counter, gauge};
use tokio::sync::mpsc;
use tracing::debug_span;

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
        let span = debug_span!(
            "command_gateway.try_submit",
            command_id = %envelope.metadata.command_id,
            correlation_id = %envelope.metadata.correlation_id,
            causation_id = ?envelope.metadata.causation_id,
            tenant_id = %envelope.metadata.tenant_id.as_str(),
            stream_id = %envelope.stream_id.as_str(),
        );
        let _entered = span.enter();
        let shard_id = self
            .router
            .route(&envelope.metadata.tenant_id, &envelope.partition_key);
        let routed = RoutedCommand { shard_id, envelope };
        let aggregate = aggregate_label::<A>();
        let depth = self.sender.max_capacity() - self.sender.capacity();
        gauge!("es_ingress_depth", "aggregate" => aggregate).set(depth as f64);

        match self.sender.try_send(routed) {
            Ok(()) => {
                let depth = self.sender.max_capacity() - self.sender.capacity();
                gauge!("es_ingress_depth", "aggregate" => aggregate).set(depth as f64);
                counter!(
                    "es_command_total",
                    "aggregate" => aggregate,
                    "outcome" => "accepted",
                )
                .increment(1);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                counter!(
                    "es_command_total",
                    "aggregate" => aggregate,
                    "outcome" => "rejected",
                )
                .increment(1);
                counter!("es_command_rejected_total", "reason" => "overloaded").increment(1);
                Err(RuntimeError::Overloaded)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                counter!(
                    "es_command_total",
                    "aggregate" => aggregate,
                    "outcome" => "rejected",
                )
                .increment(1);
                counter!("es_command_rejected_total", "reason" => "unavailable").increment(1);
                Err(RuntimeError::Unavailable)
            }
        }
    }
}

fn aggregate_label<A>() -> &'static str {
    let type_name = std::any::type_name::<A>();
    if type_name.ends_with("::Order") {
        "order"
    } else if type_name.ends_with("::Product") {
        "product"
    } else if type_name.ends_with("::User") {
        "user"
    } else {
        "aggregate"
    }
}

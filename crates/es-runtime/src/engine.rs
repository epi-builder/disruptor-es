use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use es_kernel::Aggregate;
use tokio::{sync::Notify, task::JoinHandle};
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
{
    gateway: CommandGateway<A>,
    receiver: tokio::sync::mpsc::Receiver<RoutedCommand<A>>,
    shard_senders: Vec<tokio::sync::mpsc::Sender<RoutedCommand<A>>>,
    shard_depths: Arc<Vec<AtomicUsize>>,
    worker_tasks: Vec<JoinHandle<RuntimeResult<()>>>,
    _store: Arc<S>,
    _codec: Arc<C>,
}

impl<A, S, C> CommandEngine<A, S, C>
where
    A: Aggregate + Send + 'static,
    A::Command: Send + 'static,
    A::Event: Send + 'static,
    A::Reply: Send + 'static,
    A::State: Send + 'static,
    A::Error: std::fmt::Display,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A> + Send + Sync + 'static,
{
    /// Creates a command engine with bounded ingress and one shard handle per shard.
    pub fn new(config: CommandEngineConfig, store: S, codec: C) -> RuntimeResult<Self> {
        let router = PartitionRouter::new(config.shard_count)?;
        let (gateway, receiver) = CommandGateway::new(router, config.ingress_capacity)?;
        let store = Arc::new(store);
        let codec = Arc::new(codec);
        let shard_depths = Arc::new(
            (0..config.shard_count)
                .map(|_| AtomicUsize::new(0))
                .collect::<Vec<_>>(),
        );
        let mut shard_senders = Vec::with_capacity(config.shard_count);
        let mut worker_tasks = Vec::with_capacity(config.shard_count);

        for index in 0..config.shard_count {
            let shard_id = ShardId::new(index);
            let shard = ShardHandle::new(shard_id, config.ring_size)?;
            let (sender, shard_receiver) = tokio::sync::mpsc::channel(config.ingress_capacity);
            shard_senders.push(sender);
            worker_tasks.push(tokio::spawn(run_shard_worker(
                shard,
                shard_receiver,
                store.clone(),
                codec.clone(),
                shard_depths.clone(),
            )));
        }

        Ok(Self {
            gateway,
            receiver,
            shard_senders,
            shard_depths,
            worker_tasks,
            _store: store,
            _codec: codec,
        })
    }

    /// Returns a cloneable adapter-facing command gateway.
    pub fn gateway(&self) -> CommandGateway<A> {
        self.gateway.clone()
    }

    /// Returns accepted and processable command depth for each shard.
    pub fn shard_depths(&self) -> Vec<usize> {
        self.shard_depths
            .iter()
            .map(|depth| depth.load(Ordering::Relaxed))
            .collect()
    }

    /// Dispatches one accepted command onto its owning shard worker, if one is available.
    pub async fn process_one(&mut self) -> RuntimeResult<bool>
    {
        let Some(routed) = self.receiver.recv().await else {
            return Ok(false);
        };

        self.dispatch_routed(routed).await?;
        Ok(true)
    }

    /// Runs the dispatcher loop until shutdown is requested or ingress closes.
    pub async fn run(mut self, shutdown: Arc<Notify>) -> RuntimeResult<()>
    {
        let mut shutting_down = false;

        loop {
            if shutting_down {
                self.receiver.close();
                self.drain_undispatched();
                break;
            }

            tokio::select! {
                biased;
                _ = shutdown.notified(), if !shutting_down => {
                    shutting_down = true;
                }
                routed = self.receiver.recv() => {
                    let Some(routed) = routed else {
                        break;
                    };
                    self.dispatch_routed(routed).await?;
                }
            }
        }

        self.shard_senders.clear();
        for task in self.worker_tasks {
            match task.await {
                Ok(result) => result?,
                Err(_) => return Err(RuntimeError::Unavailable),
            }
        }

        Ok(())
    }

    fn drain_undispatched(&mut self) {
        while let Ok(routed) = self.receiver.try_recv() {
            let _ = routed.envelope.reply.send(Err(RuntimeError::Unavailable));
        }
    }

    async fn dispatch_routed(&self, routed: RoutedCommand<A>) -> RuntimeResult<()>
    {
        let shard_index = routed.shard_id.value();
        let span = info_span!(
            "command_engine.dispatch",
            command_id = %routed.envelope.metadata.command_id,
            correlation_id = %routed.envelope.metadata.correlation_id,
            causation_id = ?routed.envelope.metadata.causation_id,
            tenant_id = %routed.envelope.metadata.tenant_id.as_str(),
            stream_id = %routed.envelope.stream_id.as_str(),
            shard_id = shard_index,
        );
        let _entered = span.enter();
        let Some(sender) = self.shard_senders.get(shard_index) else {
            let _ = routed.envelope.reply.send(Err(RuntimeError::Unavailable));
            return Ok(());
        };
        self.shard_depths[shard_index].fetch_add(1, Ordering::Relaxed);

        if sender.send(routed).await.is_err() {
            self.shard_depths[shard_index].fetch_sub(1, Ordering::Relaxed);
            return Err(RuntimeError::Unavailable);
        }

        Ok(())
    }
}

async fn run_shard_worker<A, S, C>(
    mut shard: ShardHandle<A>,
    mut receiver: tokio::sync::mpsc::Receiver<RoutedCommand<A>>,
    store: Arc<S>,
    codec: Arc<C>,
    shard_depths: Arc<Vec<AtomicUsize>>,
) -> RuntimeResult<()>
where
    A: Aggregate + Send + 'static,
    A::Command: Send + 'static,
    A::Event: Send + 'static,
    A::Reply: Send + 'static,
    A::State: Send + 'static,
    A::Error: std::fmt::Display,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A> + Send + Sync + 'static,
{
    let shard_index = shard.state().shard_id().value();

    while let Some(routed) = receiver.recv().await {
        let process_result: RuntimeResult<()> = async {
            shard.accept_routed(routed)?;
            shard.drain_released_handoffs()?;

            while shard
                .state_mut()
                .process_next_handoff(store.as_ref(), codec.as_ref())
                .await?
            {}

            Ok(())
        }
        .await;
        shard_depths[shard_index].fetch_sub(1, Ordering::Relaxed);
        process_result?;
    }

    Ok(())
}

use std::collections::{HashMap, VecDeque};

use es_kernel::Aggregate;

use crate::{
    AggregateCache, CommandEnvelope, CommandOutcome, DedupeCache, DedupeKey, DedupeRecord,
    DisruptorPath, RoutedCommand, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
    RuntimeResult, ShardId,
};

/// Command envelope released from the disruptor path for shard processing.
pub struct ShardHandoff<A: Aggregate> {
    /// Local disruptor sequence used only for ordered processing diagnostics.
    pub sequence: u64,
    /// Command envelope ready for the async processing stage.
    pub envelope: CommandEnvelope<A>,
}

/// Shard-local unique handoff identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LocalHandoffId(u64);

impl LocalHandoffId {
    /// Creates a local handoff identifier.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric local handoff identifier.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Token published through the disruptor path before an envelope becomes processable.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShardHandoffToken {
    /// Tenant that owns the command.
    pub tenant_id: es_core::TenantId,
    /// Stream affected by the command.
    pub stream_id: es_core::StreamId,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
    /// Fresh shard-local identity for duplicate in-flight submissions.
    pub local_handoff_id: LocalHandoffId,
}

impl ShardHandoffToken {
    fn placeholder() -> Self {
        Self {
            tenant_id: es_core::TenantId::new("__runtime_placeholder_tenant")
                .expect("placeholder tenant"),
            stream_id: es_core::StreamId::new("__runtime_placeholder_stream")
                .expect("placeholder stream"),
            idempotency_key: "__runtime_placeholder_idempotency".to_owned(),
            local_handoff_id: LocalHandoffId::new(0),
        }
    }
}

/// Shard-owned state and processable handoff queue.
pub struct ShardState<A: Aggregate> {
    shard_id: ShardId,
    cache: AggregateCache<A>,
    dedupe: DedupeCache,
    handoffs: VecDeque<ShardHandoff<A>>,
}

impl<A: Aggregate> ShardState<A> {
    /// Creates empty state owned by one local shard.
    pub fn new(shard_id: ShardId) -> Self {
        Self {
            shard_id,
            cache: AggregateCache::new(),
            dedupe: DedupeCache::new(),
            handoffs: VecDeque::new(),
        }
    }

    /// Returns this state owner's shard id.
    pub const fn shard_id(&self) -> ShardId {
        self.shard_id
    }

    /// Returns the shard-local aggregate cache.
    pub fn cache(&self) -> &AggregateCache<A> {
        &self.cache
    }

    /// Returns the mutable shard-local aggregate cache.
    pub fn cache_mut(&mut self) -> &mut AggregateCache<A> {
        &mut self.cache
    }

    /// Returns the shard-local dedupe cache.
    pub fn dedupe(&self) -> &DedupeCache {
        &self.dedupe
    }

    /// Returns the mutable shard-local dedupe cache.
    pub fn dedupe_mut(&mut self) -> &mut DedupeCache {
        &mut self.dedupe
    }

    /// Records a disruptor-released handoff in ascending sequence order.
    pub fn record_released_handoff(&mut self, sequence: u64, envelope: CommandEnvelope<A>) {
        let handoff = ShardHandoff { sequence, envelope };
        let index = self
            .handoffs
            .iter()
            .position(|existing| existing.sequence > sequence)
            .unwrap_or(self.handoffs.len());

        self.handoffs.insert(index, handoff);
    }

    /// Pops the next processable shard handoff.
    pub fn pop_handoff(&mut self) -> Option<ShardHandoff<A>> {
        self.handoffs.pop_front()
    }

    /// Returns the number of processable handoffs.
    pub fn pending_handoffs(&self) -> usize {
        self.handoffs.len()
    }

    /// Processes one disruptor-released handoff through replay, decide, durable append, and reply.
    pub async fn process_next_handoff<S, C>(&mut self, store: &S, codec: &C) -> RuntimeResult<bool>
    where
        S: RuntimeEventStore,
        C: RuntimeEventCodec<A>,
        A::Error: std::fmt::Display,
    {
        let Some(handoff) = self.pop_handoff() else {
            return Ok(false);
        };
        let envelope = handoff.envelope;

        let current_state = if let Some(cached) = self.cache.get(&envelope.stream_id) {
            cached.clone()
        } else {
            match rehydrate_state(store, codec, &envelope).await {
                Ok(rehydrated) => {
                    self.cache
                        .commit_state(envelope.stream_id.clone(), rehydrated.clone());
                    rehydrated
                }
                Err(error) => {
                    let _ = envelope.reply.send(Err(error));
                    return Ok(true);
                }
            }
        };

        let decision = match A::decide(&current_state, envelope.command, &envelope.metadata) {
            Ok(decision) => decision,
            Err(error) => {
                let _ = envelope.reply.send(Err(RuntimeError::Domain {
                    message: error.to_string(),
                }));
                return Ok(true);
            }
        };

        let mut new_events = Vec::with_capacity(decision.events.len());
        for event in &decision.events {
            match codec.encode(event, &envelope.metadata) {
                Ok(encoded) => new_events.push(encoded),
                Err(error) => {
                    let _ = envelope.reply.send(Err(error));
                    return Ok(true);
                }
            }
        }

        let append_request = match es_store_postgres::AppendRequest::new(
            envelope.stream_id.clone(),
            envelope.expected_revision,
            envelope.metadata.clone(),
            envelope.idempotency_key.clone(),
            new_events,
        ) {
            Ok(request) => request,
            Err(error) => {
                let _ = envelope
                    .reply
                    .send(Err(RuntimeError::from_store_error(error)));
                return Ok(true);
            }
        };

        match store.append(append_request).await {
            Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
                let mut staged_state = current_state;
                for event in &decision.events {
                    A::apply(&mut staged_state, event);
                }
                self.cache
                    .commit_state(envelope.stream_id.clone(), staged_state);
                self.dedupe.record(
                    DedupeKey {
                        tenant_id: envelope.metadata.tenant_id.clone(),
                        idempotency_key: envelope.idempotency_key.clone(),
                    },
                    DedupeRecord {
                        append: committed.clone(),
                    },
                );
                let _ = envelope
                    .reply
                    .send(Ok(CommandOutcome::new(decision.reply, committed)));
            }
            Ok(es_store_postgres::AppendOutcome::Duplicate(committed)) => {
                self.dedupe.record(
                    DedupeKey {
                        tenant_id: envelope.metadata.tenant_id.clone(),
                        idempotency_key: envelope.idempotency_key.clone(),
                    },
                    DedupeRecord {
                        append: committed.clone(),
                    },
                );
                let _ = envelope
                    .reply
                    .send(Ok(CommandOutcome::new(decision.reply, committed)));
            }
            Err(error) => {
                let _ = envelope
                    .reply
                    .send(Err(RuntimeError::from_store_error(error)));
            }
        }

        Ok(true)
    }
}

async fn rehydrate_state<A, S, C>(
    store: &S,
    codec: &C,
    envelope: &CommandEnvelope<A>,
) -> RuntimeResult<A::State>
where
    A: Aggregate,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A>,
{
    let batch = store
        .load_rehydration(&envelope.metadata.tenant_id, &envelope.stream_id)
        .await
        .map_err(RuntimeError::from_store_error)?;
    let mut state = match &batch.snapshot {
        Some(snapshot) => codec.decode_snapshot(snapshot)?,
        None => A::State::default(),
    };

    for stored in &batch.events {
        let event = codec.decode(stored)?;
        A::apply(&mut state, &event);
    }

    Ok(state)
}

/// Disruptor-backed handle for accepting routed commands into one shard.
pub struct ShardHandle<A: Aggregate> {
    shard_id: ShardId,
    state: ShardState<A>,
    path: DisruptorPath<ShardHandoffToken>,
    pending: HashMap<ShardHandoffToken, CommandEnvelope<A>>,
    next_local_handoff_id: u64,
}

impl<A: Aggregate> ShardHandle<A> {
    /// Creates a shard handle with an owned state object and disruptor path.
    pub fn new(shard_id: ShardId, ring_size: usize) -> RuntimeResult<Self> {
        Ok(Self {
            shard_id,
            state: ShardState::new(shard_id),
            path: DisruptorPath::new(shard_id, ring_size, ShardHandoffToken::placeholder)?,
            pending: HashMap::new(),
            next_local_handoff_id: 0,
        })
    }

    /// Returns this handle's shard-owned state.
    pub fn state(&self) -> &ShardState<A> {
        &self.state
    }

    /// Returns this handle's mutable shard-owned state.
    pub fn state_mut(&mut self) -> &mut ShardState<A> {
        &mut self.state
    }

    /// Returns the number of accepted envelopes waiting for disruptor release.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Publishes a routed command token without making the command processable yet.
    pub fn accept_routed(&mut self, routed: RoutedCommand<A>) -> RuntimeResult<u64> {
        if routed.shard_id != self.shard_id {
            return Err(RuntimeError::Unavailable);
        }

        let local_handoff_id = LocalHandoffId::new(self.next_local_handoff_id);
        let token = ShardHandoffToken {
            tenant_id: routed.envelope.metadata.tenant_id.clone(),
            stream_id: routed.envelope.stream_id.clone(),
            idempotency_key: routed.envelope.idempotency_key.clone(),
            local_handoff_id,
        };

        let sequence = self.path.try_publish(token.clone())?;
        self.pending.insert(token, routed.envelope);
        self.next_local_handoff_id += 1;

        Ok(sequence)
    }

    /// Drains disruptor-released tokens and records matching processable handoffs.
    pub fn drain_released_handoffs(&mut self) -> RuntimeResult<usize> {
        let mut drained = 0;

        for released in self.path.poll_released() {
            if let Some(envelope) = self.pending.remove(&released.event) {
                self.state
                    .record_released_handoff(released.sequence, envelope);
                drained += 1;
            }
        }

        Ok(drained)
    }
}

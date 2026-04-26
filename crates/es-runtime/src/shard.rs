use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use es_kernel::Aggregate;
use metrics::{gauge, histogram};
use tracing::info_span;

use crate::{
    AggregateCache, AggregateCacheKey, CommandEnvelope, CommandOutcome, DedupeCache, DedupeKey,
    DedupeRecord, DisruptorPath, RoutedCommand, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
    RuntimeResult, ShardId,
};

/// Command envelope released from the disruptor path for shard processing.
pub struct ShardHandoff<A: Aggregate> {
    /// Local disruptor sequence used only for ordered processing diagnostics.
    pub sequence: u64,
    /// Time the disruptor release was observed by the shard owner.
    pub released_at: Instant,
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
        let handoff = ShardHandoff {
            sequence,
            released_at: Instant::now(),
            envelope,
        };
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
        let command_started_at = Instant::now();
        let ring_wait_seconds = command_started_at
            .duration_since(handoff.released_at)
            .as_secs_f64();
        let shard_label = self.shard_id.value().to_string();
        gauge!("es_shard_queue_depth", "shard" => shard_label.clone())
            .set(self.pending_handoffs() as f64);
        histogram!("es_ring_wait_seconds", "shard" => shard_label.clone())
            .record(ring_wait_seconds);
        let envelope = handoff.envelope;
        let aggregate = aggregate_label::<A>();
        let span = info_span!(
            "shard.process_handoff",
            command_id = %envelope.metadata.command_id,
            correlation_id = %envelope.metadata.correlation_id,
            causation_id = ?envelope.metadata.causation_id,
            tenant_id = %envelope.metadata.tenant_id.as_str(),
            stream_id = %envelope.stream_id.as_str(),
            shard_id = self.shard_id.value(),
            global_position = tracing::field::Empty,
        );
        let _entered = span.enter();
        let dedupe_key = DedupeKey {
            tenant_id: envelope.metadata.tenant_id.clone(),
            idempotency_key: envelope.idempotency_key.clone(),
        };

        if let Some(record) = self.dedupe.get(&dedupe_key) {
            let outcome = replay_command_outcome::<A, C>(codec, &record.replay);
            let _ = envelope.reply.send(outcome);
            histogram!(
                "es_command_latency_seconds",
                "aggregate" => aggregate,
                "outcome" => "duplicate_cache",
            )
            .record(command_started_at.elapsed().as_secs_f64());
            return Ok(true);
        }

        match store
            .lookup_command_replay(&envelope.metadata.tenant_id, &envelope.idempotency_key)
            .await
        {
            Ok(Some(replay)) => {
                if let Some(global_position) = replay.append.global_positions.last() {
                    span.record("global_position", global_position);
                }
                let outcome = replay_command_outcome::<A, C>(codec, &replay);
                if outcome.is_ok() {
                    self.dedupe
                        .record(dedupe_key.clone(), DedupeRecord { replay });
                }
                let _ = envelope.reply.send(outcome);
                histogram!(
                    "es_command_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "duplicate_store",
                )
                .record(command_started_at.elapsed().as_secs_f64());
                return Ok(true);
            }
            Ok(None) => {}
            Err(error) => {
                let _ = envelope
                    .reply
                    .send(Err(RuntimeError::from_store_error(error)));
                return Ok(true);
            }
        }

        let cache_key = AggregateCacheKey {
            tenant_id: envelope.metadata.tenant_id.clone(),
            stream_id: envelope.stream_id.clone(),
        };
        let expected_revision = A::expected_revision(&envelope.command);

        let current_state = if let Some(cached) = self.cache.get(&cache_key) {
            cached.clone()
        } else if expected_revision == es_core::ExpectedRevision::NoStream {
            A::State::default()
        } else {
            match rehydrate_state(
                store,
                codec,
                &envelope.metadata.tenant_id,
                &envelope.stream_id,
            )
            .await
            {
                Ok(rehydrated) => {
                    self.cache
                        .commit_state(cache_key.clone(), rehydrated.clone());
                    rehydrated
                }
                Err(error) => {
                    let _ = envelope.reply.send(Err(error));
                    return Ok(true);
                }
            }
        };

        let decision_started_at = Instant::now();
        let decision = match A::decide(&current_state, envelope.command, &envelope.metadata) {
            Ok(decision) => {
                histogram!(
                    "es_decision_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "success",
                )
                .record(decision_started_at.elapsed().as_secs_f64());
                decision
            }
            Err(error) => {
                histogram!(
                    "es_decision_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "domain_error",
                )
                .record(decision_started_at.elapsed().as_secs_f64());
                histogram!(
                    "es_command_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "domain_error",
                )
                .record(command_started_at.elapsed().as_secs_f64());
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
        let command_reply_payload = match codec.encode_reply(&decision.reply) {
            Ok(payload) => payload,
            Err(error) => {
                let _ = envelope.reply.send(Err(error));
                return Ok(true);
            }
        };

        let append_request = match es_store_postgres::AppendRequest::new(
            envelope.stream_id.clone(),
            envelope.expected_revision,
            envelope.metadata.clone(),
            envelope.idempotency_key.clone(),
            new_events,
        ) {
            Ok(request) => request.with_command_reply_payload(command_reply_payload.clone()),
            Err(error) => {
                let _ = envelope
                    .reply
                    .send(Err(RuntimeError::from_store_error(error)));
                return Ok(true);
            }
        };

        match store.append(append_request).await {
            Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
                if let Some(global_position) = committed.global_positions.last() {
                    span.record("global_position", global_position);
                }
                let mut staged_state = current_state;
                for event in &decision.events {
                    A::apply(&mut staged_state, event);
                }
                self.cache.commit_state(cache_key, staged_state);
                self.dedupe.record(
                    dedupe_key,
                    DedupeRecord {
                        replay: es_store_postgres::CommandReplayRecord {
                            append: committed.clone(),
                            reply: command_reply_payload.clone(),
                        },
                    },
                );
                let reply = decision.reply;
                let _ = envelope
                    .reply
                    .send(Ok(CommandOutcome::new(reply, committed)));
                histogram!(
                    "es_command_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "committed",
                )
                .record(command_started_at.elapsed().as_secs_f64());
            }
            Ok(es_store_postgres::AppendOutcome::Duplicate(committed)) => {
                if let Some(global_position) = committed.global_positions.last() {
                    span.record("global_position", global_position);
                }
                match store
                    .lookup_command_replay(&envelope.metadata.tenant_id, &envelope.idempotency_key)
                    .await
                {
                    Ok(Some(replay)) => {
                        let outcome = replay_command_outcome::<A, C>(codec, &replay);
                        if outcome.is_ok() {
                            match rehydrate_state(
                                store,
                                codec,
                                &envelope.metadata.tenant_id,
                                &envelope.stream_id,
                            )
                            .await
                            {
                                Ok(refreshed) => {
                                    self.cache.commit_state(cache_key.clone(), refreshed);
                                }
                                Err(_) => {
                                    self.cache.invalidate(&cache_key);
                                }
                            }
                            self.dedupe.record(dedupe_key, DedupeRecord { replay });
                        }
                        let _ = envelope.reply.send(outcome);
                    }
                    Ok(None) => {
                        let _ = envelope.reply.send(Err(RuntimeError::Codec {
                            message: "duplicate append did not have a command replay record"
                                .to_owned(),
                        }));
                    }
                    Err(error) => {
                        let _ = envelope
                            .reply
                            .send(Err(RuntimeError::from_store_error(error)));
                    }
                }
                histogram!(
                    "es_command_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "duplicate",
                )
                .record(command_started_at.elapsed().as_secs_f64());
            }
            Err(error) => {
                histogram!(
                    "es_command_latency_seconds",
                    "aggregate" => aggregate,
                    "outcome" => "store_error",
                )
                .record(command_started_at.elapsed().as_secs_f64());
                let _ = envelope
                    .reply
                    .send(Err(RuntimeError::from_store_error(error)));
            }
        }

        Ok(true)
    }
}

fn replay_command_outcome<A, C>(
    codec: &C,
    replay: &es_store_postgres::CommandReplayRecord,
) -> RuntimeResult<CommandOutcome<A::Reply>>
where
    A: Aggregate,
    C: RuntimeEventCodec<A>,
{
    let reply = codec.decode_reply(&replay.reply)?;
    Ok(CommandOutcome::new(reply, replay.append.clone()))
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

async fn rehydrate_state<A, S, C>(
    store: &S,
    codec: &C,
    tenant_id: &es_core::TenantId,
    stream_id: &es_core::StreamId,
) -> RuntimeResult<A::State>
where
    A: Aggregate,
    S: RuntimeEventStore,
    C: RuntimeEventCodec<A>,
{
    let batch = store
        .load_rehydration(tenant_id, stream_id)
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

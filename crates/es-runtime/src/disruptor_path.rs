use std::fmt;

use disruptor::{BusySpinWithSpinLoopHint, EventPoller, Polling, Producer, SingleConsumerBarrier};

use crate::{RuntimeError, RuntimeResult, ShardId};

/// Event released by the disruptor consumer/poller path for async shard processing.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleasedHandoff<E> {
    /// Local disruptor sequence used only for diagnostics and ordered handoff.
    pub sequence: u64,
    /// Event copied from the released disruptor slot.
    pub event: E,
}

/// Narrow nonblocking disruptor publication path for one shard.
pub struct DisruptorPath<E: Clone + Send + Sync + 'static> {
    shard_id: ShardId,
    producer: disruptor::SingleProducer<E, SingleConsumerBarrier>,
    poller: EventPoller<E, disruptor::SingleProducerBarrier>,
    next_release_sequence: u64,
}

impl<E: Clone + Send + Sync + 'static> fmt::Debug for DisruptorPath<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DisruptorPath")
            .field("shard_id", &self.shard_id)
            .field("next_release_sequence", &self.next_release_sequence)
            .finish_non_exhaustive()
    }
}

impl<E: Clone + Send + Sync + 'static> DisruptorPath<E> {
    /// Creates a single-producer disruptor path with a caller-controlled event poller.
    pub fn new(
        shard_id: ShardId,
        ring_size: usize,
        event_factory: impl Fn() -> E + Send + Sync + 'static,
    ) -> RuntimeResult<Self> {
        if ring_size == 0 {
            return Err(RuntimeError::InvalidRingSize);
        }

        let builder =
            disruptor::build_single_producer(ring_size, event_factory, BusySpinWithSpinLoopHint);
        let (poller, builder) = builder.new_event_poller();
        let producer = builder.build();

        Ok(Self {
            shard_id,
            producer,
            poller,
            next_release_sequence: 0,
        })
    }

    /// Attempts to publish without waiting for ring capacity.
    pub fn try_publish(&mut self, event: E) -> RuntimeResult<u64> {
        self.producer
            .try_publish(|slot| {
                *slot = event;
            })
            .map(|sequence| sequence as u64)
            .map_err(|disruptor::RingBufferFull| RuntimeError::ShardOverloaded {
                shard_id: self.shard_id.value(),
            })
    }

    /// Drains handoffs that have been released through the disruptor poller path.
    pub fn poll_released(&mut self) -> Vec<ReleasedHandoff<E>> {
        let mut released = Vec::new();

        loop {
            match self.poller.poll() {
                Ok(mut events) => {
                    for event in &mut events {
                        released.push(ReleasedHandoff {
                            sequence: self.next_release_sequence,
                            event: event.clone(),
                        });
                        self.next_release_sequence += 1;
                    }
                }
                Err(Polling::NoEvents | Polling::Shutdown) => break,
            }
        }

        released
    }
}

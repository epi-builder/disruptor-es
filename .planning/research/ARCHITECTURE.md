# Architecture Research: Disruptor Event Sourcing Template

## Recommended Shape

The project should be a modular Rust workspace with a single runnable service and clear crate boundaries. The service should be deployable as one process for v1, while keeping adapter, command engine, storage, projection, and outbox boundaries explicit enough to split later.

```text
Client / Load Test
  -> Thin Adapter (HTTP/gRPC)
  -> bounded ingress
  -> Command Router
  -> Shard Runtime
       -> disruptor command ring
       -> processor-local aggregate cache
       -> rehydrate / decide / append
  -> Event Store
       -> events
       -> streams
       -> snapshots
       -> command_dedup
       -> outbox
  -> committed event notification
  -> Projectors / Outbox Dispatcher / Process Managers
  -> Query Store / Integration Stream
```

## Internal Components

### Thin Adapter

Responsibilities:

- Decode HTTP/gRPC requests into `CommandEnvelope`.
- Authenticate and validate request shape.
- Attach `command_id`, `idempotency_key`, `correlation_id`, `causation_id`, and trace context.
- Send command to a bounded engine ingress handle.
- Await reply via oneshot or equivalent response channel.

Non-responsibilities:

- No aggregate cache.
- No projector mutation.
- No broker publish.
- No long-held mutex around business state.

### Command Router

Responsibilities:

- Extract partition key from command metadata or domain command.
- Route to a shard owner.
- Apply bounded backpressure.
- Optionally expose queue-depth metrics per shard.

The v1 router can be local and static:

```text
shard_id = hash(tenant_id, bounded_context, aggregate_type, aggregate_id) % shard_count
```

The interface should leave room for future remote partition ownership:

```text
PartitionOwner { local_shard | remote_node(endpoint) | unavailable }
```

### Shard Runtime

Responsibilities:

- Own shard-local state and aggregate cache.
- Consume commands in shard order.
- Rehydrate aggregate from snapshot plus stream tail on cache miss.
- Run typed `decide`.
- Append events with optimistic concurrency.
- Advance cache only after append success.
- Reply to caller only after durable commit.
- Notify local committed-event ring or tailer after commit.

The shard runtime is where `disruptor-rs` belongs. It should be treated as a local execution fabric, not as a cross-process communication layer.

### Event Store

Minimum responsibilities:

- `load_stream(stream_id)`.
- `append(stream_id, expected_revision, events, metadata)`.
- `read_global_from(position, limit)`.
- `load_snapshot(stream_id)`.
- `save_snapshot(stream_id, version, state_blob)`.

Append transaction should write:

- Domain events.
- Stream version update.
- Command dedup result.
- Outbox rows derived from committed events.

### Projection Runtime

Projection should be checkpointed by global position:

```text
BEGIN
  apply read model mutations
  update projector_offsets
COMMIT
```

Projectors should be restartable and rebuildable. They can consume local committed-event notifications for low latency, but they must recover from event store tailing.

### Outbox Dispatcher

The dispatcher should use durable outbox rows as the source:

```text
SELECT pending ORDER BY global_position
publish
mark published
```

It should not publish directly from the command handler. The broker adapter should be optional in v1; a logging or in-memory publisher is enough if the outbox contract is real.

### Process Manager

For commerce, a process manager can handle:

```text
OrderPlaced
  -> ReserveProductInventory
InventoryReserved
  -> ConfirmOrder
InventoryReservationRejected
  -> RejectOrder
```

This validates cross-entity behavior without distributed transactions. The process manager should issue commands back through the same command gateway and rely on idempotency.

## Ordered and Unordered Modes

The template should expose a common command API while supporting two execution modes:

| Mode | Use Case | Routing | Concurrency |
|------|----------|---------|-------------|
| Unordered/OCC | Ordinary aggregate commands | Any capable instance, then stream OCC | Optimistic concurrency retry |
| Ordered partition | Matching, ledgers, account balances, hot ordered keys | Mandatory owner for key | Single logical writer per key |

The v1 implementation can build ordered local partitions first, because it also works for ordinary aggregate commands and teaches the right hot-path structure.

## Future Service-Level Architecture

```text
CDN / LB / API Gateway
  -> HTTP API Gateway / WebSocket Gateway
  -> internal gRPC command RPC
  -> Domain Command Service
       -> local disruptor shard runtimes
       -> event store + outbox
  -> Broker / CDC from outbox
  -> Projection Workers
  -> Query API / WebSocket Fanout
```

Gateway-to-command-service calls should be synchronous only for command acceptance/commit result. Cross-bounded-context propagation should be asynchronous through outbox and broker.

## Build Order Implications

1. Define workspace, core types, command metadata, and domain kernel traits.
2. Implement event store contract and a PostgreSQL or SQLite-backed adapter.
3. Implement local command router and shard runtime.
4. Integrate `disruptor-rs` in the shard execution path.
5. Add commerce example domain.
6. Add projections and query models.
7. Add outbox and dispatcher.
8. Add adapter skeleton and benchmarks.
9. Add process-manager example and stress scenarios.

## Scaling and Failover Assumptions

v1 should be honest about limits:

- Local static shards prove the programming model.
- Distributed partition ownership is deferred.
- If a local process dies, it recovers from event store and snapshots.
- Future HA needs a coordinator and owner transfer protocol; disruptor does not solve this.

The design should document that logical single-writer does not require permanent physical single-node ownership, but implementing failover is a separate distributed-systems phase.

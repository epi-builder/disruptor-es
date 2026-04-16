# Features Research: Disruptor Event Sourcing Template

## Domain Framing

This is a technical service template, not a commerce application. The commerce/order domain is a fixture used to prove that the generic engine handles related entities and cross-entity workflows:

- `User`: customer identity and account status.
- `Product`: catalog item and inventory policy.
- `Order`: order lifecycle that references a user and one or more products.

The template should make it straightforward to replace those domain types with wallet, settlement, risk, trading, or other bounded contexts later.

## Table Stakes

### Generic Domain Kernel

- Typed `AggregateKernel` trait with associated `Command`, `Event`, `State`, `Reply`, and `Error` types.
- Deterministic `stream_id` and `partition_key` derivation.
- `decide` function that emits zero or more domain events and a reply.
- `apply` function that mutates state from committed or replayed events.
- Explicit prohibition on remote RPC inside `decide`; external work belongs in process managers or reference-data caches.

### Command Execution Runtime

- Bounded ingress path from adapters into the command engine.
- Partition router that maps commands to shard owners by aggregate key.
- Shard-local aggregate cache with no global mutable business state.
- `disruptor-rs` command ring inside each shard runtime or bounded shard group.
- Commit acknowledgment only after event store append succeeds.
- Bounded backpressure behavior when ingress or shard queues are full.

### Event Store

- Append-only `events` table with global position and per-stream revision.
- Optimistic concurrency checks via expected revision.
- Command deduplication keyed by tenant and idempotency key.
- Snapshot support for rehydration acceleration.
- Replay APIs for stream and global-position catch-up.
- Metadata capture: event ID, command ID, causation ID, correlation ID, tenant ID, schema version, recorded time.

### CQRS Projection

- Projector trait with named projector identity.
- Projector offset table updated in the same transaction as read-model changes.
- Startup catch-up from event store.
- Rebuild support for disposable read models.
- Query API option to wait briefly for a minimum projected global position.

### Outbox and Integration Events

- Outbox rows written in the same transaction as domain events.
- Domain event to integration event mapping.
- Dispatcher that publishes pending outbox rows and marks them published.
- Retry, attempt count, and next-at scheduling.
- Idempotent publication keyed by source event and topic.

### Process Manager / Saga Basics

- Event-reactive process manager trait.
- At least one sample cross-entity flow, such as product reservation during order placement.
- Process manager state persisted by events or by an explicit durable state table.
- Compensating commands/events for failures instead of distributed rollback.

### Adapter Boundary

- Thin HTTP/gRPC adapter skeleton that decodes requests and forwards commands through bounded channels.
- Reply channel from command engine back to adapter.
- No adapter-owned aggregate maps or shared mutable domain state.
- WebSocket guidance documented as separate gateway/fanout path, not hot command path.

### Observability and Stress Testing

- Metrics for ingress depth, shard queue depth, ring wait, decision time, append time, projection lag, outbox lag, and p95/p99 end-to-end latency.
- Structured tracing with correlation and causation IDs.
- Benchmark suites split into ring-only, domain-only, adapter-only, storage, full E2E, soak, and degraded dependency scenarios.
- Hot-key and burst traffic scenarios, not only uniform distribution.

## Differentiators

- Two execution modes behind one command-service abstraction:
  - Unordered/OCC mode for ordinary aggregate workloads.
  - Ordered partition mode for strict per-key sequencing.
- Out-of-band branch patterns for slow projectors, audit, metrics, and broker notification.
- Local single-node partition ownership interface designed to evolve toward etcd/Raft/Kubernetes ownership.
- Example documentation showing how to extract a new bounded context from the template.
- Optional event poller path for projection rebuild and offline catch-up.

## Anti-Features

- A global `Arc<Mutex<EngineState>>` shared by all request handlers.
- Treating disruptor rings as durable queues.
- Publishing uncommitted events to brokers or WebSocket clients.
- Blocking command success on projection completion.
- Running broker publish, slow read-model joins, or client fanout as gating consumers on the command ring.
- JSON/reflection-based domain logic in the hot path.
- Sequence modulo distribution for ordered aggregates.
- Unbounded channels between adapter and engine.

## Dependencies and Complexity

| Capability | Complexity | Dependencies |
|------------|------------|--------------|
| Typed aggregate kernel | Medium | Core domain crates |
| Partitioned command runtime | High | Kernel, routing, disruptor integration |
| Event store append/OCC | High | SQL schema, repository abstraction |
| Projection checkpoints | Medium | Event store and query DB |
| Outbox dispatcher | Medium | Event store transaction and broker adapter seam |
| Process manager | Medium-High | Event store, command gateway, idempotency |
| Adapter skeleton | Medium | Runtime handles, bounded ingress, reply path |
| Stress tests | High | Runtime instrumentation and repeatable workloads |

## Recommended v1 Scope

Include the kernel, one bounded context, local partitioning, SQL event store, projections, outbox table and dispatcher loop, HTTP or gRPC adapter skeleton, and benchmark harness. Defer distributed partition ownership and real broker integration until the local contracts are stable.

# Pitfalls Research: Disruptor Event Sourcing Template

## 1. Measuring the Wrong Thing

**Pitfall:** Comparing disruptor microbenchmark results to end-to-end HTTP/WebSocket performance.

**Warning signs:**

- Ring-only latency is quoted as service latency.
- p99 is much worse than average but not investigated.
- Serialization, routing, database append, socket write, and queue wait are not measured separately.

**Prevention:**

- Keep benchmark layers separate: ring-only, domain-only, adapter-only, storage-only, full E2E, soak, and chaos.
- Record p50, p95, p99, max, throughput, queue depth, and reject rate.

**Metric/Test:** Per-stage latency histogram with correlation ID.

**Phase:** Benchmark and observability phase.

## 2. Mutex-Heavy Hot Path

**Pitfall:** Putting aggregate state, dedupe cache, or projector state behind `Arc<Mutex<_>>` shared by async handlers.

**Warning signs:**

- gRPC/HTTP handlers directly mutate aggregate maps.
- Mutex guards cross `.await`.
- CPU is underused while latency rises.
- Lock wait time appears in profiling.

**Prevention:**

- Use thin adapters and bounded message passing.
- Make shard runtime the single owner of hot aggregate state.
- Keep processor-local state inside the disruptor consumer or shard worker.

**Metric/Test:** Lock wait profiling, shard queue depth, handler await time.

**Phase:** Command runtime phase.

## 3. Treating Disruptor as Durable Infrastructure

**Pitfall:** Returning success when a command is published to a ring instead of when events are committed.

**Warning signs:**

- Ring sequence is used as business event position.
- Crash loses accepted commands.
- Projectors receive events before append commit.

**Prevention:**

- Acknowledge only after event store append transaction commits.
- Treat ring publication as execution scheduling, not durability.
- Persist outbox in the same append transaction.

**Metric/Test:** Kill process between publish and append; accepted commands must not be falsely acknowledged.

**Phase:** Event store phase.

## 4. Slow Consumers Gating the Critical Ring

**Pitfall:** Audit, projection, broker publish, and metrics all run as gating consumers on the same command-critical disruptor path.

**Warning signs:**

- A slow projector reduces command throughput.
- Broker outage causes command timeout.
- Ring producer stalls behind non-critical consumers.

**Prevention:**

- Separate command append path from projection/outbox dispatch.
- Use local notifications only as hints; durable catch-up comes from event store/outbox.
- Move slow work to out-of-band branches, pollers, or separate worker loops.

**Metric/Test:** Stop broker/projector during load; command append path should degrade only through storage pressure, not broker availability.

**Phase:** Projection and outbox phases.

## 5. Wrong Partition Key

**Pitfall:** Splitting ordered business state by sequence modulo or random worker assignment.

**Warning signs:**

- Same aggregate appears on multiple shards.
- OCC conflicts spike under normal load.
- Replays produce nondeterministic results.

**Prevention:**

- Route by tenant, bounded context, aggregate type, and aggregate ID.
- For strict ordered domains, route by ordered key such as symbol or account.
- Make partition key derivation part of the domain contract.

**Metric/Test:** Invariant test that all commands for a key are routed to one shard under stable partition config.

**Phase:** Routing phase.

## 6. Over-Generic Domain Logic

**Pitfall:** Implementing all rules through JSON, reflection, or string DSL evaluation in the hot path.

**Warning signs:**

- Domain state is `serde_json::Value`.
- Every decision allocates strings and dynamic maps.
- Compiler cannot enforce event/command compatibility.

**Prevention:**

- Keep infrastructure generic, domain logic typed.
- Use policy snapshots or precompiled decision tables if rules are configurable.
- Record `policy_version` in event metadata for replay and audit.

**Metric/Test:** Allocation profiling in command benchmarks.

**Phase:** Domain kernel phase.

## 7. Unbounded Queues

**Pitfall:** Letting adapters enqueue unlimited commands when command service is slower than ingress.

**Warning signs:**

- Memory grows during bursts.
- Latency keeps rising long after traffic drops.
- Clients time out and retry, making overload worse.

**Prevention:**

- Use bounded ingress queues.
- Return 429/503 or retryable gRPC status when full.
- Expose queue depth and admission rejection metrics.

**Metric/Test:** Burst test with fixed command capacity and explicit rejection budget.

**Phase:** Adapter and runtime phase.

## 8. Projection Consistency Confusion

**Pitfall:** Treating read models as immediately consistent with command writes.

**Warning signs:**

- Command handler waits for all projectors.
- Query tests assume immediate read-after-write without an explicit wait.
- Projector outage breaks command success.

**Prevention:**

- Return committed `global_position` in command replies.
- Support query options like `min_global_position` with short wait.
- Document eventual consistency.

**Metric/Test:** Projection lag metric and read-your-own-write integration test using `min_global_position`.

**Phase:** Projection/query phase.

## 9. Outbox Double-Write Bugs

**Pitfall:** Publishing to broker directly after event append without a durable outbox.

**Warning signs:**

- Event committed but integration message missing after crash.
- Broker publish succeeds but DB commit fails.
- Retried command emits duplicate external messages.

**Prevention:**

- Write outbox rows in append transaction.
- Dispatch from outbox table.
- Make dispatcher idempotent by source event and topic.

**Metric/Test:** Crash/failure injection between append, publish, and mark-published.

**Phase:** Outbox phase.

## 10. Premature Distributed Ownership

**Pitfall:** Trying to implement Raft/etcd partition ownership before local engine contracts are stable.

**Warning signs:**

- Distributed failover dominates v1.
- Local command correctness is untested.
- Ownership movement hides kernel bugs.

**Prevention:**

- Define owner abstraction now.
- Implement static local owner first.
- Add distributed ownership as a later phase with explicit recovery tests.

**Metric/Test:** Local recovery from event store after process restart.

**Phase:** Roadmap/future HA phase.

# Research Summary: Disruptor Event Sourcing Template

**Date:** 2026-04-16

## Key Findings

### Stack

Use a Rust 2024 workspace with project-owned event sourcing/CQRS abstractions. The recommended baseline is:

- Rust 2024.
- `disruptor` 4.x from `nicholassm/disruptor-rs` for the in-process Disruptor implementation.
- `tokio` for adapter/background I/O, not domain decisions.
- `axum` first for a thin HTTP adapter.
- `tonic` later or optional for internal gRPC boundaries.
- PostgreSQL plus `sqlx` for event store, snapshots, command deduplication, outbox, projector offsets, and read models.
- `tracing`, OpenTelemetry OTLP, and `hdrhistogram` for visibility into p95/p99 behavior.
- `criterion`, `divan`, `proptest`, `testcontainers`, and `cargo-nextest` for benchmark and correctness coverage.

The project should not adopt a generic Rust CQRS framework as the foundation. The template needs exact control over append transactions, optimistic concurrency, outbox semantics, partition routing, and benchmark boundaries.

### Table Stakes

- Typed aggregate kernel with deterministic stream and partition derivation.
- Bounded command ingress and partition router.
- Shard-local single-owner runtime.
- `disruptor` integration inside the command runtime only.
- Durable event store with per-stream revision, global position, metadata, snapshots, and command deduplication.
- CQRS projector runtime with durable checkpoints.
- Outbox dispatcher sourced from committed outbox rows.
- Commerce fixture domain with `User`, `Product`, and `Order`.
- Thin adapter boundary that avoids shared mutable business state.
- Stress-test harnesses that isolate ring, domain, adapter, storage, full E2E, projection, and outbox behavior.

### Architecture

The primary process shape is:

```text
Thin Adapter
  -> bounded ingress
  -> partition router
  -> shard runtime
  -> disruptor command ring
  -> typed rehydrate/decide/append
  -> event store commit
  -> reply

Committed events / outbox
  -> projectors
  -> read models
  -> outbox dispatcher
  -> optional broker adapter
```

The disruptor ring is not the source of truth. It is a local scheduling and fan-out primitive. The event store append commit is the command success boundary.

### Watch Out For

- Do not compare disruptor microbenchmarks directly to full HTTP/WebSocket service latency.
- Do not put aggregate maps behind global `Arc<Mutex<_>>`.
- Do not acknowledge commands before durable append.
- Do not let projectors, broker publish, audit, or WebSocket fanout gate the command ring.
- Do not route ordered aggregate commands by sequence modulo.
- Do not make domain decisions through JSON/reflection in the hot path.
- Do not use unbounded queues between adapters and the engine.
- Do not block command success on CQRS projection completion.
- Do not implement distributed partition ownership before local engine correctness is proven.

## Recommended Phase Bias

1. Establish contracts and workspace boundaries before implementation details.
2. Build the event store and append transaction early because it is the trust boundary.
3. Validate `disruptor` integration with ring-only and domain-only benchmarks before relying on it.
4. Add the commerce fixture after kernel/runtime contracts exist.
5. Add projections, outbox, and adapter skeletons after durable command handling works.
6. Add stress tests and observability before considering the template credible.
7. Defer distributed owner failover until the local single-owner model is correct and measurable.

## Decisions to Carry Forward

| Decision | Reason |
|----------|--------|
| Use commerce fixture for v1 | Tests related entities and process-manager behavior without exchange matching complexity. |
| Use `disruptor` crate, not literal `disruptor-rs` crate | The maintained crate matching `nicholassm/disruptor-rs` is published as `disruptor`. |
| Use PostgreSQL + `sqlx` for storage | Explicit transactions and schemas matter more than ORM convenience. |
| Keep adapters thin | Prevents HTTP/gRPC/WebSocket concerns from forcing locks into business state. |
| Split benchmarks by layer | End-to-end tests alone hide the actual bottleneck. |

## Sources

See `STACK.md` for current crate and platform source links. `FEATURES.md`, `ARCHITECTURE.md`, and `PITFALLS.md` contain local synthesis from the project brief and architecture constraints.

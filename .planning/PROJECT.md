# Disruptor Event Sourcing Template

## What This Is

A Rust reference project for building generic business-logic processing services with `disruptor-rs`, event sourcing, CQRS, outbox-based integration, and partitioned single-owner execution. The first implementation uses a compact commerce domain with users, products, and orders so the architecture can prove relationships, command routing, event append, projection, and cross-entity workflow patterns without turning the project into a full product.

The output should be usable as a template for later domain services. Domain behavior matters only insofar as it validates the technical architecture, extension points, performance boundaries, and operational practices.

## Core Value

Provide a reusable, production-shaped Rust service template where committed events are the source of truth and `disruptor-rs` is used only as the in-process ordered execution engine.

## Requirements

### Validated

- [x] Phase 01 validated a Rust 2024 workspace with typed core metadata, synchronous aggregate kernel contracts, visible service boundary crates, an example commerce aggregate, and dependency-boundary tests for deterministic lower-level crates.
- [x] Phase 02 validated PostgreSQL as the durable event-store source of truth with stream optimistic concurrency, full event metadata, tenant-scoped command dedupe, snapshot rehydration inputs, and global-position reads.
- [x] Phase 03 validated a bounded local command runtime with tenant-aware partition routing, shard-local aggregate and dedupe caches, nonblocking disruptor handoff, and commit-gated replies wired through `CommandEngine`.
- [x] Phase 04 validated the commerce fixture domain with typed user, product, and order aggregates, replayable lifecycle/inventory/order events, explicit cross-entity IDs, typed invalid-state errors, and generated replay/invariant tests.
- [x] Phase 05 validated CQRS projection contracts, tenant-scoped PostgreSQL projector offsets, order summary and product inventory read models, restart-safe catch-up, malformed payload rollback, and bounded read-your-own-write query waits.
- [x] Phase 06 validated durable outbox integration rows, append-transaction outbox creation, idempotent dispatcher publication/retry semantics, and an app-composed commerce process manager that issues follow-up commands through runtime gateways.
- [x] Phase 07 validated thin HTTP command adapters, bounded gateway responses, structured observability, PostgreSQL integration coverage, layer-separated benchmarks, measured single-service stress signals, and template guidance for hot-path boundaries.
- [x] Phase 08 validated runtime duplicate command replay with durable typed reply payloads, shard-local and PostgreSQL idempotency lookup before aggregate decision, HTTP duplicate retry coverage, and process-manager follow-up retry coverage.
- [x] Phase 09 validated tenant-scoped runtime aggregate cache identity, same-stream cross-tenant rehydration isolation, conflict-safe cache behavior, and duplicate append replay cache refresh/invalidation.

### Active

- [ ] Implement a generic command-processing kernel that supports typed aggregates, commands, events, replies, and domain errors.

### Out of Scope

- Full exchange matching engine v1 - valuable later, but it would dominate the initial architecture with price-time priority, market data, and hot-symbol failover concerns.
- Production-grade distributed partition ownership v1 - specify the interface and assumptions first; implement local/single-node ownership before Raft/etcd/Kubernetes controller integration.
- Full user-facing commerce product - the commerce domain is a technical fixture, not the product being built.
- Broker-specific production deployment v1 - define outbox contracts and provide an adapter seam before committing to Kafka, NATS, Redpanda, or another broker.
- Multi-language SDKs - Rust service template first.

## Context

The design should preserve a hard boundary between execution and durability:

- `disruptor-rs` is an in-process hot-path execution engine, not a durable queue or distributed bus.
- The event store commit is the authoritative success point for commands.
- Only committed events can feed projectors, outbox rows, sagas, metrics, and client-visible state changes.
- CQRS read models are eventually consistent and must not be treated as part of command success.
- Slow consumers, broker publishing, WebSocket fanout, analytics, and heavy projection work must stay off the critical command ring.

Prior exploration established several important pitfalls to avoid:

- Do not wrap the hot aggregate state in global `Arc<Mutex<HashMap<...>>>` structures.
- Do not let async adapters own business state directly; use thin request decoding plus bounded message passing.
- Do not let slow projectors or outbox dispatchers gate command throughput.
- Do not assume disruptor microbenchmarks predict end-to-end WebSocket or HTTP performance.
- Do not use sequence modulo workload splitting for ordered business state; route by aggregate or partition key.
- Do not publish to external brokers directly from request handling; use an outbox committed with the domain events.

The initial example domain should be intentionally small:

- `User` represents the actor/customer.
- `Product` represents a sellable item with inventory or availability rules.
- `Order` represents reservation/placement/cancellation and references both user and product.

This gives enough relationships to test uniqueness, entity references, projection joins, idempotency, inventory reservation, and process-manager behavior while keeping the architectural work visible.

## Constraints

- **Package manager**: Prefer `pnpm` for Node tooling and `uv` for Python tooling - required by local project instructions.
- **Language**: Rust-first service implementation - the core value is a Rust template around `disruptor-rs`.
- **Architecture**: Event store is the source of truth - disruptor rings must never be treated as durable state.
- **Consistency**: Same aggregate or ordered partition key must map to the same shard owner - required for replayable ordering and hot aggregate cache locality.
- **Concurrency**: Hot business state should be single-owner and processor-local where practical - avoid shared mutable state in adapter handlers.
- **Integration**: External publication must flow through outbox rows committed in the same transaction as domain events - avoids double-write failure modes.
- **Scalability**: Adapter, command engine, projection, and outbox concerns should be separable - enables later MSA deployment and independent stress testing.
- **Testing**: Performance tests must separate ring-only, domain-only, adapter-only, single-service integrated, full E2E, soak, and chaos scenarios - otherwise bottlenecks are hidden.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use commerce/order as the initial example domain | It provides multiple related entities and realistic workflows without the complexity of a matching engine. | Validated in Phase 04 with user, product, and order aggregates plus generated replay/invariant tests. |
| Keep `disruptor-rs` inside the command service process | The library is for in-process sequencing and fan-out, not cross-service communication. | Validated in Phase 03 with shard-local disruptor handoff and no durability coupling. |
| Treat event store append commit as command success | Durability must not depend on ring publication or projection completion. | Validated in Phase 02 with PostgreSQL append/OCC/dedupe transactions. |
| Keep projection freshness as a bounded query concern | Read-your-own-write support should not make projection completion part of command success. | Validated in Phase 05 with `MinimumGlobalPosition`, `WaitPolicy`, and `ProjectionLag` behavior. |
| Use durable outbox rows and process-manager offsets for integration workflows | External effects and cross-entity follow-ups must be recoverable, idempotent, and outside the hot command path. | Validated in Phase 06 with append-created outbox rows, dispatcher retries, worker ownership checks, and a commerce process manager composed in `app`. |
| Use typed domain kernels instead of JSON/reflection in the hot path | Preserves Rust type safety and avoids erasing the performance benefits of preallocated ring entries. | - Pending |
| Split generic infrastructure from domain rules | Enables reuse across future services while keeping domain logic strongly typed. | - Pending |
| Model adapters as thin ingress layers with bounded queues and reply channels | Prevents HTTP/gRPC/WebSocket concerns from forcing mutex-heavy business state. | Validated in Phase 03 with bounded `CommandGateway` ingress and `CommandEngine` wiring. |
| Treat stress report fields as operational claims | A template user will rely on append latency, queue depth, projection lag, and outbox lag to diagnose bottlenecks, so fields must be measured from the component they name. | Validated in Phase 07 with durable projection-lag computation, measured append latency, read-only shard-depth sampling, and backlog regression tests. |
| Replay duplicate commands before aggregate decision | Retries must preserve the original committed response even after state mutation, runtime cache miss, or process-manager replay. | Validated in Phase 08 with typed durable replay records, pre-decision shard/runtime lookup, and HTTP/process-manager duplicate retry tests. |
| Key shard-local aggregate cache state by tenant plus stream | Runtime hot state is an optimization over tenant-scoped durable rehydration, so a stream-only cache key can bypass tenant isolation. | Validated in Phase 09 with `AggregateCacheKey`, same-stream tenant regressions, conflict coverage, and duplicate append cache refresh/invalidation. |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `$gsd-transition`):
1. Requirements invalidated? -> Move to Out of Scope with reason
2. Requirements validated? -> Move to Validated with phase reference
3. New requirements emerged? -> Add to Active
4. Decisions to log? -> Add to Key Decisions
5. "What This Is" still accurate? -> Update if drifted

**After each milestone** (via `$gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check - still the right priority?
3. Audit Out of Scope - reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-19 after Phase 09 completion*

# Disruptor Event Sourcing Business Kernel

## What This Is

This project builds a reusable Rust template for high-throughput business logic services using disruptor-rs as the in-process execution fabric, event sourcing as the durable source of truth, and CQRS for read-side models. The first implementation uses a small Wallet bounded context as an example domain, but the primary product is the technical architecture and reusable kernel for future domain services.

It is intended for building large-traffic backend services where HTTP, WebSocket, or gRPC adapters remain thin while command processing, aggregate ownership, durable event appends, projection, and outbox publishing are handled by a domain command service.

## Core Value

The system must preserve correct per-aggregate command ordering and durable event-store commits while keeping the hot path free from shared mutable locks and slow external side effects.

## Requirements

### Validated

(None yet - ship to validate)

### Active

- [ ] Provide a Rust workspace template for disruptor-based command processing services.
- [ ] Implement a generic event-sourcing domain kernel with strongly typed commands, events, state, replies, and errors.
- [ ] Route each aggregate to a stable shard owner so write-side state can be processed without Arc<Mutex<...>> in the hot path.
- [ ] Treat the event store append commit as the source-of-truth success boundary.
- [ ] Separate domain events, integration events, outbox publishing, projectors, and query models.
- [ ] Provide thin HTTP/gRPC/WebSocket adapter patterns that communicate with the domain engine through bounded queues/RPC and oneshot replies.
- [ ] Include load-test and benchmark harnesses that measure ring-only, domain-only, adapter-only, full E2E, and soak/chaos scenarios separately.
- [ ] Document production deployment guidance for adapter tier, command service tier, event store, broker/outbox relay, projector workers, query APIs, and realtime fanout.

### Out of Scope

- Full production trading, wallet, or settlement product semantics - the example domain exists to prove the architecture, not to model a regulated financial product.
- Distributed transaction support across bounded contexts - cross-service consistency will be modeled through outbox, broker events, saga/process manager patterns, and compensation.
- Using disruptor-rs as a remote inter-process bus - disruptor remains an in-process execution primitive only.
- Direct client push or external broker publishing from the write-side hot path - these belong behind committed events, outbox, read models, or integration streams.
- A UI-first application - this project focuses on backend architecture, runtime behavior, correctness, and performance testing.

## Context

The motivating architecture is:

- External clients connect through HTTP, gRPC, or WebSocket gateways.
- Adapter services normalize client requests into canonical command envelopes.
- Domain command services own disruptor-based shard runtimes inside each process.
- Each aggregate is routed by tenant, bounded context, aggregate type, and aggregate id to a stable shard owner.
- The shard owner rehydrates state, runs decision logic, appends domain events transactionally, advances local cache only after commit, and replies after durable commit.
- Projectors, outbox relay, integration events, and realtime fanout run outside the critical write path and must recover from durable checkpoints.

The prior design discussion established several implementation constraints:

- disruptor-rs is an execution fabric, not a durability layer.
- Benchmarks for disruptor-rs do not represent end-to-end WebSocket or HTTP request latency because network I/O, parsing, serialization, async scheduling, DB append latency, Arc refcounting, mutex contention, and socket write fanout dominate once adapters are included.
- The hot path should prefer single-owner shard state, bounded queues, preallocated or small payloads, batch-friendly flow, and dedicated threads/cores where appropriate.
- Adapter and domain tiers may run in one process for small deployments, but large traffic systems should split adapter services from command services so connection churn, TLS, compression, slow clients, and realtime fanout do not contend with the business engine.
- Production readiness requires phase-separated stress testing: ring-only, domain-only, adapter-only, full E2E, and soak/chaos.

## Constraints

- **Package managers**: Use `pnpm` for Node tooling and `uv` for Python tooling - project-level instruction.
- **Language**: Rust is the primary implementation language because disruptor-rs, event-sourcing kernel traits, and low-level runtime ownership are central to the design.
- **Hot path concurrency**: Avoid shared mutable state guarded by `Arc<Mutex<_>>` in command decision and aggregate mutation paths - it destroys the single-owner performance model.
- **Durability boundary**: A command may only be acknowledged as successful after the event store append transaction commits - ring publication alone is not success.
- **Integration boundary**: External broker publishing must go through an outbox committed with the domain events - prevents dual-write loss and keeps broker failures out of the request path.
- **Read consistency**: CQRS read models are eventually consistent - command replies must expose positions/revisions so clients can request read-your-own-write waits when needed.
- **Backpressure**: All ingress paths must be bounded - unbounded queues hide overload until memory or latency collapses.
- **Performance evidence**: Benchmarks must report p50/p95/p99, queue depth, append latency, outbox lag, projection lag, and per-core CPU behavior, not only average throughput.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use disruptor-rs only inside domain command service processes | The library optimizes in-process thread handoff and consumer graphs, not distributed service communication | - Pending |
| Use Wallet as the first example bounded context | It is small enough to keep domain logic secondary while still exercising aggregate ordering, dedupe, event append, projection, and outbox | - Pending |
| Build strongly typed domain kernels instead of JSON/Any hot path reflection | Preserves Rust type safety, monomorphization, and disruptor-style performance assumptions | - Pending |
| Keep HTTP/gRPC/WebSocket adapters thin | Prevents protocol handling, sockets, slow clients, and async locks from becoming the business state owner | - Pending |
| Model production MSA communication as internal RPC for commands and outbox/broker for cross-context events | Maintains immediate command success/failure semantics without coupling bounded contexts through synchronous chains | - Pending |
| Treat benchmarking as layered, not only end-to-end | Layered tests reveal whether bottlenecks are ring handoff, decision CPU, DB append, adapter I/O, projection, broker, or fanout | - Pending |

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
*Last updated: 2026-04-16 after initialization*

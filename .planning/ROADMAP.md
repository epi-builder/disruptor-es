# Roadmap: Disruptor Event Sourcing Template

## Overview

This roadmap delivers a Rust service template where committed events are the source of truth and `disruptor-rs` is only the in-process ordered execution engine. The phases move from reusable contracts, to durable append, to local single-owner command execution, then prove the architecture with a compact commerce domain, CQRS, outbox/process-manager workflows, thin adapters, observability, and stress coverage. Distributed partition ownership is intentionally v2/out of scope for this milestone.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Workspace and Typed Kernel Contracts** - Developers can build the Rust workspace and define deterministic typed aggregate contracts. Completed 2026-04-16.
- [ ] **Phase 2: Durable Event Store Source of Truth** - Commands can persist committed events, metadata, dedupe records, snapshots, and global reads through the event store boundary.
- [ ] **Phase 3: Local Command Runtime and Disruptor Execution** - Adapter requests flow through bounded local shards that own hot state and reply only after durable append.
- [ ] **Phase 4: Commerce Fixture Domain** - User, product, and order behavior proves typed decisions, replay, relationships, and invariants.
- [ ] **Phase 5: CQRS Projection and Query Catch-Up** - Committed events feed checkpointed read models with restart and read-your-own-write support.
- [ ] **Phase 6: Outbox and Process Manager Workflows** - Committed events create durable integration rows and cross-entity workflows without distributed transactions.
- [ ] **Phase 7: Adapters, Observability, Stress, and Template Guidance** - Thin APIs, metrics, integration tests, single-service stress tests, benchmarks, and documentation make the template credible.

## Phase Details

### Phase 1: Workspace and Typed Kernel Contracts
**Goal**: Developers can create new typed event-sourced domains on top of a clean Rust workspace without pulling adapters, storage, brokers, or async runtime concerns into deterministic domain logic.
**Depends on**: Nothing (first phase)
**Requirements**: CORE-01, CORE-02, CORE-03, CORE-04
**Success Criteria** (what must be TRUE):
  1. Developer can build a Rust 2024 workspace with separate crates for core types, kernel traits, runtime, storage, projection, outbox, example domain, adapters, and app composition.
  2. Developer can define a typed aggregate with commands, events, replies, errors, stream IDs, partition keys, expected revisions, and metadata through reusable contracts.
  3. Developer can run domain decision logic synchronously and deterministically without adapter, database, broker, network, or shared mutable runtime dependencies.
  4. Developer can inspect crate boundaries and see that lower-level core/kernel crates do not depend on HTTP, gRPC, PostgreSQL, broker, or Tokio adapter concerns.
**Plans**: 4 plans
Plans:
- [x] 01-01-PLAN.md — Create root Rust 2024 workspace policy, toolchain pin, dependency policy, and validation strategy.
- [x] 01-02-PLAN.md — Implement typed core IDs/metadata and synchronous aggregate kernel contracts.
- [x] 01-03-PLAN.md — Create runtime, storage, projection, outbox, adapter, and app boundary crate placeholders.
- [x] 01-04-PLAN.md — Add example aggregate, replay tests, dependency boundary tests, and full workspace verification.

### Phase 2: Durable Event Store Source of Truth
**Goal**: Command success is anchored to durable append-only event-store commits, with stream concurrency, metadata, dedupe, snapshots, replay, and global-position reads available before runtime behavior depends on them.
**Depends on**: Phase 1
**Requirements**: STORE-01, STORE-02, STORE-03, STORE-04, STORE-05
**Success Criteria** (what must be TRUE):
  1. Developer can append domain events to a durable event store with per-stream optimistic concurrency and a clear committed result.
  2. Developer can inspect stored events and find event ID, stream ID, revision, global position, command/correlation/causation IDs, tenant ID, type, schema version, payload, metadata, and timestamp.
  3. Repeating a command with the same tenant/idempotency key returns the prior committed result instead of appending duplicate events.
  4. Aggregate state can be rehydrated from the latest snapshot plus subsequent stream events.
  5. Projectors and outbox workers can read committed events by global position, independent of any disruptor ring sequence.
**Plans**: 4 plans
Plans:
- [ ] 02-01-PLAN.md — Create PostgreSQL schema, storage dependencies, and migrated integration-test harness.
- [ ] 02-02-PLAN.md — Define storage API contracts, validation models, typed errors, and Rust UUID helper.
- [ ] 02-03-PLAN.md — Implement durable append, optimistic concurrency, metadata persistence, and command dedupe.
- [ ] 02-04-PLAN.md — Implement snapshot rehydration and tenant-scoped global-position reads.

### Phase 3: Local Command Runtime and Disruptor Execution
**Goal**: Requests enter a bounded local command engine, route by aggregate/partition key to a single shard owner, execute through an in-process disruptor path, and reply only after event-store commit.
**Depends on**: Phase 2
**Requirements**: RUNTIME-01, RUNTIME-02, RUNTIME-03, RUNTIME-04, RUNTIME-05, RUNTIME-06
**Success Criteria** (what must be TRUE):
  1. Adapter-facing callers can submit commands through bounded ingress and receive explicit overload behavior when capacity is exhausted.
  2. Commands for the same aggregate key consistently reach the same local shard owner under stable partition configuration.
  3. Shard-local aggregate and dedupe caches are owned by the shard runtime without global `Arc<Mutex<_>>` business-state maps.
  4. The runtime uses `disruptor-rs` as an in-process execution/fan-out mechanism, not as durability, a broker, or distributed ownership.
  5. Command replies are sent only after durable event-store append succeeds, and optimistic concurrency failures surface as typed conflict or retryable errors.
**Plans**: TBD

### Phase 4: Commerce Fixture Domain
**Goal**: The template includes a compact but realistic typed commerce fixture that proves related aggregates, cross-entity references, replayable events, and invalid-state prevention.
**Depends on**: Phase 3
**Requirements**: DOM-01, DOM-02, DOM-03, DOM-04, DOM-05, TEST-01
**Success Criteria** (what must be TRUE):
  1. Developer can register and activate/deactivate users, create and adjust products, and place/confirm/reject/cancel orders with replayable events.
  2. Orders reference user and product identifiers explicitly, and domain behavior validates the relationship assumptions needed by later process managers.
  3. Invalid orders, negative inventory, duplicate order placement, inactive users, and unavailable products are rejected by typed domain errors.
  4. Generated or equivalent command-sequence tests verify replay determinism and domain invariants for the fixture aggregates.
**Plans**: TBD

### Phase 5: CQRS Projection and Query Catch-Up
**Goal**: Committed events drive eventually consistent read models through checkpointed projectors that can restart, rebuild, catch up, and optionally satisfy read-your-own-write queries.
**Depends on**: Phase 4
**Requirements**: PROJ-01, PROJ-02, PROJ-03, PROJ-04
**Success Criteria** (what must be TRUE):
  1. Projectors apply committed events to read models and persist projector offsets in the same transaction.
  2. Developer can query order summary and product inventory read models derived only from committed events.
  3. After restart, a projector resumes from its saved global-position checkpoint and catches up without duplicating read-model effects.
  4. Query callers can request a minimum global position to support read-your-own-write behavior without making projection completion part of command success.
**Plans**: TBD

### Phase 6: Outbox and Process Manager Workflows
**Goal**: Integration events and cross-entity workflows are driven from committed events through durable outbox rows and process managers, keeping broker publication and workflow follow-ups off the hot command path.
**Depends on**: Phase 5
**Requirements**: INT-01, INT-02, INT-03, INT-04
**Success Criteria** (what must be TRUE):
  1. Event append transactions can create outbox rows derived from committed domain events in the same durable commit.
  2. A dispatcher can publish pending outbox rows through a publisher trait and mark successful rows as published.
  3. Dispatcher retries are idempotent by source event and topic, so repeated attempts do not create duplicate external effects.
  4. A process manager reacts to order/product events and issues follow-up commands through the same command gateway without distributed transactions.
**Plans**: TBD

### Phase 7: Adapters, Observability, Stress, and Template Guidance
**Goal**: The template is usable from thin HTTP boundaries, observable under load, verified against real storage paths, benchmarked by layer, and documented with the rules that keep the architecture correct.
**Depends on**: Phase 6
**Requirements**: API-01, API-02, API-03, API-04, OBS-01, OBS-02, TEST-02, TEST-03, TEST-04, DOC-01
**Success Criteria** (what must be TRUE):
  1. HTTP command endpoints decode requests, attach metadata, send through bounded ingress, and return stream revision, global position, correlation ID, and typed success/error payloads.
  2. Adapter code can be inspected and shown not to mutate aggregate state, projector state, or outbox state directly.
  3. Structured traces and metrics expose command identity, shard/global positions when available, ingress and shard depth, ring wait, decision latency, append latency, conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency.
  4. Integration tests verify append, conflicts, dedupe, snapshots, projector checkpoints, and outbox dispatch against real or containerized PostgreSQL.
  5. Benchmark artifacts separate ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded-dependency behavior.
  6. A single-service integrated stress test exercises the actual production-shaped service composition in one process and reports throughput, p50/p95/p99 latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization.
  7. Documentation states hot-path rules plus service-boundary guidance and explains how to interpret single-service stress results separately from ring-only microbenchmarks.
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Workspace and Typed Kernel Contracts | 4/4 | Complete | 2026-04-16 |
| 2. Durable Event Store Source of Truth | 0/TBD | Not started | - |
| 3. Local Command Runtime and Disruptor Execution | 0/TBD | Not started | - |
| 4. Commerce Fixture Domain | 0/TBD | Not started | - |
| 5. CQRS Projection and Query Catch-Up | 0/TBD | Not started | - |
| 6. Outbox and Process Manager Workflows | 0/TBD | Not started | - |
| 7. Adapters, Observability, Stress, and Template Guidance | 0/TBD | Not started | - |

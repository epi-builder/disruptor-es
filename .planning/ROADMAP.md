# Roadmap: Disruptor Event Sourcing Template

## Overview

This roadmap delivers a Rust service template where committed events are the source of truth and `disruptor-rs` is only the in-process ordered execution engine. The phases move from reusable contracts, to durable append, to local single-owner command execution, then prove the architecture with a compact commerce domain, CQRS, outbox/process-manager workflows, thin adapters, observability, and stress coverage. Distributed partition ownership is intentionally v2/out of scope for this milestone.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Workspace and Typed Kernel Contracts** - Developers can build the Rust workspace and define deterministic typed aggregate contracts. Completed 2026-04-16.
- [x] **Phase 2: Durable Event Store Source of Truth** - Commands can persist committed events, metadata, dedupe records, snapshots, and global reads through the event store boundary. Completed 2026-04-17.
- [x] **Phase 3: Local Command Runtime and Disruptor Execution** - Adapter requests flow through bounded local shards that own hot state and reply only after durable append. Completed 2026-04-17.
- [x] **Phase 4: Commerce Fixture Domain** - User, product, and order behavior proves typed decisions, replay, relationships, and invariants. Completed 2026-04-17.
- [x] **Phase 5: CQRS Projection and Query Catch-Up** - Committed events feed checkpointed read models with restart and read-your-own-write support. Completed 2026-04-18.
- [x] **Phase 6: Outbox and Process Manager Workflows** - Committed events create durable integration rows and cross-entity workflows without distributed transactions. Completed 2026-04-18.
- [x] **Phase 7: Adapters, Observability, Stress, and Template Guidance** - Thin APIs, metrics, integration tests, single-service stress tests, benchmarks, and documentation make the template credible. Completed 2026-04-19.
- [x] **Phase 8: Runtime Duplicate Command Replay** - Duplicate command retries are replayed from runtime/store idempotency before aggregate decision so API and process-manager retries return the original committed result. Completed 2026-04-19.
- [x] **Phase 9: Tenant-Scoped Runtime Aggregate Cache** - Shard-local aggregate cache entries include tenant identity so runtime hot state cannot bleed across tenant-scoped event-store streams. Completed 2026-04-19.
- [ ] **Phase 10: Duplicate-Safe Process Manager Follow-Up Keys** - Process-manager reserve/release follow-up commands use collision-safe idempotency keys for duplicate product lines while preserving retry replay.
- [ ] **Phase 11: v1 Archive Hygiene and HTTP E2E Debt** - Resolve accepted audit debt around runnable HTTP composition, HTTP-inclusive stress coverage, stale requirements traceability, validation hygiene, and advisory domain hardening.

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
- [x] 02-01-PLAN.md — Create PostgreSQL schema, storage dependencies, and migrated integration-test harness.
- [x] 02-02-PLAN.md — Define storage API contracts, validation models, typed errors, and Rust UUID helper.
- [x] 02-03-PLAN.md — Implement durable append, optimistic concurrency, metadata persistence, and command dedupe.
- [x] 02-04-PLAN.md — Implement snapshot rehydration and tenant-scoped global-position reads.

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
**Plans**: 4 plans
Plans:
- [x] 03-01-PLAN.md — Create runtime dependencies, typed command/error/store contracts, and fake-store test support.
- [x] 03-02-PLAN.md — Implement bounded gateway ingress and stable tenant-aware partition routing.
- [x] 03-03-PLAN.md — Prove shard-local cache ownership and compile the disruptor non-blocking handoff path.
- [x] 03-04-PLAN.md — Wire commit-gated command processing, conflict-safe cache behavior, and runtime flow validation.

### Phase 4: Commerce Fixture Domain
**Goal**: The template includes a compact but realistic typed commerce fixture that proves related aggregates, cross-entity references, replayable events, and invalid-state prevention.
**Depends on**: Phase 3
**Requirements**: DOM-01, DOM-02, DOM-03, DOM-04, DOM-05, TEST-01
**Success Criteria** (what must be TRUE):
  1. Developer can register and activate/deactivate users, create and adjust products, and place/confirm/reject/cancel orders with replayable events.
  2. Orders reference user and product identifiers explicitly, and domain behavior validates the relationship assumptions needed by later process managers.
  3. Invalid orders, negative inventory, duplicate order placement, inactive users, and unavailable products are rejected by typed domain errors.
  4. Generated or equivalent command-sequence tests verify replay determinism and domain invariants for the fixture aggregates.
**Plans**: 4 plans
Plans:
- [x] 04-01-PLAN.md — Create commerce module foundation, typed IDs, quantity value object, and compile-visible aggregate module contracts.
- [x] 04-02-PLAN.md — Implement user registration, activation, deactivation, replay, and typed lifecycle errors.
- [x] 04-03-PLAN.md — Implement product creation, inventory adjustment, reservation, release, replay, and nonnegative inventory invariants.
- [x] 04-04-PLAN.md — Implement order lifecycle relationships and generated command-sequence invariant tests.

### Phase 5: CQRS Projection and Query Catch-Up
**Goal**: Committed events drive eventually consistent read models through checkpointed projectors that can restart, rebuild, catch up, and optionally satisfy read-your-own-write queries.
**Depends on**: Phase 4
**Requirements**: PROJ-01, PROJ-02, PROJ-03, PROJ-04
**Success Criteria** (what must be TRUE):
  1. Projectors apply committed events to read models and persist projector offsets in the same transaction.
  2. Developer can query order summary and product inventory read models derived only from committed events.
  3. After restart, a projector resumes from its saved global-position checkpoint and catches up without duplicating read-model effects.
  4. Query callers can request a minimum global position to support read-your-own-write behavior without making projection completion part of command success.
**Plans**: 3 plans
Plans:
- [x] 05-01-PLAN.md — Create projection contracts, validated checkpoints, catch-up outcomes, and bounded minimum-position wait policy.
- [x] 05-02-PLAN.md — Add serde-backed commerce event payload support for order summary and product inventory projections.
- [x] 05-03-PLAN.md — Implement PostgreSQL projection schema, atomic catch-up, read-model queries, restart/idempotence tests, and read-your-own-write support.

### Phase 6: Outbox and Process Manager Workflows
**Goal**: Integration events and cross-entity workflows are driven from committed events through durable outbox rows and process managers, keeping broker publication and workflow follow-ups off the hot command path.
**Depends on**: Phase 5
**Requirements**: INT-01, INT-02, INT-03, INT-04
**Success Criteria** (what must be TRUE):
  1. Event append transactions can create outbox rows derived from committed domain events in the same durable commit.
  2. A dispatcher can publish pending outbox rows through a publisher trait and mark successful rows as published.
  3. Dispatcher retries are idempotent by source event and topic, so repeated attempts do not create duplicate external effects.
  4. A process manager reacts to order/product events and issues follow-up commands through the same command gateway without distributed transactions.
**Plans**: 5 plans
Plans:
- [x] 06-01-PLAN.md — Define storage-neutral outbox contracts and publisher idempotency.
- [x] 06-02-PLAN.md — Add PostgreSQL outbox schema, repository, and process-manager offsets.
- [x] 06-03-PLAN.md — Insert derived outbox rows inside append transactions.
- [x] 06-04-PLAN.md — Dispatch pending outbox rows with idempotent retry.
- [x] 06-05-PLAN.md — Implement the commerce process-manager workflow through command gateways.

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
**Plans**: 7 plans
Plans:
- [x] 07-01-PLAN.md — Implement the thin Axum HTTP command adapter, response/error contract, dependency catalog, and adapter boundary tests.
- [x] 07-02-PLAN.md — Add app observability bootstrap plus runtime, storage, projection, and outbox trace/metric instrumentation.
- [x] 07-03-PLAN.md — Add real PostgreSQL Phase 7 integration coverage for append, conflicts, dedupe, snapshots, projections, and outbox dispatch.
- [x] 07-04-PLAN.md — Create layer-separated benchmark artifacts for ring, domain, adapter, storage, projector/outbox, and required scenario smoke coverage.
- [x] 07-05-PLAN.md — Implement the single-service integrated stress runner and stress-smoke app bootstrap.
- [x] 07-06-PLAN.md — Document hot-path rules, gateway boundaries, template extension steps, and stress-result interpretation.
- [x] 07-07-PLAN.md — Close projection lag and stress signal verifier gaps with durable backlog metrics and measured stress fields.

### Phase 8: Runtime Duplicate Command Replay
**Goal**: Repeated commands with the same tenant and idempotency key are detected before aggregate decision and return the original committed result, preserving duplicate retry behavior across HTTP, runtime, storage, and process-manager replay paths.
**Depends on**: Phase 7
**Requirements**: STORE-03, RUNTIME-03, RUNTIME-05, INT-04, API-01, API-03
**Gap Closure**: Closes gaps from `.planning/v1.0-MILESTONE-AUDIT.md` for runtime/store idempotency integration and duplicate retry flows.
**Success Criteria** (what must be TRUE):
  1. Shard command processing checks shard-local idempotency before rehydrating aggregate state or calling domain `decide`.
  2. Duplicate commands return the original committed success/error reply shape without appending events or surfacing fresh domain validation errors.
  3. Durable store dedupe remains the source of truth when the runtime cache misses, after restart, or across process-manager replay scenarios.
  4. HTTP duplicate retries and deterministic process-manager follow-up retries are covered by tests that prove original committed results are replayed.
**Plans**: 3 plans
Plans:
- [x] 08-01-PLAN.md — Create durable typed command replay payload persistence and lookup in PostgreSQL command dedupe.
- [x] 08-02-PLAN.md — Add runtime pre-decision duplicate replay from shard-local and durable idempotency records.
- [x] 08-03-PLAN.md — Prove HTTP duplicate retry and process-manager follow-up retry replay original committed outcomes.

### Phase 9: Tenant-Scoped Runtime Aggregate Cache
**Goal**: Shard-owned runtime aggregate state remains isolated by tenant as well as stream, so cache hits cannot bypass tenant-scoped rehydration or evaluate commands against another tenant's state.
**Depends on**: Phase 8
**Requirements**: STORE-04, RUNTIME-03, RUNTIME-05, RUNTIME-06, DOM-05
**Gap Closure**: Closes the `es-runtime::AggregateCache` to tenant-scoped event-store rehydration gap from `.planning/v1.0-MILESTONE-AUDIT.md`.
**Success Criteria** (what must be TRUE):
  1. Aggregate cache entries are keyed by tenant identity plus stream ID, while remaining shard-owned and free of global mutable business-state locks.
  2. Runtime command processing cannot reuse cached aggregate state across tenants that share a stream ID.
  3. Store rehydration remains tenant-scoped and is not skipped by a stream-only cache hit.
  4. Regression tests prove same-stream, different-tenant commands preserve isolated domain state and conflict behavior.
**Plans**: 1 plan
Plans:
- [x] 09-01-PLAN.md — Add tenant-scoped aggregate cache keys, shard runtime isolation, same-stream tenant regressions, and validation sign-off.

### Phase 10: Duplicate-Safe Process Manager Follow-Up Keys
**Goal**: Process-manager follow-up commands remain deterministic for retry replay while avoiding idempotency collisions for orders that contain repeated product lines.
**Depends on**: Phase 9
**Requirements**: STORE-03, RUNTIME-05, DOM-04, DOM-05, INT-04
**Gap Closure**: Closes the commerce process-manager to runtime/store idempotency replay gap from `.planning/v1.0-MILESTONE-AUDIT.md`.
**Success Criteria** (what must be TRUE):
  1. Reserve and release follow-up idempotency keys distinguish duplicate product lines or otherwise coalesce them before command emission.
  2. True process-manager retries still replay original committed follow-up outcomes through runtime/store idempotency.
  3. Duplicate same-product order lines cannot collapse distinct reserve/release commands into the wrong replay record.
  4. App-level process-manager tests cover duplicate product lines and replayed follow-up processing.
**Plans**: TBD

### Phase 11: v1 Archive Hygiene and HTTP E2E Debt
**Goal**: Clean up non-blocking milestone audit debt so v1 can be archived with clear runnable service guidance, HTTP-inclusive stress coverage, current requirement traceability, validation hygiene, and advisory domain hardening.
**Depends on**: Phase 10
**Requirements**: API-02, API-04, OBS-01, TEST-03, TEST-04, DOC-01
**Gap Closure**: Addresses tech debt noted in `.planning/v1.0-MILESTONE-AUDIT.md` after the correctness blockers are closed.
**Success Criteria** (what must be TRUE):
  1. The app crate exposes a runnable HTTP service entrypoint or explicitly documents the accepted reason it remains out of scope.
  2. Full E2E stress coverage exercises HTTP DTO decode, router, and error mapping, or records an explicit accepted-debt decision.
  3. `REQUIREMENTS.md` checkboxes and traceability reflect current verification evidence for adapter boundaries, gateway docs, observability, benchmark coverage, and template documentation.
  4. Validation hygiene for partial Nyquist phases is resolved or explicitly routed to accepted follow-up work.
  5. Advisory aggregate lifecycle command ID hardening is implemented or recorded as accepted debt with rationale.
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Workspace and Typed Kernel Contracts | 4/4 | Complete | 2026-04-16 |
| 2. Durable Event Store Source of Truth | 4/4 | Complete | 2026-04-17 |
| 3. Local Command Runtime and Disruptor Execution | 4/4 | Complete | 2026-04-17 |
| 4. Commerce Fixture Domain | 4/4 | Complete | 2026-04-17 |
| 5. CQRS Projection and Query Catch-Up | 3/3 | Complete | 2026-04-18 |
| 6. Outbox and Process Manager Workflows | 5/5 | Complete | 2026-04-18 |
| 7. Adapters, Observability, Stress, and Template Guidance | 7/7 | Complete | 2026-04-19 |
| 8. Runtime Duplicate Command Replay | 3/3 | Complete | 2026-04-19 |
| 9. Tenant-Scoped Runtime Aggregate Cache | 1/1 | Complete | 2026-04-19 |
| 10. Duplicate-Safe Process Manager Follow-Up Keys | 0/TBD | Pending | - |
| 11. v1 Archive Hygiene and HTTP E2E Debt | 0/TBD | Pending | - |

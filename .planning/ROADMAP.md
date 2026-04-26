# Roadmap: Disruptor Event Sourcing Template

## Overview

This roadmap delivers a Rust service template where committed events are the source of truth and `disruptor-rs` is only the in-process ordered execution engine. The phases move from reusable contracts, to durable append, to local single-owner command execution, then prove the architecture with a compact commerce domain, CQRS, outbox/process-manager workflows, thin adapters, observability, and stress coverage. Distributed partition ownership is intentionally v2/out of scope for this milestone.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

**Milestone closure policy:** milestone-critical gaps do not become accepted debt. If runnable HTTP composition, external-process HTTP verification, validation hygiene, or domain hardening turns out to be required for v1 acceptance, the roadmap expands with additional phases or reopens earlier phase artifacts until the gap is actually closed.

- [x] **Phase 1: Workspace and Typed Kernel Contracts** - Developers can build the Rust workspace and define deterministic typed aggregate contracts. Completed 2026-04-16.
- [x] **Phase 2: Durable Event Store Source of Truth** - Commands can persist committed events, metadata, dedupe records, snapshots, and global reads through the event store boundary. Completed 2026-04-17.
- [x] **Phase 3: Local Command Runtime and Disruptor Execution** - Adapter requests flow through bounded local shards that own hot state and reply only after durable append. Completed 2026-04-17.
- [x] **Phase 4: Commerce Fixture Domain** - User, product, and order behavior proves typed decisions, replay, relationships, and invariants. Completed 2026-04-17.
- [x] **Phase 5: CQRS Projection and Query Catch-Up** - Committed events feed checkpointed read models with restart and read-your-own-write support. Completed 2026-04-18.
- [x] **Phase 6: Outbox and Process Manager Workflows** - Committed events create durable integration rows and cross-entity workflows without distributed transactions. Completed 2026-04-18.
- [x] **Phase 7: Adapters, Observability, Stress, and Template Guidance** - Thin APIs, metrics, integration tests, single-service stress tests, benchmarks, and documentation make the template credible. Completed 2026-04-19.
- [x] **Phase 8: Runtime Duplicate Command Replay** - Duplicate command retries are replayed from runtime/store idempotency before aggregate decision so API and process-manager retries return the original committed result. Completed 2026-04-19.
- [x] **Phase 9: Tenant-Scoped Runtime Aggregate Cache** - Shard-local aggregate cache entries include tenant identity so runtime hot state cannot bleed across tenant-scoped event-store streams. Completed 2026-04-19.
- [x] **Phase 10: Duplicate-Safe Process Manager Follow-Up Keys** - Process-manager reserve/release follow-up commands use collision-safe idempotency keys for duplicate product lines while preserving retry replay. Completed 2026-04-20.
- [x] **Phase 11: Evidence Recovery and Runnable HTTP Service** - Restore the archive evidence chain, add the official `app serve` runtime entrypoint, and document how the real HTTP composition is started and smoke-tested. Completed 2026-04-21.
- [x] **Phase 12: External-Process HTTP E2E, Stress, and Benchmark Closure** - Replace shortcut “full E2E” paths with external-process HTTP workloads that exercise the real serving path for end-to-end tests, stress runs, and benchmark baselines. (completed 2026-04-25)
- [x] **Phase 13: Live External-Process HTTP Steady-State Stress Testing** - Add a long-lived `app serve` HTTP stress lane that separates startup cost from steady-state request latency and throughput so live-service performance can be estimated from sustained load. (completed 2026-04-26)
- [ ] **Phase 13.1: Disruptor Throughput Bottleneck Investigation and Runtime Stress Optimization (INSERTED)** - Investigate why Phase 13 measured throughput is far below expected disruptor-style high-throughput behavior, isolate implementation and stress-harness bottlenecks, and improve the runtime or test path before archive sign-off. (reopened 2026-04-27 for verification gap closure)
- [ ] **Phase 14: Milestone Debt Closure and Archive Sign-Off** - Close every remaining milestone-critical validation and hardening gap, reopen earlier phase artifacts when needed, and rerun the final audit before v1 archive.

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
**Plans**: 1 plan
Plans:
- [x] 10-01-PLAN.md — Add line-aware reserve/release follow-up keys and duplicate-line retry replay coverage.

### Phase 11: Evidence Recovery and Runnable HTTP Service
**Goal**: Rebuild the milestone evidence chain and make the HTTP adapter composition actually runnable so later external-process tests, stress runs, and benchmarks target an official `app serve` path instead of a route-only stub.
**Depends on**: Phase 10
**Requirements**: API-02, API-04, OBS-01, DOC-01, TEST-04
**Gap Closure**: Closes the missing Phase 10 verification artifact, stale archive traceability, and missing runnable HTTP server entrypoint from `.planning/v1.0-MILESTONE-AUDIT.md`.
**Success Criteria** (what must be TRUE):
  1. Phase 10 gains a formal `10-VERIFICATION.md` artifact and the milestone evidence chain is consistent across roadmap, state, validation, verification, and summaries.
  2. `REQUIREMENTS.md` traceability and checkboxes are reconciled with verified Phase 7/10 evidence for API-02, API-04, OBS-01, TEST-03, TEST-04, and DOC-01.
  3. The app crate exposes an official `serve` entrypoint that starts the real HTTP router with bind/config wiring suitable for smoke tests and later external-process workloads.
  4. Smoke coverage proves the server can be started and reached through the documented HTTP path without bypassing adapter composition.
  5. Documentation explains how to launch, verify, and reuse the runnable HTTP path for later E2E, stress, and benchmark phases.
**Plans**: 2 plans
Plans:
- [x] 11-01-PLAN.md — Restore the evidence chain by adding the missing Phase 10 verification artifact and reconciling archive traceability across REQUIREMENTS, ROADMAP, STATE, and milestone audit references.
- [x] 11-02-PLAN.md — Add the official `app serve` entrypoint, startup/config wiring, smoke verification, and runnable-service documentation for the real HTTP adapter path.

### Phase 12: External-Process HTTP E2E, Stress, and Benchmark Closure
**Goal**: Prove the actual serving path under external-process HTTP workloads so end-to-end tests, stress scenarios, and benchmark baselines measure the executable service rather than in-process shortcuts.
**Depends on**: Phase 11
**Requirements**: API-01, API-03, TEST-03, TEST-04, OBS-02
**Gap Closure**: Replaces the misleading `FullE2eInProcess` shortcut with external-process HTTP coverage that exercises DTO decode, router wiring, error mapping, and service-process overhead.
**Success Criteria** (what must be TRUE):
  1. Canonical E2E and stress workloads launch the real service process and drive it through an HTTP client instead of direct `CommandEnvelope` submission.
  2. The previous in-process shortcut path is renamed, demoted, or removed so archive evidence no longer treats it as the representative full-E2E scenario.
  3. External-process coverage measures throughput, p50/p95/p99 latency, queue depth, append latency, reject rate, and other required stress fields against the runnable HTTP path.
  4. Benchmark artifacts clearly distinguish in-process component microbenchmarks from external-process HTTP end-to-end baselines.
  5. Documentation explains how to run the external-process E2E/stress harness and how its results differ from single-process integrated and ring-only measurements.
**Plans**: 2 plans
Plans:
- [x] 12-01-PLAN.md — Build the external-process HTTP harness and canonical request scenarios for E2E verification through the real `app serve` entrypoint.
- [x] 12-02-PLAN.md — Replace misleading full-E2E naming, add external-process stress/benchmark coverage, and document how to interpret the resulting measurements.

### Phase 13: Live External-Process HTTP Steady-State Stress Testing
**Goal**: Add a live-service HTTP stress lane that starts `app serve` once, warms it up, drives sustained external HTTP load for a configurable duration/concurrency profile, and reports steady-state throughput, latency percentiles, error/reject rates, and resource/lag signals without including startup/container setup time in the measured loop.
**Depends on**: Phase 12
**Requirements**: TEST-03, TEST-04, OBS-02
**Gap Closure**: Closes the measurement gap where Phase 12 external-process benchmark numbers include container startup and service boot overhead, making them useful as smoke evidence but misleading as live steady-state performance estimates.
**Success Criteria** (what must be TRUE):
  1. Developer can run a documented external-process HTTP stress command that keeps one `app serve` process alive across warmup and measurement windows.
  2. The measured interval excludes PostgreSQL container startup, service process boot, migration, readiness probing, and benchmark harness compilation.
  3. Reports include sustained throughput, p50/p95/p99/max latency, success/error/reject counts, reject rate, append latency, ingress/shard depth, projection lag, outbox lag, CPU/core count, run duration, concurrency, and environment metadata.
  4. The stress lane supports at least smoke, baseline, burst, and hot-key style profiles without conflating them with Criterion microbenchmarks.
  5. Documentation explains how to interpret steady-state live HTTP results separately from Phase 12 external-process smoke benchmarks and in-process integrated stress.
**Plans**: 2 plans
Plans:
- [x] 13-01-PLAN.md — Add bounded steady-state live HTTP runner semantics with warmup/measurement separation and measured-window reporting.
- [x] 13-02-PLAN.md — Add configurable `app http-stress` CLI, keep Criterion secondary, and document Phase 13 steady-state interpretation.

### Phase 13.1: Disruptor Throughput Bottleneck Investigation and Runtime Stress Optimization (INSERTED)

**Goal:** Phase 13 live HTTP stress results showed unexpectedly low throughput for a runtime that should benefit from disruptor-style ordered execution. Identify whether the bottleneck is in the current runtime implementation, HTTP/stress harness, storage path, shard admission, disruptor integration, projection/outbox side effects, or measurement methodology, then make the highest-confidence improvements before final archive sign-off.
**Requirements**: RUNTIME-01, RUNTIME-02, RUNTIME-05, TEST-03, TEST-04, OBS-02
**Gap Closure**: Reopens the performance evidence path after Phase 13 because the measured steady-state throughput is too low to support the template's high-throughput disruptor claim without bottleneck analysis and targeted optimization.
**Success Criteria** (what must be TRUE):
  1. Phase 13 stress artifacts are reviewed and the dominant throughput limit is classified by layer: HTTP client/server, adapter admission, command routing, shard queueing, disruptor execution, aggregate decision, event-store append, projection/outbox work, or measurement configuration.
  2. Ring-only, runtime-only, storage-only, adapter-only, and live HTTP measurements are compared enough to prove where the low throughput is introduced rather than assuming the disruptor path is at fault.
  3. At least one concrete implementation or harness bottleneck is fixed when evidence shows it is suppressing throughput, or the phase documents why no safe code change is justified yet.
  4. Updated stress output includes before/after or baseline/comparison evidence with throughput, p50/p95/p99/max latency, reject/error counts, queue depth, append latency, and relevant resource metadata.
  5. Documentation explains the remaining performance ceiling and separates disruptor/ring capability from full-service throughput limits such as durable PostgreSQL append, HTTP overhead, or configured backpressure.
**Depends on:** Phase 13
**Plans:** 3/6 plans complete

Plans:
- [x] 13.1-01-PLAN.md — Refactor the runtime to run one worker per shard and add a safe `ExpectedRevision::NoStream` cold-cache fast path.
- [x] 13.1-02-PLAN.md — Fix the live HTTP harness so it can saturate offered load, model true hot-key traffic, and report trustworthy scrape diagnostics.
- [x] 13.1-03-PLAN.md — Add one repeatable layer-comparison script and update docs that explain the resulting throughput ceiling.
- [ ] 13.1-04-PLAN.md — Fix shutdown semantics so accepted-but-undispatched runtime commands resolve deterministically during `app serve` shutdown.
- [ ] 13.1-05-PLAN.md — Correct live HTTP report semantics so observed metrics, estimated fallbacks, and repeated-stream diagnostics are explicitly separated.
- [ ] 13.1-06-PLAN.md — Regenerate storage and baseline comparison evidence under the corrected semantics and document scrape-gated ceiling rules.

### Phase 14: Milestone Debt Closure and Archive Sign-Off

**Goal:** Finish every remaining milestone-critical validation and hardening task, even when that requires reopening prior phase artifacts, so v1 archive happens with no known goal-critical debt still parked outside the closure path.
**Requirements**: STORE-03, RUNTIME-05, DOM-04, DOM-05, INT-04, TEST-02
**Gap Closure**: Resolves partial Nyquist validation and any milestone-critical commerce lifecycle hardening still standing after the HTTP/evidence work is done.
**Depends on:** Phase 13.1
**Success Criteria** (what must be TRUE):
  1. Partial Nyquist phases 02, 04, 06, and 07 are fully closed by new validation evidence or by reopening and repairing their underlying phase artifacts.
  2. Commerce lifecycle command-ID hardening is implemented wherever milestone acceptance still depends on it, or the underlying risk is disproven by targeted verification so no open milestone debt remains.
  3. Any earlier phase documents or code paths that must be revisited for milestone closure are updated in-place rather than deferred as accepted debt.
  4. A refreshed milestone audit shows no archive blockers across runnable HTTP composition, external-process HTTP coverage, evidence traceability, validation hygiene, lifecycle hardening, or live steady-state HTTP performance evidence.
  5. The roadmap, requirements, state, and phase artifacts all agree that v1 is archive-ready.
**Plans:** 2 plans

Plans:
- [ ] 14-01-PLAN.md — Close partial Nyquist validation for Phases 02, 04, 06, and 07, reopening prior phase artifacts when validation gaps expose unfinished milestone work.
- [ ] 14-02-PLAN.md — Implement or conclusively verify commerce lifecycle command-ID hardening, rerun the milestone audit, and record archive sign-off with zero milestone-critical accepted debt.

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13 → 13.1 → 14

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
| 10. Duplicate-Safe Process Manager Follow-Up Keys | 1/1 | Complete | 2026-04-20 |
| 11. Evidence Recovery and Runnable HTTP Service | 2/2 | Complete | 2026-04-21 |
| 12. External-Process HTTP E2E, Stress, and Benchmark Closure | 2/2 | Complete   | 2026-04-25 |
| 13. Live External-Process HTTP Steady-State Stress Testing | 2/2 | Complete    | 2026-04-26 |
| 13.1. Disruptor Throughput Bottleneck Investigation and Runtime Stress Optimization | 3/3 | Complete   | 2026-04-26 |
| 14. Milestone Debt Closure and Archive Sign-Off | 0/2 | Pending | - |

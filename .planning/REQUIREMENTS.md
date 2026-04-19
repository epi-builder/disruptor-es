# Requirements: Disruptor Event Sourcing Template

**Defined:** 2026-04-16
**Core Value:** Provide a reusable, production-shaped Rust service template where committed events are the source of truth and `disruptor-rs` is used only as the in-process ordered execution engine.

## v1 Requirements

Requirements for the initial template release. Each maps to roadmap phases.

### Workspace and Contracts

- [x] **CORE-01**: Developer can create and build a Rust 2024 workspace with separate crates for core types, domain kernel, runtime, storage, projection, outbox, example domain, adapters, and app composition.
- [x] **CORE-02**: Developer can define typed commands, events, aggregate state, replies, and errors through a generic aggregate kernel trait.
- [x] **CORE-03**: Developer can derive stream IDs, partition keys, expected revisions, command metadata, and event metadata through reusable core types.
- [x] **CORE-04**: Domain decision logic is synchronous, deterministic, typed, and free of adapter, database, broker, and network dependencies.

### Event Store

- [x] **STORE-01**: Command handling can append domain events to a durable event store with per-stream optimistic concurrency.
- [x] **STORE-02**: Event store records include event ID, stream ID, stream revision, global position, command ID, causation ID, correlation ID, tenant ID, event type, schema version, payload, metadata, and recorded timestamp.
- [ ] **STORE-03**: Command deduplication returns the prior committed result for a repeated tenant/idempotency key.
- [x] **STORE-04**: Aggregate rehydration can load the latest snapshot and replay subsequent stream events.
- [x] **STORE-05**: Event store exposes global-position reads for projector and outbox catch-up.

### Command Runtime

- [x] **RUNTIME-01**: Adapter requests enter the command engine through bounded ingress with explicit overload behavior.
- [x] **RUNTIME-02**: Partition routing sends all commands for the same aggregate key to the same local shard owner.
- [x] **RUNTIME-03**: Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks.
- [x] **RUNTIME-04**: Shard runtime integrates the `disruptor` crate as the local command execution/fan-out mechanism.
- [x] **RUNTIME-05**: Command replies are sent only after durable event-store append commit succeeds.
- [x] **RUNTIME-06**: Optimistic concurrency conflicts are surfaced as typed retryable or conflict errors without corrupting shard-local cache.

### Example Domain

- [x] **DOM-01**: Example domain includes `User`, `Product`, and `Order` aggregates or entity models with explicit relationships.
- [x] **DOM-02**: User commands can register, activate/deactivate, and emit replayable user events.
- [x] **DOM-03**: Product commands can create products, adjust inventory, reserve inventory, and release inventory.
- [ ] **DOM-04**: Order commands can place, confirm, reject, and cancel orders referencing user and product identifiers.
- [x] **DOM-05**: Domain invariants prevent invalid orders, negative inventory, duplicate order placement, and operations against inactive users or unavailable products.

### Projection and Query

- [x] **PROJ-01**: Projector runtime applies committed events to read models and updates projector offsets in the same transaction.
- [x] **PROJ-02**: Example read models expose order summary and product inventory views derived from events.
- [x] **PROJ-03**: Projection runtime can catch up from a saved global-position checkpoint after restart.
- [x] **PROJ-04**: Query path can optionally wait for a minimum global position to support read-your-own-write behavior.

### Outbox and Process Managers

- [x] **INT-01**: Append transaction can create outbox rows derived from committed domain events.
- [x] **INT-02**: Outbox dispatcher publishes pending rows through a publisher trait and marks successful rows as published.
- [x] **INT-03**: Outbox dispatch is retryable and idempotent by source event and topic.
- [ ] **INT-04**: A process-manager example reacts to order/product events and issues follow-up commands through the same command gateway.

### Adapter and API

- [x] **API-01**: Thin HTTP adapter exposes command endpoints that decode requests, attach metadata, send through bounded ingress, and await command replies.
- [ ] **API-02**: Adapter code does not mutate aggregate state, projector state, or outbox state directly.
- [x] **API-03**: API responses include stream revision, global position, correlation ID, and typed success/error payloads.
- [ ] **API-04**: Project documentation explains how WebSocket or gRPC gateways should connect without sharing hot business state.

### Observability and Stress Testing

- [ ] **OBS-01**: Runtime emits structured traces with command ID, correlation ID, causation ID, tenant ID, stream ID, shard ID, and global position when available.
- [x] **OBS-02**: Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, OCC conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency.
- [x] **TEST-01**: Test suite verifies aggregate replay determinism and domain invariants with generated command sequences or equivalent coverage.
- [x] **TEST-02**: Integration tests verify event append, OCC conflicts, deduplication, snapshots, projector checkpoints, and outbox dispatch against a real or containerized PostgreSQL database.
- [ ] **TEST-03**: Benchmark harnesses separately measure ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded dependency scenarios.
- [ ] **TEST-04**: A single-service integrated stress test runs the production-shaped composition in one service process and reports throughput, p50/p95/p99 latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization under realistic traffic.
- [ ] **DOC-01**: Documentation states hot-path rules, forbidden patterns, service-boundary guidance, and how to create a new domain service from the template.

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Distributed Operation

- **DIST-01**: Runtime supports distributed partition ownership backed by etcd, Raft, Kubernetes leases, or another coordinator.
- **DIST-02**: Partition owner failover can transfer ordered keys to another node and recover from event store checkpoints.
- **DIST-03**: Internal gRPC command RPC supports adapter service to command service separation.
- **DIST-04**: Broker adapters include production-ready NATS, Kafka, or Redpanda publishers.

### Advanced Domains

- **ADV-01**: Exchange-style ordered matching engine fixture demonstrates strict symbol-level sequencing.
- **ADV-02**: Ledger/wallet fixture demonstrates account-level ordered debits, credits, and reservations.
- **ADV-03**: Multi-bounded-context saga example coordinates separate services.

### Operations

- **OPS-01**: Kubernetes manifests or Helm chart deploy adapter, command engine, projector, and outbox workers separately.
- **OPS-02**: Chaos test suite covers node restart, broker outage, database latency injection, projector poison event, and reconnect storm.
- **OPS-03**: Schema evolution tooling supports event upcasters and snapshot migration.

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Full commerce storefront | The commerce domain is a technical fixture, not the product. |
| Full exchange matching engine in v1 | Too much domain-specific complexity before the generic architecture is proven. |
| Distributed partition ownership in v1 | Requires separate HA design and recovery protocol after local single-owner correctness is validated. |
| Direct broker publish from command handlers | Violates outbox and durability boundaries. |
| Immediate read-model consistency as command success condition | CQRS projections are eventually consistent by design. |
| Generic JSON/reflection rule engine in hot path | Undermines typed Rust domain logic and disruptor performance assumptions. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 | Phase 1 | Complete |
| CORE-02 | Phase 1 | Complete |
| CORE-03 | Phase 1 | Complete |
| CORE-04 | Phase 1 | Complete |
| STORE-01 | Phase 2 | Complete |
| STORE-02 | Phase 2 | Complete |
| STORE-03 | Phase 10 | Pending |
| STORE-04 | Phase 9 | Complete |
| STORE-05 | Phase 2 | Complete |
| RUNTIME-01 | Phase 3 | Complete |
| RUNTIME-02 | Phase 3 | Complete |
| RUNTIME-03 | Phase 9 | Complete |
| RUNTIME-04 | Phase 3 | Complete |
| RUNTIME-05 | Phase 9, Phase 10 | Complete |
| RUNTIME-06 | Phase 9 | Complete |
| DOM-01 | Phase 4 | Complete |
| DOM-02 | Phase 4 | Complete |
| DOM-03 | Phase 4 | Complete |
| DOM-04 | Phase 10 | Pending |
| DOM-05 | Phase 9, Phase 10 | Complete |
| PROJ-01 | Phase 5 | Complete |
| PROJ-02 | Phase 5 | Complete |
| PROJ-03 | Phase 5 | Complete |
| PROJ-04 | Phase 5 | Complete |
| INT-01 | Phase 6 | Complete |
| INT-02 | Phase 6 | Complete |
| INT-03 | Phase 6 | Complete |
| INT-04 | Phase 10 | Pending |
| API-01 | Phase 8 | Complete |
| API-02 | Phase 11 | Pending |
| API-03 | Phase 8 | Complete |
| API-04 | Phase 11 | Pending |
| OBS-01 | Phase 11 | Pending |
| OBS-02 | Phase 7 | Complete |
| TEST-01 | Phase 4 | Complete |
| TEST-02 | Phase 7 | Complete |
| TEST-03 | Phase 11 | Pending |
| TEST-04 | Phase 11 | Pending |
| DOC-01 | Phase 11 | Pending |

**Coverage:**
- v1 requirements: 39 total
- Mapped to phases: 39
- Unmapped: 0

---
*Requirements defined: 2026-04-16*
*Last updated: 2026-04-20 after milestone audit gap closure phase creation*

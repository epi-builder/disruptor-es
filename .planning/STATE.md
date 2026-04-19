---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 07-07-PLAN.md
last_updated: "2026-04-19T13:37:49.392Z"
last_activity: 2026-04-19 -- Phase 08 planning complete
progress:
  total_phases: 8
  completed_phases: 7
  total_plans: 34
  completed_plans: 31
  percent: 91
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-18)

**Core value:** Provide a reusable, production-shaped Rust service template where committed events are the source of truth and `disruptor-rs` is used only as the in-process ordered execution engine.
**Current focus:** Phase 07 — adapters-observability-stress-and-template-guidance

## Current Position

Phase: 07 (adapters-observability-stress-and-template-guidance) — COMPLETE
Plan: 7 of 7
Status: Ready to execute
Last activity: 2026-04-19 -- Phase 08 planning complete

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**

- Total plans completed: 31
- Average duration: N/A
- Total execution time: 0.0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01 | 4 | - | - |
| 02 | 4 | - | - |
| 03 | 4 | - | - |
| 04 | 4 | - | - |
| 05 | 3 | - | - |
| 06 | 5 | - | - |
| 07 | 7 | - | - |

**Recent Trend:**

- Last 5 plans: None
- Trend: N/A

*Updated after each plan completion*
| Phase 02-durable-event-store-source-of-truth P01 | 443 | 3 tasks | 7 files |
| Phase 02-durable-event-store-source-of-truth P02 | 264 | 3 tasks | 5 files |
| Phase 02-durable-event-store-source-of-truth P03 | 415 | 2 tasks | 6 files |
| Phase 02-durable-event-store-source-of-truth P04 | 405 | 2 tasks | 8 files |
| Phase 03-local-command-runtime-and-disruptor-execution P01 | 11 min | 3 tasks | 10 files |
| Phase 04-commerce-fixture-domain P01 | 3min 27s | 2 tasks | 6 files |
| Phase 04-commerce-fixture-domain P02 | 4min 4s | 2 tasks | 2 files |
| Phase 04-commerce-fixture-domain P03 | 5min 5s | 2 tasks | 2 files |
| Phase 04-commerce-fixture-domain P04 | 4min 9s | 2 tasks | 3 files |
| Phase 05-cqrs-projection-and-query-catch-up P01 | 5min 30s | 3 tasks | 8 files |
| Phase 05-cqrs-projection-and-query-catch-up P02 | 3min | 2 tasks | 5 files |
| Phase 05-cqrs-projection-and-query-catch-up P03 | - | 3 tasks | 7 files |
| Phase 06-outbox-and-process-manager-workflows P01 | 4min 41s | 1 tasks | 7 files |
| Phase 06-outbox-and-process-manager-workflows P02 | 5min 43s | 1 tasks | 7 files |
| Phase 06-outbox-and-process-manager-workflows P03 | 4min 5s | 1 tasks | 4 files |
| Phase 06-outbox-and-process-manager-workflows P04 | 4min 31s | 1 tasks | 6 files |
| Phase 06-outbox-and-process-manager-workflows P05 | 8min 48s | 1 tasks | 10 files |
| Phase 07-adapters-observability-stress-and-template-guidance P07 | 10min 42s | 3 tasks | 5 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Phase 1]: Start with Rust 2024 workspace and typed kernel contracts before runtime/storage coupling.
- [Phase 2]: Event store append commit is the authoritative command success point.
- [Phase 3]: `disruptor-rs` is in-process execution fabric only; distributed partition ownership is v2/out of scope.
- [Phase 7]: Single-service integrated stress testing is required in addition to ring-only and full distributed/E2E benchmarks.
- [Phase 02]: Use SQLx 0.8.6 and Testcontainers 0.25.0/0.13.0 to stay compatible with the Rust 1.85 workspace floor.
- [Phase 02]: Use PostgreSQL identity global positions and Rust-supplied UUIDs; the migration does not use DB-side uuidv7 defaults.
- [Phase 02]: Connect to the local Testcontainers PostgreSQL instance with SSL disabled while preserving the postgres:18 test target.
- [Phase 02]: AppendRequest derives tenant ownership from CommandMetadata rather than accepting a separate append-level tenant field.
- [Phase 02]: PostgresEventStore exposes the storage method surface now while append/read SQL remains owned by Plans 03 and 04.
- [Phase 02]: Event IDs are generated in Rust through a small IdGenerator trait using UUIDv7.
- [Phase 02]: Use a transaction-scoped PostgreSQL advisory lock derived from tenant/idempotency key before stream or event writes.
- [Phase 02]: Store the full CommittedAppend JSON in command_dedup.response_payload so duplicate replies preserve exact event IDs and global positions.
- [Phase 02]: Keep append SQL in a private sql.rs helper while PostgresEventStore remains the public storage facade.
- [Phase 02]: Represent snapshot records with state_payload and metadata to match the PostgreSQL snapshots table and plan contract.
- [Phase 02]: Keep rehydration in storage as latest snapshot plus ordered StoredEvent rows; aggregate replay remains kernel/runtime responsibility.
- [Phase 02]: Validate negative global cursors and read limits before SQL execution instead of casting them into unsigned values.
- [Phase 03]: Expose runtime errors as typed variants for overload, unavailable, invalid capacity, conflicts, domain, codec, and store failures.
- [Phase 03]: Keep CommandOutcome tied to CommittedAppend so successful replies carry durable event-store positions instead of disruptor sequence state.
- [Phase 03]: Use a boxed-future RuntimeEventStore trait to test runtime behavior without PostgreSQL while preserving the Phase 2 PostgresEventStore boundary.
- [Phase 04-commerce-fixture-domain]: Keep commerce foundation dependency-light: only existing es-core, es-kernel, and thiserror dependencies are used.
- [Phase 04-commerce-fixture-domain]: Use validated domain newtypes for commerce IDs and positive u32 quantities before commands are built.
- [Phase 04-commerce-fixture-domain]: Split user, product, and order into separate compile-visible modules for later aggregate behavior plans.
- [Phase 04-commerce-fixture-domain]: User registration emits UserRegistered and leaves the lifecycle Inactive until ActivateUser is accepted.
- [Phase 04-commerce-fixture-domain]: User stream IDs and partition keys use the same user-{UserId} routing key for ordered single-owner execution.
- [Phase 04-commerce-fixture-domain]: User aggregate remains synchronous and dependency-light, with no storage, async runtime, adapter, or shared mutable state.
- [Phase 04-commerce-fixture-domain]: Order stores UserId, ProductId, SKU, quantity, and product availability assumptions, not UserState or ProductState objects.
- [Phase 04-commerce-fixture-domain]: PlaceOrder uses ExpectedRevision::NoStream; confirm, reject, and cancel use ExpectedRevision::Any.
- [Phase 04-commerce-fixture-domain]: Generated Phase 04 tests use plain proptest command sequences rather than adding proptest-state-machine.
- [Phase 05-cqrs-projection-and-query-catch-up]: Keep es-projection storage-neutral; PostgreSQL StoredEvent conversion remains in es-store-postgres.
- [Phase 05-cqrs-projection-and-query-catch-up]: Use typed constructors to reject invalid projector names, positions, batch limits, and wait policies before storage calls.
- [Phase 05-cqrs-projection-and-query-catch-up]: Minimum-position query waits are bounded by timeout and return ProjectionLag instead of blocking indefinitely.
- [Phase 05-cqrs-projection-and-query-catch-up]: PostgreSQL projection catch-up updates read models and tenant-scoped offsets in the same transaction, with explicit rollback on malformed payload failures.
- [Phase 06-outbox-and-process-manager-workflows]: Keep es-outbox storage-neutral with typed contracts, futures::BoxFuture, and no SQLx, broker, adapter, or disruptor runtime APIs.
- [Phase 06-outbox-and-process-manager-workflows]: Use deterministic outbox publisher idempotency keys in the form tenant_id:topic:source_event_id.
- [Phase 06-outbox-and-process-manager-workflows]: Separate pre-append PendingSourceEventRef from persisted SourceEventRef so global positions are only required after storage commit.
- [Phase 06-outbox-and-process-manager-workflows]: Use PostgreSQL row locking with FOR UPDATE SKIP LOCKED for concurrent dispatcher claims instead of in-memory locks.
- [Phase 06-outbox-and-process-manager-workflows]: Keep process-manager progress as tenant-scoped monotonic offsets using GREATEST on upsert.
- [Phase 06-outbox-and-process-manager-workflows]: Validate inserted outbox rows against the source event's tenant and global position before storing them.
- [Phase 06-outbox-and-process-manager-workflows]: Keep AppendRequest::new source-compatible by delegating to new_with_outbox with an empty outbox message list.
- [Phase 06-outbox-and-process-manager-workflows]: Validate pre-append outbox source event IDs against the events in the same append request before storage.
- [Phase 06-outbox-and-process-manager-workflows]: Insert outbox rows after event rows have committed transaction-local global positions and before command dedupe is recorded.
- [Phase 06-outbox-and-process-manager-workflows]: Use tenant_id from CommandMetadata for append-created outbox rows and ON CONFLICT (tenant_id, source_event_id, topic) DO NOTHING for late duplicate idempotency.
- [Phase 06-outbox-and-process-manager-workflows]: Keep dispatch orchestration in es-outbox storage-neutral; PostgreSQL implements the OutboxStore port instead of leaking SQLx into dispatcher code.
- [Phase 06-outbox-and-process-manager-workflows]: Count publisher failures from the storage retry outcome so rows exhausted at max attempts are reported as failed, not retried.
- [Phase 06-outbox-and-process-manager-workflows]: Use a fixed 30-second PostgreSQL claim lock in the OutboxStore adapter while preserving the repository's explicit lock-duration API.
- [Phase 06-outbox-and-process-manager-workflows]: Keep process-manager contracts in es-outbox storage-neutral; app composes es-outbox, es-runtime, and example-commerce to avoid crate dependency cycles.
- [Phase 06-outbox-and-process-manager-workflows]: Read process-manager batches from PostgresEventStore::read_global using the saved tenant-scoped offset before delegating to process_batch.
- [Phase 06-outbox-and-process-manager-workflows]: Use deterministic follow-up idempotency keys in the form pm:{process_manager}:{source_event_id}:{action}:{target_id}.
- [Phase 06-outbox-and-process-manager-workflows]: Advance durable process-manager offsets only after follow-up command replies complete or an event is intentionally skipped.
- [Phase 07]: Projection lag is computed from tenant-scoped durable event-store max global position rather than fetched batch size.
- [Phase 07]: Single-service stress append latency is recorded around RuntimeEventStore::append instead of command round-trip latency.
- [Phase 07]: Stress shard depth samples read-only runtime shard state without exposing mutable shard internals.

### Pending Todos

- Phase 07 verification gaps:
  - Projection lag metric underreports backlog; compute against tenant durable max global position and test backlog behavior.
  - Single-service stress report has synthetic append latency, shard depth, and projection lag fields; replace with measured values or change the report contract.

### Blockers/Concerns

None yet.

## Quick Tasks Completed

| Date | Quick Task | Summary |
|------|------------|---------|
| 2026-04-18 | 260418-state-progress-reconciliation | Reconciled STATE.md progress, stopped_at, and session continuity after Phase 05 completion. |
| 2026-04-18 | 260418-1pp-update-requirements-documentation-to-mar | Reconciled Phase 1 CORE requirement status in REQUIREMENTS.md with completed Phase 1 project and roadmap records. |

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| Distributed operation | Distributed partition ownership/failover via etcd, Raft, Kubernetes leases, or similar coordinator | v2/out of scope | Roadmap creation |

## Session Continuity

Last session: 2026-04-19T02:11:55.753Z
Stopped at: Completed 07-07-PLAN.md
Resume file: None

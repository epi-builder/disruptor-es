---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: verifying
stopped_at: Completed 09-01-PLAN.md
last_updated: "2026-04-19T21:17:13.614Z"
last_activity: 2026-04-19
progress:
  total_phases: 11
  completed_phases: 9
  total_plans: 35
  completed_plans: 35
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-19)

**Core value:** Provide a reusable, production-shaped Rust service template where committed events are the source of truth and `disruptor-rs` is used only as the in-process ordered execution engine.
**Current focus:** Phase 09 — tenant-scoped-runtime-aggregate-cache

## Current Position

Phase: 09 (tenant-scoped-runtime-aggregate-cache) — COMPLETE
Plan: 1 of 1
Status: Phase complete — ready for verification
Last activity: 2026-04-19

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**

- Total plans completed: 34
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
| 08 | 3 | - | - |
| 09 | 1 | - | - |

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
| Phase 08-runtime-duplicate-command-replay P01 | 6min | 2 tasks | 7 files |
| Phase 08-runtime-duplicate-command-replay P02 | 10min 5s | 2 tasks | 10 files |
| Phase 08-runtime-duplicate-command-replay P03 | 10min 35s | 3 tasks | 7 files |
| Phase 09-tenant-scoped-runtime-aggregate-cache P01 | 6min 29s | 3 tasks | 6 files |

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
- [Phase 08-runtime-duplicate-command-replay]: Store typed command replies as CommandReplayRecord { append, reply } in command_dedup.response_payload for new appends. — This preserves the original typed reply for restart/cache-miss replay without adding a table or re-running aggregate decide.
- [Phase 08-runtime-duplicate-command-replay]: Keep legacy CommittedAppend response_payload rows readable for append dedupe while typed replay lookup returns None for legacy rows. — This keeps existing duplicate append semantics stable while preventing typed replay consumers from treating legacy append-only rows as full command replies.
- [Phase 08-runtime-duplicate-command-replay]: Use tenant_id plus idempotency_key for durable replay lookup, matching command_dedup primary key scope. — This preserves tenant isolation and satisfies the Phase 08 replay threat model.
- [Phase 08-runtime-duplicate-command-replay]: Runtime duplicate replay now checks shard-local dedupe first, then durable tenant/idempotency lookup, before aggregate rehydration or decision.
- [Phase 08-runtime-duplicate-command-replay]: Runtime codecs own typed reply payload validation so stored replay records are decoded without calling aggregate decide.
- [Phase 08-runtime-duplicate-command-replay]: Duplicate append races require a durable CommandReplayRecord lookup and return a codec error when no typed replay row exists.
- [Phase 08-runtime-duplicate-command-replay]: HTTP duplicate retry coverage uses the real order CommandEngine and a test RuntimeEventStore instead of adapter-local idempotency or manual reply injection.
- [Phase 08-runtime-duplicate-command-replay]: Process-manager retry coverage reuses deterministic pm:{manager}:{source_event_id}:... keys through real product/order CommandEngines instead of process-manager-local dedupe state.
- [Phase 08-runtime-duplicate-command-replay]: Phase 08 validation is recorded as requirement-level sampling because each plan contributes cross-cutting replay coverage.
- [Phase 09-tenant-scoped-runtime-aggregate-cache]: Aggregate cache identity is a first-class AggregateCacheKey containing TenantId and StreamId. — Matches existing typed DedupeKey pattern and prevents stream-only cache hits from bypassing tenant-scoped rehydration.
- [Phase 09-tenant-scoped-runtime-aggregate-cache]: ShardState constructs one AggregateCacheKey after duplicate replay misses and reuses it for cache hit, rehydration fill, and committed cache replacement. — Preserves Phase 8 duplicate replay ordering while keeping cache identity stable throughout a handoff.

### Pending Todos

- Milestone gap closure routing:
  - Phase 09 completed tenant-scoped aggregate cache keys for non-duplicate commands that share a stream ID across tenants.
  - Phase 10 remains pending for process-manager reserve/release idempotency keys with duplicate product lines.

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

Last session: 2026-04-19T21:17:05.817Z
Stopped at: Completed 09-01-PLAN.md
Resume file: None

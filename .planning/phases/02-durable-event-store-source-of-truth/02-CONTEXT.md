# Phase 02: Durable Event Store Source of Truth - Context

**Gathered:** 2026-04-17
**Status:** Ready for planning

<domain>

## Phase Boundary

Phase 02 implements the durable PostgreSQL event-store source of truth for command success. It delivers stream optimistic concurrency, append-only event records with full metadata, durable command deduplication, snapshot storage and rehydration reads, and global-position event reads for later projectors and outbox workers.

This phase does not implement disruptor command execution, shard-local aggregate caches, HTTP/gRPC adapters, projector runtimes, outbox dispatchers, or commerce workflow behavior. Those later components must depend on committed event-store results, not replace them.

</domain>

<decisions>

## Implementation Decisions

### Event Store Backend

- **D-01:** Phase 02 uses PostgreSQL as the default and only v1 durable event-store backend.
- **D-02:** SQLite is excluded from v1 core storage. It may be reconsidered later as an optional single-process local development or demo backend after the PostgreSQL event-store contract is stable.
- **D-03:** Existing crates such as `postgres-es` may be used as references, but `es-store-postgres` is a project-owned implementation, not a wrapper around a generic CQRS framework.
- **D-04:** SQLite tests, mocks, or in-memory substitutes must not replace PostgreSQL acceptance tests for STORE-01 through STORE-05.

### Storage API Boundary

- **D-05:** `es-store-postgres` exposes storage-level APIs: append events, read stream events, read events by global position, save/load snapshots, and return committed append results.
- **D-06:** `es-store-postgres` does not execute aggregate `decide`, own shard-local aggregate caches, run disruptor processors, implement adapter behavior, or publish to brokers.
- **D-07:** Rehydration support should provide the latest snapshot plus subsequent stream events. Applying those events to typed aggregate state remains a kernel/runtime responsibility.

### Idempotency and Empty Appends

- **D-08:** Durable command deduplication is implemented in PostgreSQL with tenant-scoped idempotency keys.
- **D-09:** Repeated commands with the same tenant/idempotency key return the prior committed append result without appending duplicate events.
- **D-10:** New empty appends are rejected by the low-level store. No-op command replies are a later runtime concern, not a Phase 02 event-store append behavior.

### PostgreSQL Schema Policy

- **D-11:** PostgreSQL 18 is the default development and integration-test target because it is a sufficiently current, broadly usable baseline for this template.
- **D-12:** The schema should use PostgreSQL-native transaction semantics, `jsonb`, `timestamptz`, identity/global-position columns, unique constraints, indexes, `ON CONFLICT`, and `RETURNING`.
- **D-13:** Avoid unnecessary PostgreSQL-version lock-in where the project can stay portable without weakening the design.
- **D-14:** UUIDs are generated in Rust through a dedicated module/helper rather than depending on DB-side `uuidv7()` generation. This keeps event, command, correlation, and causation ID creation portable and testable.

### Testing Boundary

- **D-15:** STORE-01 through STORE-05 require real or containerized PostgreSQL integration tests.
- **D-16:** Integration tests must cover append success, stream revision conflicts, command dedupe replay, snapshot save/load, global-position reads, and transaction rollback behavior.
- **D-17:** Unit tests are appropriate only for pure Rust validation, serialization wrappers, model behavior, and error mapping that do not depend on database semantics.
- **D-18:** As working functionality is implemented, add end-to-end scripts wherever practical to exercise and verify the behavior through the real paths, not just isolated unit tests.

### the agent's Discretion

- The planner may choose exact Rust module names inside `crates/es-store-postgres` as long as the storage/API boundary stays clear.
- The planner may decide whether migration files are split by table group or kept in one initial migration.
- The planner may choose the exact shape of append request/result structs, provided they expose the metadata required by STORE-01 through STORE-05 and do not pull runtime execution concerns into storage.

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Scope and Requirements

- `.planning/PROJECT.md` - Project vision, event-store source-of-truth rule, disruptor durability boundary, outbox transaction rule, and testing constraints.
- `.planning/REQUIREMENTS.md` - STORE-01 through STORE-05 requirements for Phase 02 and TEST-02 guidance for PostgreSQL-backed verification.
- `.planning/ROADMAP.md` - Phase 02 goal, dependency on Phase 01, and success criteria.
- `.planning/STATE.md` - Current focus and accumulated project decisions.

### Prior Phase Context

- `.planning/phases/01-workspace-and-typed-kernel-contracts/01-CONTEXT.md` - Locked decisions for typed kernel contracts, crate boundaries, metadata, and deterministic aggregate behavior.

### Phase Research

- `.planning/phases/02-durable-event-store-source-of-truth/02-RESEARCH.md` - Prescriptive research for PostgreSQL/sqlx stack, schema patterns, event-store transactions, and pitfalls.

</canonical_refs>

<code_context>

## Existing Code Insights

### Reusable Assets

- `crates/es-core` - Provides `StreamId`, `PartitionKey`, `TenantId`, `StreamRevision`, `ExpectedRevision`, `CommandMetadata`, and `EventMetadata` that storage APIs should reuse rather than duplicate.
- `crates/es-kernel` - Provides the synchronous deterministic aggregate contract. Storage may support replay inputs but must not call aggregate `decide`.
- `crates/es-store-postgres` - Exists as a Phase 01 shell crate and is the implementation target for Phase 02.

### Established Patterns

- Lower-level crates should keep boundaries strict and avoid pulling runtime, adapter, broker, or domain workflow concerns into core/storage contracts.
- Domain decisions are typed and deterministic; durable storage should preserve typed metadata and serialized payloads without turning the hot path into a generic JSON/reflection framework.
- PostgreSQL-backed behavior must be verified against real database semantics rather than SQLite or mocks.

### Integration Points

- New storage code connects to `es-core` metadata and revision types.
- Future Phase 03 runtime will call storage append APIs and send command replies only after durable commit succeeds.
- Future Phase 05 projectors and Phase 06 outbox workers will read committed events by durable global position, not disruptor ring sequence.

</code_context>

<specifics>

## Specific Ideas

- `es-store-postgres` is not an existing open-source dependency in this project; it is the project-owned PostgreSQL storage crate.
- PostgreSQL is intentionally chosen over SQLite for v1 because this template needs production-shaped concurrency, transaction, projection, outbox, and stress-test behavior.
- Use a dedicated Rust ID generation module/helper for UUID creation so tests can control IDs and the schema does not require DB-side UUID generation.
- The storage API should keep this invariant clear: append success means at least one new event was durably committed; dedupe hit means a prior committed result was returned.
- E2E scripts should be added as functionality becomes runnable so the team can verify real behavior through the same paths users will exercise.

</specifics>

<deferred>

## Deferred Ideas

- Optional SQLite backend for single-process local development or demos - defer until after the PostgreSQL event-store trait and behavior contract are stable.

</deferred>

---

*Phase: 02-durable-event-store-source-of-truth*
*Context gathered: 2026-04-17*

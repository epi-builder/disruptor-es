# Phase 02: Durable Event Store Source of Truth - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-04-17
**Phase:** 02-durable-event-store-source-of-truth
**Areas discussed:** Event store backend, storage API boundary, idempotency and empty appends, PostgreSQL schema policy, testing boundary

---

## Event Store Backend

| Option | Description | Selected |
|--------|-------------|----------|
| Project-owned PostgreSQL implementation | Implement `es-store-postgres` directly in this workspace and use existing event-store crates only as references. | yes |
| External CQRS/event-store crate | Adopt a crate such as `postgres-es` and align with its framework contracts. | |
| SQLite default backend | Use SQLite as the v1 durable event store for local portability. | |

**User's choice:** Project-owned PostgreSQL implementation.
**Notes:** The user asked whether `es-store-postgres` is an existing open-source tool. It was clarified as this workspace's project-owned storage crate. A subagent compared PostgreSQL and SQLite and recommended PostgreSQL as the v1 default, with SQLite deferred as an optional later local/demo backend.

---

## Storage API Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Storage-level API | Expose append, stream reads, global-position reads, snapshot reads/writes, and committed append results. | yes |
| Aggregate/runtime API | Have storage execute aggregate decisions or own runtime/cache behavior. | |
| Agent discretion | Let planner choose API level without locking the boundary. | |

**User's choice:** Recommended storage-level API boundary.
**Notes:** Storage must remain the durable transaction boundary and must not absorb Phase 03 runtime or aggregate execution responsibilities.

---

## Idempotency and Empty Appends

| Option | Description | Selected |
|--------|-------------|----------|
| Reject new empty appends | Append success always means at least one new event was committed; no-op command replies are handled later by runtime. | yes |
| Allow empty append results | Let storage store command dedupe results without new events for no-op commands. | |
| Agent discretion | Let planner choose during implementation. | |

**User's choice:** Recommended policy: reject new empty appends in low-level storage.
**Notes:** Repeated commands should still return the prior committed result through durable dedupe. The rejection applies to new append requests with no events.

---

## PostgreSQL Schema Policy

| Option | Description | Selected |
|--------|-------------|----------|
| PostgreSQL 18 baseline with portable choices | Use a modern PostgreSQL baseline while avoiding unnecessary lock-in; generate UUIDs in Rust. | yes |
| PostgreSQL 18-specific features everywhere | Lean on DB-side features such as `uuidv7()` wherever available. | |
| Older-version compatibility first | Avoid newer PostgreSQL assumptions even where they simplify the template. | |

**User's choice:** PostgreSQL 18 or similarly current, widely usable version as the baseline, while avoiding unnecessary dependency on version-specific features.
**Notes:** The user specifically preferred UUID generation through a separate module/helper for portability.

---

## Testing Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| PostgreSQL integration tests plus E2E scripts | Use real/containerized PostgreSQL for storage acceptance and add E2E scripts for working features as they emerge. | yes |
| Unit tests and mocks mainly | Keep most testing outside real database behavior. | |
| SQLite substitute tests | Use SQLite to avoid PostgreSQL setup in storage acceptance tests. | |

**User's choice:** Recommended PostgreSQL integration testing, with additional E2E scripts for working functionality wherever practical.
**Notes:** SQLite or mocks must not replace PostgreSQL acceptance tests for STORE-01 through STORE-05. Unit tests remain useful for pure Rust model/error/serialization behavior.

---

## the agent's Discretion

- Exact Rust module names inside `crates/es-store-postgres`.
- Exact migration split strategy.
- Exact append request/result type shape, within the locked boundary and metadata requirements.

## Deferred Ideas

- Optional SQLite backend for single-process local development or demos after the PostgreSQL event-store contract is stable.

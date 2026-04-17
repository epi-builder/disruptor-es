# Phase 5: CQRS Projection and Query Catch-Up - Research

**Researched:** 2026-04-18  
**Domain:** Rust CQRS projections, PostgreSQL read models, checkpointed catch-up workers  
**Confidence:** HIGH

## Summary

Phase 5 should implement projection as an asynchronous query-side subsystem driven only by committed PostgreSQL events ordered by `global_position`. [VERIFIED: .planning/REQUIREMENTS.md; crates/es-store-postgres/migrations/20260417000000_event_store.sql; crates/es-store-postgres/src/event_store.rs] The command runtime must remain commit-gated on event-store append only; projection catch-up may provide read-your-own-write waiting, but projection completion must not become part of command success. [VERIFIED: .planning/PROJECT.md; .planning/ROADMAP.md]

Use PostgreSQL as both the source cursor store and the initial read-model store. [VERIFIED: Cargo.toml; crates/es-store-postgres/migrations/20260417000000_event_store.sql] A projector should read batches with `PostgresEventStore::read_global(tenant_id, after_position, limit)`, apply only events it understands, upsert denormalized rows, and update `projector_offsets` in the same SQLx transaction as the read-model changes. [VERIFIED: crates/es-store-postgres/src/event_store.rs; CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html]

**Primary recommendation:** Use a project-owned `es-projection` abstraction plus PostgreSQL SQLx implementation; do not adopt a generic Rust CQRS framework or make projections part of the disruptor hot path. [VERIFIED: cargo search cqrs; .planning/PROJECT.md]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Projector catch-up loop | API / Backend worker | Database / Storage | Worker owns polling, batching, decode, and retry behavior; PostgreSQL owns durable event order and offsets. [VERIFIED: crates/es-store-postgres/src/event_store.rs; CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions] |
| Read-model persistence | Database / Storage | API / Backend worker | Read-model rows and `projector_offsets` must commit atomically in PostgreSQL. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html] |
| Order summary query | API / Backend query service | Database / Storage | Query service reads DTO-shaped rows and may wait for `last_applied_global_position >= minimum_global_position`. [VERIFIED: .planning/REQUIREMENTS.md] |
| Product inventory query | API / Backend query service | Database / Storage | Query service reads denormalized inventory rows derived from product events. [VERIFIED: crates/example-commerce/src/product.rs] |
| Read-your-own-write wait | API / Backend query service | Projector worker | Query path may wait on read-model freshness with a timeout; command path must not wait for projection. [VERIFIED: .planning/PROJECT.md; .planning/REQUIREMENTS.md] |

## Project Constraints (from AGENTS.md / Project Docs)

- Prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: user-provided AGENTS.md]
- Rust-first service implementation around `disruptor-rs`. [VERIFIED: .planning/PROJECT.md]
- Event store is the source of truth; disruptor rings must never be treated as durable state. [VERIFIED: .planning/PROJECT.md]
- Same aggregate or ordered partition key must map to the same shard owner. [VERIFIED: .planning/PROJECT.md]
- Hot business state should be single-owner and processor-local where practical. [VERIFIED: .planning/PROJECT.md]
- External publication belongs to the outbox committed with domain events; Phase 5 must not implement broker publication. [VERIFIED: .planning/ROADMAP.md]
- Adapter, command engine, projection, and outbox concerns should be separable. [VERIFIED: .planning/PROJECT.md]
- Performance tests must separate ring-only, domain-only, adapter-only, full E2E, soak, and chaos scenarios. [VERIFIED: .planning/PROJECT.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROJ-01 | Projector runtime applies committed events to read models and updates projector offsets in the same transaction. | SQLx transactions and PostgreSQL upsert semantics support atomic read-model + offset commits. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html; CITED: https://www.postgresql.org/docs/18/sql-insert.html] |
| PROJ-02 | Example read models expose order summary and product inventory views derived from events. | Commerce events expose order lifecycle and product inventory event shapes needed for these projections. [VERIFIED: crates/example-commerce/src/order.rs; crates/example-commerce/src/product.rs] |
| PROJ-03 | Projection runtime can catch up from a saved global-position checkpoint after restart. | Existing event store exposes tenant-scoped ordered reads after `global_position`; checkpointing a position is the standard catch-up pattern. [VERIFIED: crates/es-store-postgres/src/event_store.rs; CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions] |
| PROJ-04 | Query path can optionally wait for a minimum global position. | Add bounded query-side waiting against read-model metadata/offset state; this preserves eventual consistency and avoids gating command success. [VERIFIED: .planning/REQUIREMENTS.md; CITED: https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs] |

</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust workspace | Edition 2024, `rust-version = "1.85"` | Projection crate and storage integration baseline | Existing workspace policy requires Rust 2024 and Rust 1.85. [VERIFIED: Cargo.toml] |
| `sqlx` | 0.8.6, published 2025-05-19; latest crate metadata shows 0.9.0-alpha.1 exists | PostgreSQL read-model transactions, migrations, typed row mapping | Existing workspace uses 0.8.6; stable SQLx supports explicit `begin`, `commit`, rollback-on-drop transactions. [VERIFIED: Cargo.toml; VERIFIED: cargo info sqlx; VERIFIED: crates.io API; CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html] |
| PostgreSQL | Test target image `postgres:18` | Durable event log, projector offsets, read models | Existing migrations use PostgreSQL identity global positions and tenant-scoped indexes. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs; crates/es-store-postgres/migrations/20260417000000_event_store.sql] |
| `tokio` | Workspace 1.52.0; registry latest observed 1.52.1 published 2026-04-16 | Async catch-up loop, timeouts, tests | Existing workspace uses Tokio features `rt-multi-thread`, `macros`, `time`, `sync`; Context7 confirms interval/timeout/test patterns. [VERIFIED: Cargo.toml; VERIFIED: cargo info tokio; VERIFIED: crates.io API; VERIFIED: Context7 CLI /tokio-rs/tokio] |
| `serde_json` | 1.0.149, published 2026-01-06 | Decode stored JSON payloads for projection handlers | Existing event store stores payload and metadata as JSONB `serde_json::Value`. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/src/models.rs; VERIFIED: crates.io API] |
| `thiserror` | 2.0.18, published 2026-01-18 | Typed projection/query errors | Workspace already standardizes typed error enums with `thiserror`. [VERIFIED: Cargo.toml; VERIFIED: crates.io API] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `testcontainers` | 0.25.0 | PostgreSQL integration tests | Use the existing harness style for projector checkpoint/read-model tests. [VERIFIED: Cargo.toml; crates/es-store-postgres/tests/common/mod.rs] |
| `testcontainers-modules` | 0.13.0 with `postgres` | Starts PostgreSQL 18 test container | Reuse rather than introducing a new DB harness. [VERIFIED: Cargo.toml; crates/es-store-postgres/tests/common/mod.rs] |
| `time` | pinned `=0.3.44` | `OffsetDateTime` on stored events and read-model timestamps | StoredEvent already exposes `recorded_at: OffsetDateTime`. [VERIFIED: Cargo.toml; crates/es-store-postgres/src/models.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Project-owned projection API | Generic crates from `cargo search cqrs` such as `cqrs`, `mini_cqrs_es`, `cqrs-rust-lib` | Do not use for Phase 5; this template already owns event-store append, dedupe, snapshots, disruptor runtime, and tenant global reads, so a generic framework would fight local boundaries. [VERIFIED: cargo search cqrs; .planning/PROJECT.md] |
| PostgreSQL read models | Separate Redis/Elasticsearch/document store | Do not add in Phase 5; the milestone needs restart/catch-up correctness before specialized query stores. [VERIFIED: .planning/ROADMAP.md] |
| Projection from disruptor sequence | In-memory ring sequence cursor | Forbidden; committed PostgreSQL events are the only source of truth. [VERIFIED: .planning/PROJECT.md] |

**Installation:**

```bash
# No new workspace dependency is required for the first implementation.
# Add es-core, serde, serde_json, tokio, time, and thiserror to crates/es-projection/Cargo.toml as path/workspace dependencies.
# Add es-projection and example-commerce to crates/es-store-postgres/Cargo.toml for the PostgreSQL implementation.
```

## Architecture Patterns

### System Architecture Diagram

```text
Committed append transaction
  -> PostgreSQL events table (tenant_id, global_position ordered)
  -> Projector worker polls read_global(after checkpoint)
  -> For each batch:
       decode event_type/schema_version/payload
       branch:
         product events -> product_inventory_read_models upsert
         order events   -> order_summary_read_models upsert
         unknown event  -> ignore or typed unsupported-event metric hook
       update projector_offsets(projector_name, tenant_id, last_global_position)
       commit same SQLx transaction
  -> Query service reads read model
       optional minimum_global_position?
         yes -> wait until offset/read-model freshness reaches position or timeout
         no  -> return current projected row
```

### Recommended Project Structure

```text
crates/es-projection/
├── src/lib.rs              # Public projection contracts and re-exports
├── src/error.rs            # ProjectionError and ProjectionResult
├── src/checkpoint.rs       # ProjectorName, ProjectorOffset, minimum-position query types
├── src/projector.rs        # Projector trait, batch runner, catch-up outcome
├── src/query.rs            # Query wait policy and freshness result contracts
└── tests/                  # Unit tests for idempotency and wait-policy logic

crates/es-store-postgres/
├── migrations/...          # Add projector_offsets and read-model tables
├── src/projection.rs       # PostgreSQL read-model projector repository
└── tests/projections.rs    # Containerized PostgreSQL integration coverage
```

### Pattern 1: Atomic Projection Batch

**What:** Apply all read-model changes for a batch and update the projector offset in one SQL transaction. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html]  
**When to use:** Every projector batch, including rebuild/catch-up. [CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions]  
**Example:**

```rust
// Source: SQLx transaction docs and existing PostgresEventStore::read_global.
let events = store.read_global(&tenant_id, offset.last_global_position, batch_size).await?;
if events.is_empty() {
    return Ok(CatchUpOutcome::Idle);
}

let mut tx = pool.begin().await?;
for event in &events {
    apply_order_summary(&mut tx, event).await?;
    apply_product_inventory(&mut tx, event).await?;
}
let last = events.last().expect("non-empty batch").global_position;
upsert_projector_offset(&mut tx, projector_name, &tenant_id, last).await?;
tx.commit().await?;
```

### Pattern 2: Idempotent Read-Model Upserts

**What:** Make each read-model row converge when an event is reprocessed by writing deterministic values and storing `last_applied_global_position`. [CITED: https://www.postgresql.org/docs/18/sql-insert.html]  
**When to use:** For order lifecycle rows and product inventory rows. [VERIFIED: crates/example-commerce/src/order.rs; crates/example-commerce/src/product.rs]  
**Example:**

```sql
-- Source: PostgreSQL INSERT ... ON CONFLICT docs.
INSERT INTO product_inventory_read_models (
    tenant_id, product_id, sku, name, available_quantity,
    reserved_quantity, last_applied_global_position
)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (tenant_id, product_id) DO UPDATE
SET sku = EXCLUDED.sku,
    name = EXCLUDED.name,
    available_quantity = EXCLUDED.available_quantity,
    reserved_quantity = EXCLUDED.reserved_quantity,
    last_applied_global_position = GREATEST(
        product_inventory_read_models.last_applied_global_position,
        EXCLUDED.last_applied_global_position
    );
```

### Pattern 3: Minimum Global Position Query Wait

**What:** Queries accept an optional `minimum_global_position` and bounded `timeout`; they poll/check offset freshness before returning a read model. [VERIFIED: .planning/REQUIREMENTS.md; VERIFIED: Context7 CLI /tokio-rs/tokio]  
**When to use:** Client has a command response with committed `global_position` and wants read-your-own-write without making command success depend on projection. [VERIFIED: .planning/PROJECT.md]  
**Example:**

```rust
// Source: Tokio timeout/sleep docs via Context7 CLI.
let deadline = tokio::time::Instant::now() + wait_policy.timeout;
loop {
    if repo.projector_position(projector_name, &tenant_id).await? >= minimum_global_position {
        return repo.order_summary(&tenant_id, &order_id).await;
    }
    if tokio::time::Instant::now() >= deadline {
        return Err(QueryError::ProjectionLag {
            required: minimum_global_position,
        });
    }
    tokio::time::sleep(wait_policy.poll_interval).await;
}
```

### Anti-Patterns to Avoid

- **Using disruptor sequence numbers for projection checkpoints:** Durable restart/catch-up must use PostgreSQL event `global_position`. [VERIFIED: .planning/PROJECT.md; crates/es-store-postgres/src/models.rs]
- **Updating read models outside the offset transaction:** A crash between row update and offset update causes duplicate effects or skipped events unless the operation is idempotent and atomic. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html; CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions]
- **Putting query DTOs in the domain crate:** Domain must stay free of SQLx/runtime/storage dependencies. [VERIFIED: .planning/PROJECT.md; crates/example-commerce/tests/dependency_boundaries.rs]
- **Treating stale read models as command conflicts:** Commands validate against aggregate state and event store, not query-side freshness. [VERIFIED: .planning/PROJECT.md]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Database transactions | Custom transaction guard | SQLx `Pool::begin`, `Transaction::commit`, rollback-on-drop | SQLx already models transaction lifecycle. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html] |
| Upsert/convergence | Manual select-then-insert/update | PostgreSQL `INSERT ... ON CONFLICT DO UPDATE` | PostgreSQL documents atomic insert-or-update behavior under concurrency. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] |
| Async wait loop timing | Busy spin or blocking thread sleep | Tokio `timeout`, `sleep`, `interval` | Tokio provides non-blocking timers for async runtimes. [VERIFIED: Context7 CLI /tokio-rs/tokio] |
| CQRS framework | Generic Rust CQRS framework | Project-owned `es-projection` traits | Existing storage/runtime/kernel boundaries are already customized and verified. [VERIFIED: .planning/STATE.md; cargo search cqrs] |
| Integration DB fake | SQLite-only projector tests | Existing Testcontainers PostgreSQL harness | PostgreSQL transaction/upsert behavior is part of the requirement. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs; CITED: https://www.postgresql.org/docs/18/sql-insert.html] |

**Key insight:** The hard part is not dispatching events; it is preserving the invariant that read-model effects and the projector checkpoint advance together. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html; CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions]

## Common Pitfalls

### Pitfall 1: Offset Advances Without Read-Model Effects

**What goes wrong:** The projector saves `last_global_position` but crashes before read-model changes commit. [ASSUMED]  
**Why it happens:** Offset and projection writes are split across transactions. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html]  
**How to avoid:** Require one transaction per batch and commit offset last inside the same transaction. [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html]  
**Warning signs:** Tests can delete/restart worker between applying events and offset update. [ASSUMED]

### Pitfall 2: Duplicate Effects On Restart

**What goes wrong:** Reprocessing the last event increments inventory twice or appends duplicate order lines. [ASSUMED]  
**Why it happens:** At-least-once projector processing needs deterministic/idempotent read-model writes. [CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions]  
**How to avoid:** Store `last_applied_global_position` and use upserts or recomputed values instead of blind increments where possible. [CITED: https://www.postgresql.org/docs/18/sql-insert.html]  
**Warning signs:** A test that runs the same batch twice changes row values after the first run. [ASSUMED]

### Pitfall 3: Cross-Tenant Checkpoint Leakage

**What goes wrong:** One tenant's checkpoint causes another tenant's events to be skipped. [ASSUMED]  
**Why it happens:** The existing global read API is tenant-scoped, so offsets must also be keyed by `(tenant_id, projector_name)`. [VERIFIED: crates/es-store-postgres/src/event_store.rs]  
**How to avoid:** Primary key `projector_offsets(tenant_id, projector_name)` and require tenant ID on every query/repository method. [VERIFIED: crates/es-store-postgres/migrations/20260417000000_event_store.sql]

### Pitfall 4: Query Wait Has No Deadline

**What goes wrong:** A request hangs indefinitely when a projector is stopped or lagging. [ASSUMED]  
**Why it happens:** Minimum-position waits are implemented as unbounded loops. [ASSUMED]  
**How to avoid:** Make wait policy explicit with a short timeout and return typed `ProjectionLag` when freshness is not reached. [VERIFIED: Context7 CLI /tokio-rs/tokio]

## Code Examples

### Checkpoint Table Shape

```sql
-- Source: existing PostgreSQL schema style plus EventStoreDB checkpoint guidance.
CREATE TABLE projector_offsets (
    tenant_id text NOT NULL,
    projector_name text NOT NULL CHECK (projector_name <> ''),
    last_global_position bigint NOT NULL CHECK (last_global_position >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, projector_name)
);
```

### Read-Model Table Shapes

```sql
-- Source: Phase 5 success criteria and commerce event fields.
CREATE TABLE order_summary_read_models (
    tenant_id text NOT NULL,
    order_id text NOT NULL,
    user_id text NOT NULL,
    status text NOT NULL,
    line_count integer NOT NULL CHECK (line_count >= 0),
    total_quantity integer NOT NULL CHECK (total_quantity >= 0),
    rejection_reason text NULL,
    last_applied_global_position bigint NOT NULL CHECK (last_applied_global_position >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, order_id)
);

CREATE TABLE product_inventory_read_models (
    tenant_id text NOT NULL,
    product_id text NOT NULL,
    sku text NOT NULL,
    name text NOT NULL,
    available_quantity integer NOT NULL CHECK (available_quantity >= 0),
    reserved_quantity integer NOT NULL CHECK (reserved_quantity >= 0),
    last_applied_global_position bigint NOT NULL CHECK (last_applied_global_position >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, product_id)
);
```

### Projector Contract

```rust
// Source: project-owned boundary recommendation from current crate layout.
// es-projection owns storage-neutral DTOs and traits. PostgreSQL-specific
// StoredEvent conversion and SQLx transaction ownership live in es-store-postgres.
pub struct ProjectionEvent {
    pub global_position: i64,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub tenant_id: TenantId,
}

pub trait Projector {
    fn name(&self) -> &'static str;

    fn handles(&self, event_type: &str, schema_version: i32) -> bool;

    fn apply<'a>(
        &'a self,
        event: &'a ProjectionEvent,
    ) -> Pin<Box<dyn Future<Output = ProjectionResult<()>> + Send + 'a>>;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Directly query write model after commands | Separate read DTOs/materialized views with eventual consistency | CQRS pattern is current Microsoft architecture guidance as of page crawl/open in 2026. [CITED: https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs] | Phase 5 should build DTO-shaped query rows, not expose aggregate internals. [VERIFIED: .planning/PROJECT.md] |
| Save only an in-memory worker cursor | Persist checkpoint position in a durable store | EventStoreDB catch-up subscription docs document persistent checkpoints. [CITED: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions] | Restart starts from saved `global_position`, not from zero or memory. |
| Use custom merge logic for insert/update | Use PostgreSQL `ON CONFLICT DO UPDATE` | PostgreSQL 18 docs define atomic upsert semantics. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] | Upserts should be the default for read models. |

**Deprecated/outdated:**
- Projection from ring sequence state is invalid for this project because rings are not durable. [VERIFIED: .planning/PROJECT.md]
- Generic framework-first CQRS is not the right current approach for this template because storage/runtime/domain contracts are already project-owned. [VERIFIED: .planning/STATE.md; cargo search cqrs]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Crash windows and duplicate effects are likely projector failure modes. | Common Pitfalls | Tests may under-cover restart idempotency. |
| A2 | Query wait should use a short bounded timeout rather than block indefinitely. | Architecture Patterns, Common Pitfalls | Product/API behavior may need user confirmation in Phase 7. |
| A3 | Order summaries do not need pricing totals because Phase 4 order lines contain product, SKU, quantity, and availability but no price. | Code Examples | If pricing is added later, read-model schema must migrate. |

## Open Questions — RESOLVED

1. **RESOLVED: Should Phase 5 add event payload codecs for commerce events?**
   - What we know: Stored events are JSONB `serde_json::Value`; Phase 4 commerce events currently do not derive serde. [VERIFIED: crates/es-store-postgres/src/models.rs; crates/example-commerce/src/order.rs; crates/example-commerce/src/product.rs]
   - Decision: Phase 5 adds serde derives and JSON round-trip tests for commerce event payload DTOs in `example-commerce`, then decodes those typed payloads in `es-store-postgres`.
   - Boundary: Phase 5 does not add a generic upcaster framework. Schema-version-specific projection handling remains explicit in the PostgreSQL projection implementation; broader upcaster tooling stays out of this phase.

2. **RESOLVED: Should one worker own multiple projectors or one projector own one worker?**
   - What we know: Phase 5 needs order and product projections; Phase 7 metrics/stress will later care about per-projector lag. [VERIFIED: .planning/ROADMAP.md]
   - Decision: Phase 5 implements one worker/batch runner that can run named projectors sequentially per tenant for this milestone, while storing offsets per `(tenant_id, projector_name)`.
   - Boundary: Distributed or multi-worker projector ownership is deferred to later phases; this milestone proves durable checkpointing, atomic read-model updates, restart catch-up, and bounded read-your-own-write queries.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust/Cargo | Build and tests | yes | `cargo 1.85.1` | none |
| Docker Desktop | Testcontainers PostgreSQL integration tests | yes | Docker 29.3.1, server available | none |
| PostgreSQL CLI `psql` | Manual DB inspection only | no | - | Use SQLx/Testcontainers tests |
| cargo-nextest | Optional faster test runner | no | - | Use `cargo test` |

**Missing dependencies with no fallback:**
- None. [VERIFIED: command availability probes on 2026-04-18]

**Missing dependencies with fallback:**
- `psql` is missing; not needed for automated validation. [VERIFIED: command availability probes on 2026-04-18]
- `cargo-nextest` is missing; use `cargo test`. [VERIFIED: command availability probes on 2026-04-18]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness with Tokio async tests and Testcontainers PostgreSQL. [VERIFIED: Cargo.toml; crates/es-store-postgres/tests/common/mod.rs] |
| Config file | none; workspace `Cargo.toml` centralizes dependencies/lints. [VERIFIED: Cargo.toml] |
| Quick run command | `cargo test -p es-projection -p es-store-postgres projections -- --nocapture` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| PROJ-01 | Read-model rows and projector offset commit atomically | integration | `cargo test -p es-store-postgres projections_offset_commits_with_read_models -- --nocapture` | no - Wave 0 |
| PROJ-02 | Order summary and product inventory derive only from committed events | integration | `cargo test -p es-store-postgres projections_build_commerce_read_models -- --nocapture` | no - Wave 0 |
| PROJ-03 | Restart resumes from checkpoint without duplicate effects | integration | `cargo test -p es-store-postgres projections_resume_without_duplicate_effects -- --nocapture` | no - Wave 0 |
| PROJ-04 | Query can wait for minimum global position and timeout on lag | unit + integration | `cargo test -p es-projection minimum_position -- --nocapture` | no - Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p es-projection` and targeted `cargo test -p es-store-postgres projections -- --nocapture`. [VERIFIED: existing workspace test style]
- **Per wave merge:** `cargo test --workspace`. [VERIFIED: Cargo.toml]
- **Phase gate:** Full suite green before `/gsd-verify-work`. [VERIFIED: .planning/config.json]

### Wave 0 Gaps

- [ ] `crates/es-projection/tests/minimum_position.rs` - covers PROJ-04.
- [ ] `crates/es-store-postgres/tests/projections.rs` - covers PROJ-01, PROJ-02, PROJ-03.
- [ ] `crates/es-store-postgres/src/projection.rs` - PostgreSQL repository for offsets and read models.
- [ ] Migration adding `projector_offsets`, `order_summary_read_models`, and `product_inventory_read_models`.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 5 has no adapter auth boundary. [VERIFIED: .planning/ROADMAP.md] |
| V3 Session Management | no | Phase 5 has no user session surface. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | yes | Tenant ID must scope event reads, offsets, and read-model queries. [VERIFIED: crates/es-store-postgres/src/event_store.rs] |
| V5 Input Validation | yes | Validate nonnegative global positions, nonempty projector names, positive limits, and tenant-scoped IDs. [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| V6 Cryptography | no | Phase 5 adds no cryptographic primitives. [VERIFIED: .planning/ROADMAP.md] |

### Known Threat Patterns for Rust/PostgreSQL Projection Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant data leakage | Information Disclosure | Include `tenant_id` in every offset/read-model primary key and query predicate. [VERIFIED: crates/es-store-postgres/migrations/20260417000000_event_store.sql] |
| SQL injection | Tampering | Use SQLx parameter binding, not string-built SQL. [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Unbounded wait resource exhaustion | Denial of Service | Bound minimum-position waits with timeout and polling interval. [VERIFIED: Context7 CLI /tokio-rs/tokio] |
| Malformed event payload crash loop | Denial of Service | Return typed projection error and avoid advancing offset on failed event. [ASSUMED] |

## Sources

### Primary (HIGH confidence)

- `Cargo.toml` - workspace versions, Rust edition/floor, dependencies.
- `.planning/REQUIREMENTS.md` - PROJ-01 through PROJ-04 requirements.
- `.planning/ROADMAP.md` - Phase 5 scope and Phase 6/7 exclusions.
- `.planning/PROJECT.md` - source-of-truth, projection, hot-path, and CQRS constraints.
- `crates/es-store-postgres/migrations/20260417000000_event_store.sql` - existing durable schema and event indexes.
- `crates/es-store-postgres/src/event_store.rs` - `read_global` API.
- `crates/es-store-postgres/src/models.rs` - `StoredEvent` model and JSON payload fields.
- `crates/example-commerce/src/order.rs` and `crates/example-commerce/src/product.rs` - event shapes for read models.
- Context7 CLI `/launchbadge/sqlx` - SQLx transaction examples.
- Context7 CLI `/tokio-rs/tokio` - Tokio sleep/timeout/interval/test examples.
- SQLx docs: https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html - transaction lifecycle.
- PostgreSQL 18 `INSERT`: https://www.postgresql.org/docs/18/sql-insert.html - `ON CONFLICT` semantics.
- PostgreSQL 18 isolation: https://www.postgresql.org/docs/18/transaction-iso.html - Read Committed behavior.
- Microsoft CQRS pattern: https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs - CQRS read-model consistency guidance.
- EventStoreDB/Kurrent catch-up subscriptions: https://docs.kurrent.io/clients/java/legacy/v5.4/subscriptions - checkpointing and catch-up concepts.
- crates.io API / `cargo info` - verified crate versions and publish timestamps for `sqlx`, `tokio`, `serde_json`, `thiserror`.

### Secondary (MEDIUM confidence)

- `cargo search cqrs --limit 10` - confirms Rust CQRS crates exist but does not establish fit for this project.

### Tertiary (LOW confidence)

- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - workspace versions, crate metadata, Context7, and official docs agree.
- Architecture: HIGH - local requirements and project constraints clearly require committed-event projection, PostgreSQL global positions, and eventual consistency.
- Pitfalls: MEDIUM - atomic checkpoint/idempotency risks are standard event-projection failure modes, but some failure-mode wording is assumed and should be validated by tests.

**Research date:** 2026-04-18  
**Valid until:** 2026-05-18

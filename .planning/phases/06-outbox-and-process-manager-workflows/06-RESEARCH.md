# Phase 6: Outbox and Process Manager Workflows - Research

**Researched:** 2026-04-18 [VERIFIED: `date +%F`]
**Domain:** Rust event sourcing integration outbox, PostgreSQL queue claiming, idempotent dispatch, and event-driven process managers [VERIFIED: .planning/ROADMAP.md] [VERIFIED: crates/es-store-postgres/src/sql.rs] [VERIFIED: crates/es-runtime/src/gateway.rs]
**Confidence:** HIGH for PostgreSQL/outbox dispatch and crate boundaries; MEDIUM for process-manager example details because the current commerce events require careful correlation through metadata and command replies. [VERIFIED: PostgreSQL 18 docs] [CITED: https://microservices.io/patterns/data/transactional-outbox] [VERIFIED: crates/example-commerce/src/product.rs]

## User Constraints

- Phase 6 must implement INT-01 through INT-04: append transaction outbox creation, dispatcher publishing and marking, retry/idempotency by source event and topic, and a process-manager example that issues follow-up commands through the existing command gateway. [VERIFIED: .planning/REQUIREMENTS.md]
- Integration events and workflow follow-ups must be driven from committed events, not from disruptor sequence state. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/PROJECT.md]
- External publication must flow through outbox rows committed in the same transaction as domain events. [VERIFIED: AGENTS.md] [CITED: https://microservices.io/patterns/data/transactional-outbox]
- No direct broker publish from command handlers is allowed. [VERIFIED: .planning/REQUIREMENTS.md]
- Adapter, command engine, projection, and outbox concerns must stay separable. [VERIFIED: AGENTS.md]
- Prefer existing project patterns over new abstractions; local patterns already use neutral contract crates plus PostgreSQL-specific repositories and integration tests. [VERIFIED: crates/es-projection/src/lib.rs] [VERIFIED: crates/es-store-postgres/src/projection.rs]
- Use `pnpm` for Node tooling and `uv` for Python tooling when needed. [VERIFIED: user-provided AGENTS.md]

## Project Constraints

- No `CLAUDE.md` file was present at the project root during research. [VERIFIED: `test -f CLAUDE.md` returned no content]
- No project-local `.claude/skills/` or `.agents/skills/` skill index files were found. [VERIFIED: `find .claude/skills .agents/skills -maxdepth 2 -name SKILL.md`]
- Root workspace uses Rust 2024, resolver 3, and `rust-version = "1.85"`. [VERIFIED: Cargo.toml]
- Workspace lints forbid unsafe code and warn on missing docs. [VERIFIED: Cargo.toml]
- Current PostgreSQL test harness starts `postgres:18`, disables SSL for the local container connection, runs `sqlx::migrate!("./migrations")`, and uses `testcontainers`/`testcontainers-modules`. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| INT-01 | Append transaction can create outbox rows derived from committed domain events. [VERIFIED: .planning/REQUIREMENTS.md] | Extend the existing `sql::append` transaction after `insert_event` and before `insert_dedupe_result`/`commit`; write `outbox_messages` rows with `source_event_id`, `source_global_position`, and topic in the same transaction. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://microservices.io/patterns/data/transactional-outbox] |
| INT-02 | Outbox dispatcher publishes pending rows through a publisher trait and marks successful rows as published. [VERIFIED: .planning/REQUIREMENTS.md] | Use a storage-neutral `Publisher` trait in `es-outbox`, a PostgreSQL repository in `es-store-postgres`, and `FOR UPDATE SKIP LOCKED` queue claims for concurrent dispatchers. [VERIFIED: crates/es-outbox/src/lib.rs] [CITED: https://www.postgresql.org/docs/18/sql-select.html] |
| INT-03 | Outbox dispatch is retryable and idempotent by source event and topic. [VERIFIED: .planning/REQUIREMENTS.md] | Add a unique constraint on `(tenant_id, source_event_id, topic)` and make publisher calls carry a deterministic message key/idempotency key derived from those fields. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] [CITED: https://microservices.io/patterns/data/transactional-outbox] |
| INT-04 | A process-manager example reacts to order/product events and issues follow-up commands through the same command gateway. [VERIFIED: .planning/REQUIREMENTS.md] | Consume committed stored events by global position, build `CommandEnvelope`s with deterministic idempotency keys and causation/correlation metadata, call `CommandGateway::try_submit`, await the one-shot reply, then advance a durable process-manager offset. [VERIFIED: crates/es-runtime/src/command.rs] [VERIFIED: crates/es-runtime/src/gateway.rs] |

</phase_requirements>

## Summary

Phase 6 should implement a transactional outbox, not a broker adapter. The established pattern is to store the outbound message in the same database transaction as the business state change, then let a separate relay publish those stored messages. [CITED: https://microservices.io/patterns/data/transactional-outbox] The current event store already has the right transaction boundary: `sql::append` begins a PostgreSQL transaction, acquires dedupe and stream locks, inserts events, stores dedupe response payload, and commits. [VERIFIED: crates/es-store-postgres/src/sql.rs] The planner should extend that path so outbox rows are inserted before commit and are absent on rollback or duplicate command replay. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://microservices.io/patterns/data/transactional-outbox]

The dispatcher should be an at-least-once relay with idempotency keys, not an exactly-once broker abstraction. The outbox pattern explicitly allows the relay to publish more than once if it crashes after publish and before marking the row sent, so consumers or publisher implementations must be idempotent. [CITED: https://microservices.io/patterns/data/transactional-outbox] PostgreSQL `FOR UPDATE SKIP LOCKED` is appropriate for a queue-like table with multiple workers because PostgreSQL documents that it can avoid lock contention for consumers of queue-like tables. [CITED: https://www.postgresql.org/docs/18/sql-select.html]

The process-manager example should be explicit about the boundary between event observation and command execution. Existing `ProductEvent::InventoryReserved` does not carry an `OrderId`, so the reliable fixture flow should react to `OrderPlaced`, issue `ProductCommand::ReserveInventory` through the product command gateway, await replies, and then issue `OrderCommand::ConfirmOrder` or `OrderCommand::RejectOrder` with deterministic idempotency keys. [VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/example-commerce/src/product.rs] This avoids distributed transactions by making every follow-up a normal command append and making replay/retry safe through command dedupe. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://microservices.io/patterns/data/saga.html]

**Primary recommendation:** Add storage-neutral outbox and process-manager contracts in `es-outbox`, implement PostgreSQL schema/repository/append integration in `es-store-postgres`, and build the commerce process-manager example as a durable event consumer that submits follow-up commands through existing `CommandGateway`s with deterministic idempotency keys. [VERIFIED: crates/es-outbox/src/lib.rs] [VERIFIED: crates/es-store-postgres/src/sql.rs] [VERIFIED: crates/es-runtime/src/gateway.rs]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Derive outbox rows from domain events | API / Backend storage layer | Database / Storage | Derivation happens during event append; durable rows live in PostgreSQL and must commit atomically with event rows. [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Store pending/published/failed outbox state | Database / Storage | API / Backend worker | PostgreSQL owns durable status, attempts, locks, and timestamps; Rust worker claims and transitions rows. [CITED: https://www.postgresql.org/docs/18/sql-select.html] |
| Publish messages | API / Backend worker | External broker boundary | `es-outbox::Publisher` hides broker details while dispatcher controls retry and marking. [VERIFIED: crates/es-outbox/src/lib.rs] |
| Process-manager event consumption | API / Backend worker | Database / Storage | Process manager reads committed events and persists offsets; it does not run in aggregate `decide` or adapter handlers. [VERIFIED: crates/es-store-postgres/src/event_store.rs] |
| Follow-up command execution | API / Backend command runtime | Database / Storage | Follow-ups must enter the same `CommandGateway`/append path so shard routing, dedupe, and commit-gated replies remain authoritative. [VERIFIED: crates/es-runtime/src/gateway.rs] [VERIFIED: crates/es-runtime/src/shard.rs] |
| Broker-specific delivery | External dependency | API / Backend worker | v1 should define the publisher trait and test publisher; production NATS/Kafka adapters are deferred. [VERIFIED: .planning/REQUIREMENTS.md] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust workspace | Edition 2024, `rust-version = "1.85"` | Type-safe domain/process-manager/outbox implementation | Existing workspace baseline and Rust-first project constraint. [VERIFIED: Cargo.toml] |
| PostgreSQL | Test target `postgres:18` | Durable event store, outbox messages, dispatcher locks, process-manager offsets | Existing storage harness already uses PostgreSQL 18; PostgreSQL documents `FOR UPDATE SKIP LOCKED` for queue-like consumers. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs] [CITED: https://www.postgresql.org/docs/18/sql-select.html] |
| `sqlx` | Workspace `0.8.6`; latest registry result is `0.9.0-alpha.1` | Async PostgreSQL access, transactions, migrations | Existing store uses SQLx and explicit SQL; `0.8.6` is stable while newest registry result is alpha. [VERIFIED: Cargo.toml] [VERIFIED: `cargo info sqlx`] |
| `tokio` | Workspace `1.52.0`, registry latest observed `1.52.1` | Async dispatcher loops, sleeps/backoff, command reply awaits | Existing runtime uses Tokio `mpsc` and oneshot channels; Tokio docs verify nonblocking `try_send` semantics. [VERIFIED: Cargo.toml] [VERIFIED: `cargo info tokio`] [CITED: docs.rs tokio via Context7] |
| `futures` | Workspace `0.3.32` | Boxed async trait methods without adding `async-trait` | Existing runtime `RuntimeEventStore` uses `BoxFuture`; reuse that pattern for `Publisher` and storage ports. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-runtime/src/store.rs] |
| `serde` / `serde_json` | `serde 1.0.228`, `serde_json 1.0.149` | Serialize integration payloads and row metadata | Existing event payloads and projection payloads already use JSON/serde. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-store-postgres/src/models.rs] |
| `uuid` | Workspace `1.23.0`, registry latest observed `1.23.1` | Outbox IDs, deterministic message IDs when needed, source event references | Existing event IDs are UUIDs; uuid crate supports serde and UUIDv7 features. [VERIFIED: Cargo.toml] [VERIFIED: `cargo info uuid`] [CITED: Context7 uuid docs] |
| `time` | Workspace pinned `=0.3.44`, registry latest observed `0.3.47` | `available_at`, `locked_until`, `published_at`, timestamps | Existing stored events and snapshots use `time::OffsetDateTime`. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-store-postgres/src/models.rs] |
| `thiserror` | `2.0.18` | Typed `OutboxError`/process-manager errors | Existing storage/runtime/domain crates use typed errors through `thiserror`. [VERIFIED: Cargo.toml] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tracing` | `0.1.44` | Dispatcher and process-manager spans/events | Add span fields for topic, source event ID, outbox row ID, attempt, tenant, and global position once Phase 7 observability consumes them. [VERIFIED: Cargo.toml] [VERIFIED: .planning/REQUIREMENTS.md] |
| `testcontainers` | `0.25.0` | PostgreSQL integration tests | Reuse the existing harness style for outbox schema, claim concurrency, retry, and process-manager offset tests. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| `testcontainers-modules` | `0.13.0` with `postgres` feature | PostgreSQL 18 test container | Required by the existing container helper. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Polling outbox relay | Transaction log tailing / CDC | CDC is a valid outbox relay pattern, but v1 already has explicit SQL, migrations, and no broker commitment; polling is simpler and directly testable. [CITED: https://microservices.io/patterns/data/transactional-outbox] [VERIFIED: .planning/REQUIREMENTS.md] |
| `futures::BoxFuture` traits | `async-trait 0.1.89` | `async-trait` is current, but adding it is unnecessary because the project already uses boxed futures for async traits. [VERIFIED: `cargo info async-trait`] [VERIFIED: crates/es-runtime/src/store.rs] |
| External workflow engine | Temporal/Conductor/custom engine | Out of scope for this Rust template phase; Phase 6 only needs an embedded process-manager example over committed events and command gateway. [VERIFIED: .planning/ROADMAP.md] |
| Direct broker clients | NATS/Kafka publisher dependencies | v1 requirement is a publisher trait and durable outbox; production broker adapters are deferred. [VERIFIED: .planning/REQUIREMENTS.md] |

**Installation:**

```bash
# Prefer reusing existing workspace dependencies.
# Likely es-outbox additions:
cargo add -p es-outbox es-core futures serde serde_json thiserror time uuid --workspace

# Likely es-store-postgres additions:
cargo add -p es-store-postgres es-outbox --path crates/es-outbox
```

**Version verification:**

```bash
cargo search sqlx --limit 1
cargo info sqlx
cargo search tokio --limit 1
cargo info tokio
cargo search uuid --limit 1
cargo info uuid
cargo search async-trait --limit 1
```

`sqlx 0.8.6` is the stable version in use while `0.9.0-alpha.1` is the newest registry result; keep `0.8.6` for this phase. [VERIFIED: Cargo.toml] [VERIFIED: `cargo info sqlx`] `tokio` currently resolves to a newer compatible patch than the manifest lower bound; no phase task should churn the manifest just to bump Tokio. [VERIFIED: Cargo.toml] [VERIFIED: `cargo info tokio`]

## Architecture Patterns

### System Architecture Diagram

```text
CommandGateway
  -> shard-owned aggregate decide/apply
  -> PostgresEventStore::append
       -> BEGIN
       -> command dedupe lock/check
       -> stream lock/version check
       -> INSERT events
       -> INSERT outbox_messages derived from inserted events
       -> INSERT command_dedup response
       -> COMMIT
  -> command reply after commit

OutboxDispatcher loop
  -> claim pending rows with FOR UPDATE SKIP LOCKED
  -> publish through Publisher(topic, key, payload, metadata)
  -> mark row published OR schedule retry/dead-letter

ProcessManager loop
  -> read committed events after saved PM offset
  -> for relevant order/product events
       -> create follow-up CommandEnvelope with deterministic idempotency key
       -> CommandGateway::try_submit
       -> await command reply
  -> update PM offset only after follow-ups finish or are intentionally skipped
```

### Recommended Project Structure

```text
crates/
├── es-outbox/
│   └── src/
│       ├── lib.rs              # facade and re-exports
│       ├── error.rs            # OutboxError / OutboxResult
│       ├── models.rs           # NewOutboxMessage, OutboxMessage, status, claim records
│       ├── publisher.rs        # Publisher trait, PublishEnvelope, in-memory test publisher
│       ├── dispatcher.rs       # storage-neutral dispatch loop over an OutboxStore port
│       └── process_manager.rs  # storage-neutral PM contracts and outcomes
├── es-store-postgres/
│   ├── migrations/
│   │   └── 20260418xxxxxx_outbox.sql
│   └── src/
│       ├── outbox.rs           # PostgresOutboxStore and PM offset storage
│       ├── sql.rs              # append transaction extension
│       └── event_store.rs      # public append API extension or new append_with_outbox
└── app/ or es-outbox tests/
    └── commerce process-manager composition with Order/Product gateways
```

### Pattern 1: Atomic Outbox Insert In Append Transaction

**What:** Insert outbox rows in the same PostgreSQL transaction that inserts event rows and command-dedup response payload. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://microservices.io/patterns/data/transactional-outbox]

**When to use:** Every event that should create an external integration message or workflow trigger should produce a `NewOutboxMessage` tied to the source event ID/topic before append commits. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: existing append shape in crates/es-store-postgres/src/sql.rs
// Source: https://microservices.io/patterns/data/transactional-outbox
let inserted = insert_event(&mut tx, &request, event, stream_revision).await?;
for outbox in request.outbox_messages_for(event.event_id) {
    insert_outbox_message(&mut tx, &request, &inserted, outbox).await?;
}
```

### Pattern 2: Queue Claim With `FOR UPDATE SKIP LOCKED`

**What:** Claim rows in a transaction by locking only pending rows due for dispatch and skipping rows another worker has locked. [CITED: https://www.postgresql.org/docs/18/sql-select.html]

**When to use:** Dispatcher workers can run concurrently without central coordination. [CITED: https://www.postgresql.org/docs/18/sql-select.html]

**Example:**

```sql
-- Source: https://www.postgresql.org/docs/18/sql-select.html
WITH claimed AS (
    SELECT outbox_id
    FROM outbox_messages
    WHERE status = 'pending'
      AND available_at <= now()
    ORDER BY source_global_position, outbox_id
    LIMIT $1
    FOR UPDATE SKIP LOCKED
)
UPDATE outbox_messages AS o
SET status = 'publishing',
    locked_by = $2,
    locked_until = now() + ($3::text)::interval,
    attempts = attempts + 1,
    updated_at = now()
FROM claimed
WHERE o.outbox_id = claimed.outbox_id
RETURNING o.*;
```

### Pattern 3: At-Least-Once Dispatch With Idempotent Publisher Contract

**What:** The dispatcher may retry the same row; the publisher receives a deterministic key such as `{tenant_id}:{topic}:{source_event_id}` and must make repeated publishes safe. [CITED: https://microservices.io/patterns/data/transactional-outbox]

**When to use:** Always for outbox rows; do not promise exactly-once external delivery from database status alone. [CITED: https://microservices.io/patterns/data/transactional-outbox]

**Example:**

```rust
// Source: existing BoxFuture trait style in crates/es-runtime/src/store.rs
pub trait Publisher: Clone + Send + Sync + 'static {
    fn publish(&self, message: PublishEnvelope) -> futures::future::BoxFuture<'_, OutboxResult<()>>;
}

impl OutboxMessage {
    pub fn idempotency_key(&self) -> String {
        format!("{}:{}:{}", self.tenant_id.as_str(), self.topic, self.source_event_id)
    }
}
```

### Pattern 4: Process Manager Offset After Follow-Up Completion

**What:** A process-manager worker reads committed events, submits follow-up commands, awaits replies, and advances its durable offset only after follow-ups have finished or been intentionally ignored. [VERIFIED: crates/es-runtime/src/command.rs] [VERIFIED: crates/es-runtime/src/gateway.rs]

**When to use:** Cross-aggregate workflows such as `OrderPlaced -> ReserveInventory -> ConfirmOrder/RejectOrder`. [VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/example-commerce/src/product.rs]

**Example:**

```rust
// Source: existing CommandEnvelope::new and CommandGateway::try_submit APIs.
let (reply, receiver) = tokio::sync::oneshot::channel();
let envelope = CommandEnvelope::<Product>::new(
    ProductCommand::ReserveInventory { product_id, quantity },
    metadata_with_causation(source_event.event_id, source_event.correlation_id),
    format!("pm:reserve:{}:{}", source_event.event_id, product_id.as_str()),
    reply,
)?;
product_gateway.try_submit(envelope)?;
let reserve_result = receiver.await.map_err(|_| OutboxError::CommandReplyDropped)??;
```

### Anti-Patterns To Avoid

- **Publishing inside aggregate `decide`:** Domain logic is synchronous and storage/network-free by project requirement. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: crates/es-kernel/src/lib.rs]
- **Publishing directly after append without a row:** A process crash between commit and publish loses the external message; the transactional outbox exists to avoid that double-write failure. [CITED: https://microservices.io/patterns/data/transactional-outbox]
- **Advancing PM offset before command replies:** A crash after offset advance and before command commit permanently skips the follow-up. [VERIFIED: crates/es-runtime/src/command.rs] [ASSUMED]
- **Using disruptor sequence as outbox cursor:** The project states disruptor rings are in-process execution only and not durable state. [VERIFIED: AGENTS.md]
- **Treating `InventoryReserved` as order-correlated:** Current product events contain product and quantity fields but no order ID; correlation must come from command metadata or the process-manager command reply flow. [VERIFIED: crates/example-commerce/src/product.rs]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Durable queue row claiming | In-memory mutex queue or custom lock table | PostgreSQL row locks with `FOR UPDATE SKIP LOCKED` | PostgreSQL documents this specifically for avoiding contention in queue-like consumers. [CITED: https://www.postgresql.org/docs/18/sql-select.html] |
| Upsert/idempotent inserts | Select-then-insert race checks | Unique constraints plus `INSERT ... ON CONFLICT` | PostgreSQL documents atomic insert/update behavior for `ON CONFLICT DO UPDATE` under concurrency. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] |
| Cross-transaction DB/broker atomicity | Two-phase commit or direct broker write in command handler | Transactional outbox row plus relay | The outbox pattern addresses atomic DB update plus later message relay without 2PC. [CITED: https://microservices.io/patterns/data/transactional-outbox] |
| Workflow runtime | Generic external workflow engine | Small event-driven process manager over existing command gateway | The phase is a local Rust template example, not a distributed workflow platform. [VERIFIED: .planning/ROADMAP.md] |
| Async trait machinery | New `async-trait` dependency | Existing `futures::BoxFuture` trait style | Existing runtime ports already use boxed futures, so the pattern is local and dependency-light. [VERIFIED: crates/es-runtime/src/store.rs] |
| Broker-specific exactly-once semantics | Custom exactly-once delivery protocol | Deterministic idempotency key by source event/topic and idempotent publisher contract | The outbox relay can publish more than once after crash; idempotency is required. [CITED: https://microservices.io/patterns/data/transactional-outbox] |

**Key insight:** The durable guarantees come from PostgreSQL transactions, unique constraints, and command idempotency, while external effects remain at-least-once unless the publisher/consumer honors the deterministic key. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://microservices.io/patterns/data/transactional-outbox]

## Common Pitfalls

### Pitfall 1: Outbox Rows Created After Commit

**What goes wrong:** Event append succeeds but a crash prevents external message creation. [CITED: https://microservices.io/patterns/data/transactional-outbox]
**Why it happens:** Developers treat publication as an after-commit side effect instead of data written in the append transaction. [CITED: https://microservices.io/patterns/data/transactional-outbox]
**How to avoid:** Insert `outbox_messages` before `tx.commit()` in `sql::append`. [VERIFIED: crates/es-store-postgres/src/sql.rs]
**Warning signs:** Code calls `publisher.publish` or writes outbox rows from runtime shard code after `store.append(...)` returns. [VERIFIED: crates/es-runtime/src/shard.rs]

### Pitfall 2: Claiming Without Lock Skip

**What goes wrong:** Multiple dispatchers block each other or publish the same pending row concurrently. [CITED: https://www.postgresql.org/docs/18/sql-select.html]
**Why it happens:** Workers select pending rows without row locks or status transitions. [ASSUMED]
**How to avoid:** Claim in a transaction using `FOR UPDATE SKIP LOCKED`, update status/attempts, commit, publish, then mark. [CITED: https://www.postgresql.org/docs/18/sql-select.html]
**Warning signs:** SQL uses `SELECT ... WHERE status = 'pending' LIMIT ...` without locking. [ASSUMED]

### Pitfall 3: Marking Published Before Publish Returns

**What goes wrong:** A row is marked published but the broker call failed or never happened. [ASSUMED]
**Why it happens:** Status updates are batched before side effects to reduce round trips. [ASSUMED]
**How to avoid:** Mark published only after `Publisher::publish` returns `Ok(())`; on failure, schedule retry with `available_at` and `last_error`. [CITED: https://microservices.io/patterns/data/transactional-outbox]
**Warning signs:** Dispatcher performs one transaction that both claims rows and sets `published_at` before the publisher call. [ASSUMED]

### Pitfall 4: Assuming Exactly-Once External Delivery

**What goes wrong:** A crash after publish and before `mark_published` causes the relay to publish the same row again. [CITED: https://microservices.io/patterns/data/transactional-outbox]
**Why it happens:** Database row state cannot observe a completed external publish if the process dies before recording it. [CITED: https://microservices.io/patterns/data/transactional-outbox]
**How to avoid:** Use deterministic keys and idempotent publisher/consumer behavior; tests should intentionally republish the same source event/topic and verify no duplicate external effect in the test publisher. [CITED: https://microservices.io/patterns/data/transactional-outbox]
**Warning signs:** Docs or tests claim "exactly once" without naming a broker idempotency mechanism. [ASSUMED]

### Pitfall 5: Process Manager Creates New Idempotency Keys On Retry

**What goes wrong:** Reprocessing the same source event appends duplicate reserve/confirm/reject commands. [VERIFIED: crates/es-store-postgres/src/sql.rs]
**Why it happens:** Follow-up command idempotency key includes a random UUID or current timestamp. [ASSUMED]
**How to avoid:** Derive keys from process-manager name, source event ID, target aggregate, and action. [VERIFIED: crates/es-store-postgres/src/models.rs] [ASSUMED]
**Warning signs:** Process-manager code calls `Uuid::now_v7()` for an idempotency key. [ASSUMED]

### Pitfall 6: Offsets Advance Across Partial Workflow Completion

**What goes wrong:** The process manager records that a source event is complete even though one of several follow-up commands failed or was overloaded. [ASSUMED]
**Why it happens:** Offset logic is copied from projections, but projections are pure DB updates while process-manager follow-ups call async command gateways. [VERIFIED: crates/es-store-postgres/src/projection.rs] [VERIFIED: crates/es-runtime/src/gateway.rs]
**How to avoid:** Advance PM offset after all required follow-up command replies are received, or persist per-step workflow state before offset advance. [CITED: https://microservices.io/patterns/data/saga.html] [ASSUMED]
**Warning signs:** PM offset updates appear before `receiver.await` for command replies. [VERIFIED: crates/es-runtime/src/command.rs] [ASSUMED]

## Code Examples

### Outbox Schema

```sql
-- Source: PostgreSQL 18 INSERT/SELECT docs and existing migration style.
CREATE TABLE outbox_messages (
    outbox_id uuid PRIMARY KEY,
    tenant_id text NOT NULL,
    source_event_id uuid NOT NULL,
    source_global_position bigint NOT NULL CHECK (source_global_position >= 1),
    topic text NOT NULL CHECK (topic <> ''),
    message_key text NOT NULL CHECK (message_key <> ''),
    payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    status text NOT NULL CHECK (status IN ('pending', 'publishing', 'published', 'failed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at timestamptz NOT NULL DEFAULT now(),
    locked_by text NULL,
    locked_until timestamptz NULL,
    published_at timestamptz NULL,
    last_error text NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, source_event_id, topic),
    FOREIGN KEY (source_event_id) REFERENCES events (event_id)
);

CREATE INDEX outbox_pending_idx
    ON outbox_messages (status, available_at, source_global_position);
```

### Outbox Publisher Port

```rust
// Source: crates/es-runtime/src/store.rs BoxFuture style
pub trait Publisher: Clone + Send + Sync + 'static {
    fn publish(&self, envelope: PublishEnvelope) -> futures::future::BoxFuture<'_, OutboxResult<()>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishEnvelope {
    pub topic: String,
    pub message_key: String,
    pub idempotency_key: String,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}
```

### Dispatcher Loop Shape

```rust
// Source: Tokio mpsc/time docs via Context7; local BoxFuture style.
pub async fn dispatch_once<S, P>(
    store: &S,
    publisher: &P,
    worker_id: &str,
    limit: DispatchBatchLimit,
) -> OutboxResult<DispatchOutcome>
where
    S: OutboxStore,
    P: Publisher,
{
    let claimed = store.claim_pending(worker_id, limit).await?;
    if claimed.is_empty() {
        return Ok(DispatchOutcome::Idle);
    }

    let mut published = 0;
    for message in claimed {
        let result = publisher.publish(message.publish_envelope()).await;
        match result {
            Ok(()) => {
                store.mark_published(message.outbox_id).await?;
                published += 1;
            }
            Err(error) => {
                store.schedule_retry(message.outbox_id, error.to_string()).await?;
            }
        }
    }

    Ok(DispatchOutcome::Published { published })
}
```

### Process Manager Follow-Up

```rust
// Source: crates/es-runtime/src/command.rs and crates/es-runtime/src/gateway.rs
let (reply, receiver) = tokio::sync::oneshot::channel();
let idempotency_key = format!(
    "pm:commerce:{}:reserve:{}",
    source.event_id,
    product_id.as_str()
);

let envelope = CommandEnvelope::<Product>::new(
    ProductCommand::ReserveInventory { product_id, quantity },
    command_metadata_for_follow_up(&source),
    idempotency_key,
    reply,
)?;

product_gateway.try_submit(envelope)?;
let outcome = receiver.await.map_err(|_| OutboxError::CommandReplyDropped)??;
```

## State Of The Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct publish in command handler | Transactional outbox row plus relay | Established microservices pattern; current official page crawled 2026-04-16 by browser result | Prevents DB/broker double-write gaps. [CITED: https://microservices.io/patterns/data/transactional-outbox] |
| One worker polling all rows without locks | Multiple workers using row locks and `SKIP LOCKED` | PostgreSQL 18 docs current page dated 2026 docs set | Enables concurrent queue consumers with less lock contention. [CITED: https://www.postgresql.org/docs/18/sql-select.html] |
| Ad hoc select-then-insert idempotency | Unique constraints and `ON CONFLICT` | PostgreSQL 18 docs current page | Makes source-event/topic dedupe a database invariant. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] |
| Cross-aggregate mutation in aggregate logic | Saga/process-manager over local transactions and commands | Established saga pattern | Preserves aggregate consistency boundaries and avoids distributed transactions. [CITED: https://microservices.io/patterns/data/saga.html] |

**Deprecated/outdated:**
- Do not use 2PC for DB/broker coordination in this template; the cited outbox pattern lists 2PC as unavailable or undesirable for this problem. [CITED: https://microservices.io/patterns/data/transactional-outbox]
- Do not treat the outbox relay as exactly-once by default; the cited pattern states duplicate publish can happen and idempotency is required. [CITED: https://microservices.io/patterns/data/transactional-outbox]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | PM offset should advance only after all required command replies finish, unless a later design persists per-step workflow state. | Architecture Patterns / Common Pitfalls | A planner might implement a simpler offset-only PM that skips follow-ups after crashes. |
| A2 | Reusing `futures::BoxFuture` is preferable to adding `async-trait`. | Standard Stack | If planner needs object-safe async traits with less boilerplate, it may add a dependency contrary to the local pattern. |
| A3 | The first commerce process-manager flow should use command replies, not `InventoryReserved` event correlation alone. | Summary / Common Pitfalls | If product events are expanded with order correlation in the same phase, event-only correlation becomes viable. |
| A4 | Dispatcher retries should update `available_at` with a simple bounded backoff. | Code Examples | If users require a specific retry policy, planner needs more product input. |

## Open Questions

1. **Should Phase 6 add a product reservation event correlation field such as `order_id`?**
   - What we know: Current product events do not contain order IDs. [VERIFIED: crates/example-commerce/src/product.rs]
   - What's unclear: Whether the commerce fixture should stay minimal or add correlation to product events for clearer workflow event reactions. [ASSUMED]
   - Recommendation: Do not mutate domain event shape unless necessary; use command metadata causation/correlation and command replies for the Phase 6 example. [VERIFIED: crates/es-store-postgres/src/models.rs] [ASSUMED]

2. **Should outbox derivation be request-attached or trait-driven inside storage?**
   - What we know: `AppendRequest` currently carries stream, expected revision, command metadata, idempotency key, and events. [VERIFIED: crates/es-store-postgres/src/models.rs]
   - What's unclear: Whether the planner will prefer adding outbox messages to `AppendRequest` or adding a derivation trait passed to append. [ASSUMED]
   - Recommendation: Add an `outbox_messages: Vec<NewOutboxMessage>` field or an `append_with_outbox` API where each message references a known `source_event_id`; avoid calling domain-specific derivation from private SQL. [VERIFIED: crates/es-store-postgres/src/models.rs] [ASSUMED]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust/Cargo | Build, tests, crate metadata | Yes | `cargo 1.85.1` | None needed. [VERIFIED: command probe] |
| Docker | Testcontainers PostgreSQL tests | Yes | `Docker version 29.3.1` | If unavailable later, run non-container unit tests and flag integration tests. [VERIFIED: command probe] |
| PostgreSQL CLI `psql` | Optional manual DB inspection | No | - | Use SQLx/testcontainers logs or Rust integration assertions. [VERIFIED: command probe] |
| `pnpm` | Context7 CLI fallback / project Node tooling | Yes | `10.32.1` | `npx --yes` worked for Context7 docs. [VERIFIED: command probe] |
| cargo-nextest | Optional faster tests | Not checked as required; existing project uses `cargo test` | - | Use `cargo test`. [VERIFIED: .planning/phases/05-cqrs-projection-and-query-catch-up/05-RESEARCH.md] |

**Missing dependencies with no fallback:**
- None for Phase 6 planning; Docker is available for PostgreSQL integration tests. [VERIFIED: command probe]

**Missing dependencies with fallback:**
- `psql` is not available; Rust integration tests can verify database behavior. [VERIFIED: command probe]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness via `cargo test`; PostgreSQL integration tests via existing Testcontainers harness. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| Config file | Root `Cargo.toml`; no nextest config found. [VERIFIED: `find . -maxdepth 3 ...`] |
| Quick run command | `cargo test -p es-outbox && cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture` [ASSUMED] |
| Full suite command | `cargo test --workspace` [VERIFIED: Cargo.toml] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| INT-01 | Append creates events, command dedupe, and outbox rows atomically; rollback/conflict creates no outbox rows | PostgreSQL integration | `cargo test -p es-store-postgres --test outbox append_creates_outbox_rows_atomically -- --test-threads=1 --nocapture` | No - Wave 0. [VERIFIED: `find crates/es-store-postgres/tests`] |
| INT-02 | Dispatcher claims pending rows, publishes through trait, and marks published | Unit + PostgreSQL integration | `cargo test -p es-outbox dispatcher && cargo test -p es-store-postgres --test outbox dispatcher_marks_successful_rows_published -- --test-threads=1 --nocapture` | No - Wave 0. [VERIFIED: `find crates/es-outbox crates/es-store-postgres/tests`] |
| INT-03 | Duplicate source event/topic rows are prevented and retry republish uses same idempotency key | Unit + PostgreSQL integration | `cargo test -p es-store-postgres --test outbox outbox_is_idempotent_by_source_event_and_topic -- --test-threads=1 --nocapture` | No - Wave 0. [VERIFIED: `find crates/es-store-postgres/tests`] |
| INT-04 | Process manager consumes order/product events, submits follow-up commands through gateways, and advances offset only after replies | Unit/integration with fake gateways or runtime fake store | `cargo test -p es-outbox process_manager -- --nocapture` | No - Wave 0. [VERIFIED: `find crates/es-outbox/src`] |

### Sampling Rate

- **Per task commit:** Run the focused package test for the touched crate, plus `cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture` for PostgreSQL outbox behavior. [ASSUMED]
- **Per wave merge:** Run `cargo test -p es-outbox && cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture`. [ASSUMED]
- **Phase gate:** Run `cargo test --workspace` before `/gsd-verify-work`. [VERIFIED: prior phase validation style in .planning/phases/05-cqrs-projection-and-query-catch-up/05-VALIDATION.md]

### Wave 0 Gaps

- [ ] `crates/es-outbox/src/error.rs` - typed outbox errors for publisher, dispatcher, process-manager command replies. [VERIFIED: crates/es-outbox/src/lib.rs]
- [ ] `crates/es-outbox/src/models.rs` - validated outbox message, status, batch limit, worker ID, dispatch outcome. [VERIFIED: crates/es-outbox/src/lib.rs]
- [ ] `crates/es-outbox/src/publisher.rs` - `Publisher` trait and in-memory idempotent test publisher. [VERIFIED: crates/es-outbox/src/lib.rs]
- [ ] `crates/es-outbox/src/dispatcher.rs` - storage-neutral dispatch orchestration. [VERIFIED: crates/es-outbox/src/lib.rs]
- [ ] `crates/es-outbox/src/process_manager.rs` - process-manager contracts and commerce workflow test support. [VERIFIED: crates/es-outbox/src/lib.rs]
- [ ] `crates/es-store-postgres/migrations/*_outbox.sql` - outbox and process-manager offset tables. [VERIFIED: `find crates/es-store-postgres/migrations`]
- [ ] `crates/es-store-postgres/src/outbox.rs` - PostgreSQL claim/mark/retry and PM offset repository. [VERIFIED: `find crates/es-store-postgres/src`]
- [ ] `crates/es-store-postgres/tests/outbox.rs` - container-backed outbox atomicity, claim, retry, idempotency tests. [VERIFIED: `find crates/es-store-postgres/tests`]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | No direct auth in Phase 6 | Preserve tenant ID from `CommandMetadata`; do not add auth logic. [VERIFIED: crates/es-store-postgres/src/models.rs] |
| V3 Session Management | No | No browser/session behavior in this phase. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | Yes, tenant isolation | Every outbox/PM table query must filter by tenant where data is tenant-owned. [VERIFIED: crates/es-store-postgres/migrations/20260417000000_event_store.sql] |
| V5 Input Validation | Yes | Reuse typed constructors for non-empty topics, worker IDs, positive limits, payload size, and valid retry settings. [VERIFIED: crates/es-store-postgres/src/models.rs] |
| V6 Cryptography | No new cryptography | Use UUIDs for identifiers; do not introduce crypto. [VERIFIED: Cargo.toml] |

### Known Threat Patterns For This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant outbox publish | Information Disclosure | Include tenant ID in outbox rows and repository filters; test tenant isolation. [VERIFIED: crates/es-store-postgres/migrations/20260417000000_event_store.sql] |
| Duplicate external side effects | Tampering | Unique `(tenant_id, source_event_id, topic)` and deterministic publisher idempotency key. [CITED: https://www.postgresql.org/docs/18/sql-insert.html] [CITED: https://microservices.io/patterns/data/transactional-outbox] |
| Poison message retry loop | Denial of Service | Track attempts, `available_at`, `last_error`, and transition to `failed` after max attempts. [ASSUMED] |
| SQL injection | Tampering | Continue SQLx parameter binding; existing code binds values instead of interpolating SQL strings. [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Workflow replay duplicate commands | Tampering | Deterministic command idempotency keys and existing command dedupe table. [VERIFIED: crates/es-store-postgres/src/sql.rs] |

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` - Phase INT requirements and out-of-scope direct broker publish. [VERIFIED]
- `.planning/ROADMAP.md` - Phase 6 goal and success criteria. [VERIFIED]
- `.planning/STATE.md` - prior decisions about event store as command success point, disruptor as in-process fabric, and Phase 6 current focus. [VERIFIED]
- `Cargo.toml` - workspace versions and Rust baseline. [VERIFIED]
- `crates/es-store-postgres/src/sql.rs` - append transaction shape, dedupe, event insert, global reads. [VERIFIED]
- `crates/es-store-postgres/migrations/20260417000000_event_store.sql` - events, streams, dedupe, snapshots schema. [VERIFIED]
- `crates/es-store-postgres/tests/common/mod.rs` - PostgreSQL 18 testcontainers harness. [VERIFIED]
- `crates/es-runtime/src/command.rs`, `crates/es-runtime/src/gateway.rs`, `crates/es-runtime/src/shard.rs`, `crates/es-runtime/src/store.rs` - command gateway, envelopes, replies, and BoxFuture port pattern. [VERIFIED]
- `crates/example-commerce/src/order.rs`, `crates/example-commerce/src/product.rs` - workflow event/command shapes. [VERIFIED]
- PostgreSQL 18 SELECT docs - `FOR UPDATE SKIP LOCKED` queue-like use. [CITED: https://www.postgresql.org/docs/18/sql-select.html]
- PostgreSQL 18 INSERT docs - `ON CONFLICT` behavior and unique-index arbitration. [CITED: https://www.postgresql.org/docs/18/sql-insert.html]
- PostgreSQL 18 explicit locking docs - transaction-level advisory lock behavior. [CITED: https://www.postgresql.org/docs/18/explicit-locking.html]
- Microservices.io Transactional Outbox - atomic message row and relay pattern, duplicate publish caveat. [CITED: https://microservices.io/patterns/data/transactional-outbox]
- Microservices.io Saga - sequence of local transactions and orchestration/choreography. [CITED: https://microservices.io/patterns/data/saga.html]
- Context7 SQLx docs - transactions and executor examples. [VERIFIED: Context7 `/launchbadge/sqlx`]
- Context7 Tokio docs - `mpsc::Sender::try_send` and timing primitives. [VERIFIED: Context7 `/websites/rs_tokio`]
- Context7 uuid docs - serde and UUIDv7 feature behavior. [VERIFIED: Context7 `/uuid-rs/uuid`]

### Secondary (MEDIUM confidence)

- `cargo search` / `cargo info` for crate versions: `sqlx`, `tokio`, `uuid`, `async-trait`, `serde`, `time`, `tracing`. [VERIFIED: cargo registry commands]
- Prior project research files `.planning/research/STACK.md`, `.planning/research/ARCHITECTURE.md`, and `.planning/research/PITFALLS.md` for project-level outbox direction. [VERIFIED: `rg outbox .planning/research`]

### Tertiary (LOW confidence)

- Assumed retry policy and process-manager offset timing details where no user-specific retry/backoff policy exists yet. [ASSUMED]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - current workspace manifests, cargo registry commands, and existing code paths verify the stack. [VERIFIED: Cargo.toml] [VERIFIED: cargo registry commands]
- Architecture: HIGH for outbox/dispatcher; MEDIUM for process-manager details because current commerce product events lack order correlation and require command-reply coordination. [VERIFIED: crates/example-commerce/src/product.rs] [CITED: https://microservices.io/patterns/data/transactional-outbox]
- Pitfalls: HIGH for outbox duplicate/atomicity and PostgreSQL locking; MEDIUM for PM retry/offset policy because it is a local design choice. [CITED: https://microservices.io/patterns/data/transactional-outbox] [CITED: https://www.postgresql.org/docs/18/sql-select.html]

**Research date:** 2026-04-18 [VERIFIED: `date +%F`]
**Valid until:** 2026-05-18 for local architecture and PostgreSQL patterns; recheck crate versions before implementation if dependency changes are planned. [ASSUMED]

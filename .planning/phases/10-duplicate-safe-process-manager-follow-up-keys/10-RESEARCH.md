# Phase 10: Duplicate-Safe Process Manager Follow-Up Keys - Research

**Researched:** 2026-04-20  
**Domain:** Rust event-sourced process-manager idempotency and retry replay  
**Confidence:** HIGH

## User Constraints

### Locked Phase Scope
- Phase 10 is `Duplicate-Safe Process Manager Follow-Up Keys`; its goal is deterministic process-manager follow-up commands that avoid idempotency collisions for repeated product lines. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]
- Phase 10 depends on Phase 9 and closes the commerce process-manager to runtime/store idempotency replay gap from the v1 milestone audit. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- Phase 10 covers `STORE-03`, `RUNTIME-05`, `DOM-04`, `DOM-05`, and `INT-04`. [VERIFIED: user prompt] [VERIFIED: .planning/REQUIREMENTS.md]
- Success requires reserve and release follow-up keys to distinguish duplicate product lines or to coalesce repeated lines before command emission. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]
- Success requires true process-manager retries to replay original committed follow-up outcomes through runtime/store idempotency. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]
- Success requires duplicate same-product order lines not to collapse distinct reserve/release commands into the wrong replay record. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]
- Success requires app-level process-manager tests for duplicate product lines and replayed follow-up processing. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]

### Project Constraints
- Use Rust-first service implementation; the event store is the source of truth, and disruptor rings are not durable state. [VERIFIED: user AGENTS.md instructions] [VERIFIED: .planning/REQUIREMENTS.md]
- Process-manager follow-up commands must go through the same command gateway as other commands. [VERIFIED: user prompt] [VERIFIED: crates/app/src/commerce_process_manager.rs]
- True retries must replay original committed outcomes through runtime/store idempotency, not process-manager-local dedupe state. [VERIFIED: user prompt] [VERIFIED: .planning/STATE.md]
- Prefer `pnpm` for Node tooling and `uv` for Python tooling if those ecosystems are used. [VERIFIED: user AGENTS.md instructions]
- No `CLAUDE.md` file was present in the project root during research, and no project-local `.claude/skills` or `.agents/skills` index was found. [VERIFIED: shell `test -f CLAUDE.md`; `find .claude/skills .agents/skills -name SKILL.md`]

### Deferred Ideas
- Distributed partition ownership remains v2/out of scope for this milestone. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/STATE.md]
- Phase 11 owns HTTP E2E debt, observability/documentation archive hygiene, and stale requirements traceability cleanup. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STORE-03 | Command deduplication returns the prior committed result for a repeated tenant/idempotency key. [VERIFIED: .planning/REQUIREMENTS.md] | Runtime checks shard-local dedupe and durable replay before decision, and PostgreSQL stores command replay by `(tenant_id, idempotency_key)`. [VERIFIED: crates/es-runtime/src/shard.rs:170] [VERIFIED: crates/es-runtime/src/shard.rs:187] [VERIFIED: crates/es-store-postgres/src/sql.rs:354] |
| RUNTIME-05 | Command replies are sent only after durable event-store append commit succeeds. [VERIFIED: .planning/REQUIREMENTS.md] | Existing replay tests use real `CommandEngine` paths, so Phase 10 should extend those app tests rather than bypassing runtime/store behavior. [VERIFIED: crates/app/src/commerce_process_manager.rs:1037] [VERIFIED: cargo test -p app commerce_process_manager] |
| DOM-04 | Order commands place, confirm, reject, and cancel orders referencing user and product identifiers. [VERIFIED: .planning/REQUIREMENTS.md] | `OrderPlaced` payload contains ordered `Vec<OrderLine>`, so line ordinal is available without schema migration. [VERIFIED: crates/example-commerce/src/order.rs:28] [VERIFIED: crates/example-commerce/src/order.rs:75] |
| DOM-05 | Domain invariants prevent invalid orders and invalid inventory movements. [VERIFIED: .planning/REQUIREMENTS.md] | Duplicate same-product lines are valid domain input today, so process-manager keys must preserve line identity or intentionally aggregate quantities before emission. [VERIFIED: crates/example-commerce/src/order.rs:151] [VERIFIED: crates/example-commerce/src/order.rs:190] |
| INT-04 | A process-manager example reacts to order/product events and issues follow-up commands through the same command gateway. [VERIFIED: .planning/REQUIREMENTS.md] | `CommerceOrderProcessManager` handles `OrderPlaced`, submits `ProductCommand::ReserveInventory` and `ReleaseInventory`, then confirms or rejects through gateways. [VERIFIED: crates/app/src/commerce_process_manager.rs:40] [VERIFIED: crates/app/src/commerce_process_manager.rs:72] [VERIFIED: crates/app/src/commerce_process_manager.rs:109] [VERIFIED: crates/app/src/commerce_process_manager.rs:162] |

</phase_requirements>

## Summary

Phase 10 is a narrow correctness phase in `crates/app/src/commerce_process_manager.rs`, not a new library-selection or database-schema phase. [VERIFIED: .planning/ROADMAP.md] The current workflow decodes `OrderPlaced`, iterates `lines`, and emits one reserve command per line, but reserve keys use `pm:{manager}:{source_event_id}:reserve:{product_id}` and release keys use `pm:{manager}:{source_event_id}:release:{product_id}`. [VERIFIED: crates/app/src/commerce_process_manager.rs:54] [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: crates/app/src/commerce_process_manager.rs:115]

The established pattern is to treat an idempotency key as the caller's semantic operation identity, not as a best-effort hash of similar request payloads. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/] For a process-manager retry, the semantic identity is "the same follow-up step caused by the same committed source event"; for two repeated product lines in one order, the semantic identities are different if the workflow emits separate commands for each line. [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [VERIFIED: crates/example-commerce/src/order.rs:28]

**Primary recommendation:** keep one follow-up command per source order line and add a stable zero-based line ordinal segment to reserve/release idempotency keys, e.g. `pm:{manager}:{source_event_id}:reserve:{line_index}:{product_id}` and `pm:{manager}:{source_event_id}:release:{line_index}:{product_id}`. [VERIFIED: crates/example-commerce/src/order.rs:75] [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/] This is lower blast radius than coalescing because it does not change command quantity semantics, reserve/release sequencing, command counts, product event quantities, or existing order payload schema. [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [VERIFIED: crates/app/src/commerce_process_manager.rs:97] [VERIFIED: crates/example-commerce/src/product.rs:172]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Follow-up key construction | App process manager | Runtime dedupe | The process manager defines semantic operation identity; runtime/store only enforce tenant/idempotency replay for the key they receive. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: crates/es-runtime/src/command.rs:14] [VERIFIED: crates/es-store-postgres/src/sql.rs:354] |
| Retry replay | Runtime / Event store | App process manager | Runtime checks cache and durable replay before aggregate decision; PostgreSQL persists replay payload by tenant/idempotency key. [VERIFIED: crates/es-runtime/src/shard.rs:175] [VERIFIED: crates/es-runtime/src/shard.rs:187] [VERIFIED: crates/es-store-postgres/src/sql.rs:410] |
| Duplicate product-line distinction | Domain event payload / App process manager | Tests | `OrderPlaced` stores lines as an ordered `Vec<OrderLine>`, and the app process manager can enumerate that vector when constructing commands. [VERIFIED: crates/example-commerce/src/order.rs:75] [VERIFIED: crates/app/src/commerce_process_manager.rs:68] |
| Reserve/release compensation | App process manager | Product aggregate | Failed reserve flows release the previously reserved lines and then reject the order; product aggregate enforces available/reserved inventory invariants. [VERIFIED: crates/app/src/commerce_process_manager.rs:96] [VERIFIED: crates/app/src/commerce_process_manager.rs:106] [VERIFIED: crates/example-commerce/src/product.rs:208] |
| Durable process-manager offset | es-outbox / storage adapter | App process manager | `process_batch` advances offset only after `manager.process(event).await?` completes, so key fixes must preserve reply-gated processing. [VERIFIED: crates/es-outbox/src/process_manager.rs:97] [VERIFIED: crates/es-outbox/src/process_manager.rs:118] |

## Standard Stack

### Core

| Library / Crate | Version | Purpose | Why Standard |
|-----------------|---------|---------|--------------|
| `app` crate | workspace `0.1.0` | Owns commerce process-manager composition. [VERIFIED: cargo metadata] | The workflow depends on `es-outbox`, `es-runtime`, and `example-commerce`, so app composition is the correct place to build domain-specific follow-up commands. [VERIFIED: crates/app/Cargo.toml] |
| `es-outbox` crate | workspace `0.1.0` | Provides storage-neutral `ProcessManager`, `ProcessEvent`, and offset contracts. [VERIFIED: cargo metadata] | It keeps process-manager contracts free of runtime and PostgreSQL dependencies. [VERIFIED: crates/es-outbox/Cargo.toml] [VERIFIED: crates/es-outbox/src/process_manager.rs] |
| `es-runtime` crate | workspace `0.1.0` | Provides `CommandGateway`, `CommandEnvelope`, `CommandEngine`, and duplicate replay. [VERIFIED: cargo metadata] | It is the existing gateway/replay boundary and should not be bypassed by process-manager-local calls. [VERIFIED: crates/es-runtime/src/gateway.rs] [VERIFIED: crates/es-runtime/src/shard.rs:170] |
| `example-commerce` crate | workspace `0.1.0` | Provides typed `OrderLine`, `OrderEvent`, `ProductCommand`, and inventory semantics. [VERIFIED: cargo metadata] | `OrderPlaced` already preserves ordered line data needed for ordinal keys. [VERIFIED: crates/example-commerce/src/order.rs:75] |
| Rust | `rustc 1.85.1`, workspace `rust-version = "1.85"` | Compiler/toolchain. [VERIFIED: `rustc --version`] [VERIFIED: Cargo.toml] | The phase should stay within the existing Rust 2024 workspace baseline. [VERIFIED: Cargo.toml] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio` | `1.52.0` workspace dependency | Async tests and one-shot gateway replies. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/commerce_process_manager.rs:69] | Use existing `#[tokio::test]` app tests; do not introduce another async test framework. [VERIFIED: crates/app/src/commerce_process_manager.rs:683] |
| `serde_json` | `1.0.149` workspace dependency | Encode/decode order event payloads in tests and process manager. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/commerce_process_manager.rs:183] | Keep using typed `serde_json::to_value`/`from_value` rather than handwritten payload maps. [VERIFIED: crates/app/src/commerce_process_manager.rs:263] |
| `uuid` | `1.23.0` workspace dependency | Source event IDs and follow-up metadata command IDs. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/commerce_process_manager.rs:191] | Keep `event.event_id` as the source retry anchor and `Uuid::now_v7()` only for new command metadata. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: crates/app/src/commerce_process_manager.rs:191] |
| `sqlx` / PostgreSQL | `sqlx 0.8.6` workspace dependency | Durable command replay lookup and append dedupe. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-store-postgres/src/sql.rs:410] | No schema change is required for ordinal key strings because command dedupe stores opaque `idempotency_key` text. [VERIFIED: crates/es-store-postgres/src/sql.rs:354] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Line ordinal in idempotency key | Coalesce repeated product lines by `(product_id, sku, product_available)` and total quantity before reserve/release | Coalescing reduces command count but changes workflow granularity, replay event quantities, failure-release tracking, and test expectations. [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [VERIFIED: crates/example-commerce/src/product.rs:244] |
| App-local duplicate tracking | Runtime/store replay by `(tenant_id, idempotency_key)` | App-local tracking would not survive restart or prove Phase 8 replay behavior. [VERIFIED: crates/es-runtime/src/shard.rs:187] [VERIFIED: crates/es-store-postgres/src/sql.rs:410] |
| Random UUID segment per follow-up | Deterministic source event + ordinal + target identity | Random keys would avoid collisions but break true retry replay because a retry would emit different keys. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/] |

**Installation:**
```bash
# No new dependencies are recommended for this phase. [VERIFIED: Cargo.toml]
```

**Version verification:** local workspace versions were verified with `cargo metadata --no-deps --format-version 1`, `cargo --version`, and `rustc --version`. [VERIFIED: command output]

## Architecture Patterns

### System Architecture Diagram

```text
Committed OrderPlaced event
  |
  v
CommerceOrderProcessManager::process(event)
  |
  +--> decode typed OrderEvent::OrderPlaced payload
  |
  +--> enumerate lines with stable source-event-local ordinal
  |       |
  |       +--> ReserveInventory(product_id, quantity)
  |              idempotency = pm:{manager}:{event_id}:reserve:{line_index}:{product_id}
  |              |
  |              v
  |          CommandGateway<Product>
  |              |
  |              v
  |          es-runtime duplicate replay check by (tenant_id, idempotency_key)
  |              |
  |              +--> duplicate found: replay committed ProductReply
  |              |
  |              +--> no duplicate: decide/apply/append and store replay record
  |
  +--> if all reserves succeed: ConfirmOrder with existing order-level key
  |
  +--> if a reserve fails: release prior successful line ordinals, then RejectOrder
          |
          +--> ReleaseInventory(product_id, quantity)
                 idempotency = pm:{manager}:{event_id}:release:{line_index}:{product_id}
```

The diagram reflects the current gateway and store replay boundaries: command envelopes carry the idempotency key, runtime checks cache and durable replay before aggregate decision, and the store persists replay records by tenant/key. [VERIFIED: crates/es-runtime/src/command.rs:14] [VERIFIED: crates/es-runtime/src/shard.rs:170] [VERIFIED: crates/es-store-postgres/src/sql.rs:354]

### Recommended Project Structure

```text
crates/app/src/commerce_process_manager.rs  # Change key construction and app tests. [VERIFIED: current file]
crates/example-commerce/src/order.rs        # No schema change recommended; line order already exists. [VERIFIED: crates/example-commerce/src/order.rs:75]
crates/es-runtime/src/*                     # No runtime change recommended; replay boundary already accepts opaque keys. [VERIFIED: crates/es-runtime/src/command.rs:14]
crates/es-store-postgres/src/*              # No storage change recommended; dedupe key is already text and tenant-scoped. [VERIFIED: crates/es-store-postgres/src/sql.rs:354]
```

### Pattern 1: Deterministic Follow-Up Step Identity

**What:** Build the process-manager key from stable workflow identity plus the specific step identity: manager name, source event ID, action, line ordinal, and target product ID. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: crates/example-commerce/src/order.rs:75]

**When to use:** Use this when the process manager emits one command per item in an ordered source event collection. [VERIFIED: crates/app/src/commerce_process_manager.rs:68]

**Example:**
```rust
// Source: local pattern in crates/app/src/commerce_process_manager.rs, adjusted for line ordinal. [VERIFIED: crates/app/src/commerce_process_manager.rs:68]
for (line_index, line) in lines.into_iter().enumerate() {
    let product_id = line.product_id.clone();
    let idempotency_key = format!(
        "pm:{}:{}:reserve:{}:{}",
        self.name.as_str(),
        event.event_id,
        line_index,
        product_id.as_str()
    );

    let envelope = CommandEnvelope::<Product>::new(
        ProductCommand::ReserveInventory {
            product_id,
            quantity: line.quantity,
        },
        follow_up_metadata(event),
        idempotency_key,
        reply,
    )?;
}
```

### Pattern 2: Release Uses the Original Reserved Step Identity

**What:** Store the line ordinal alongside successful reservations, then build release keys from the same ordinal and product identity. [VERIFIED: crates/app/src/commerce_process_manager.rs:97] [VERIFIED: crates/app/src/commerce_process_manager.rs:106]

**When to use:** Use this when a later failed reserve triggers compensation for already committed reserve commands. [VERIFIED: crates/app/src/commerce_process_manager.rs:96]

**Example:**
```rust
// Source: local release-compensation pattern, adjusted to preserve ordinal. [VERIFIED: crates/app/src/commerce_process_manager.rs:97]
let mut reserved_lines: Vec<(usize, ProductId, Quantity)> = Vec::new();

// On successful reserve:
reserved_lines.push((line_index, product_id, quantity));

// On compensation:
for (line_index, product_id, quantity) in reserved_lines {
    let idempotency_key = format!(
        "pm:{}:{}:release:{}:{}",
        self.name.as_str(),
        event.event_id,
        line_index,
        product_id.as_str()
    );
    // Submit ReleaseInventory through CommandGateway<Product>. [VERIFIED: crates/app/src/commerce_process_manager.rs:109]
}
```

### Pattern 3: Replay Test Through Real CommandEngine

**What:** Use the existing `ReplayAwareProductStore`, `ReplayAwareOrderStore`, and real `CommandEngine` harness to process the same `ProcessEvent` twice, then assert append count stays at one per distinct follow-up key and replay positions are returned on retry. [VERIFIED: crates/app/src/commerce_process_manager.rs:462] [VERIFIED: crates/app/src/commerce_process_manager.rs:1037]

**When to use:** Use this for Phase 10 replay acceptance because the gap is specifically integration with runtime/store idempotency replay. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

**Example:**
```rust
// Source: existing app replay test structure. [VERIFIED: crates/app/src/commerce_process_manager.rs:1037]
let first_task = tokio::spawn(async move { manager.process(&event).await });
assert!(product_engine.process_one().await?);
assert!(order_engine.process_one().await?);
first_task.await??;

let second_task = tokio::spawn(async move { manager.process(&event).await });
assert!(product_engine.process_one().await?);
assert!(order_engine.process_one().await?);
second_task.await??;

assert_eq!(expected_append_count, product_store.append_count());
```

### Anti-Patterns to Avoid

- **Using only product ID in reserve/release keys:** This is the current collision source for repeated product lines in one `OrderPlaced` event. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- **Generating random follow-up idempotency keys:** This prevents a replayed process-manager event from addressing the original replay record. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/] [VERIFIED: crates/es-runtime/src/shard.rs:187]
- **Changing runtime/store dedupe semantics for this phase:** Runtime/store already dedupe by tenant/key and preserve typed replay payloads, so broad rewrites increase risk without addressing the source of the collision. [VERIFIED: crates/es-runtime/src/shard.rs:170] [VERIFIED: crates/es-store-postgres/src/sql.rs:391]
- **Coalescing silently without tests for quantities and compensation:** Coalescing changes the number and quantity of emitted `ReserveInventory` and `ReleaseInventory` commands. [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [VERIFIED: crates/example-commerce/src/product.rs:244]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Durable retry memory | A process-manager `HashMap` of processed line keys | Existing runtime/store replay by `(tenant_id, idempotency_key)` | Runtime/store replay survives runtime cache misses and restarts; app-local memory does not. [VERIFIED: crates/es-runtime/src/shard.rs:187] [VERIFIED: crates/es-store-postgres/src/sql.rs:410] |
| Follow-up command delivery | Direct product/order aggregate calls | Existing `CommandGateway<Product>` and `CommandGateway<Order>` | INT-04 requires follow-up commands through the same gateway, and existing tests already inspect real routed envelopes. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: crates/app/src/commerce_process_manager.rs:87] |
| Payload parsing | Handwritten JSON maps for `OrderPlaced` | `serde_json::from_value::<OrderEvent>` | The current code already decodes typed commerce events, which preserves variant/schema handling. [VERIFIED: crates/app/src/commerce_process_manager.rs:183] |
| New line identity schema | New `OrderLineId` migration for this phase | Source-event-local `enumerate()` ordinal | `OrderPlaced` already stores an ordered vector of lines, and success criteria allow distinguishing duplicate lines without schema migration. [VERIFIED: crates/example-commerce/src/order.rs:75] [VERIFIED: user prompt] |
| Saga execution engine | Temporal/custom workflow engine | Existing `ProcessManager` trait and gateway composition | The phase is a localized correctness fix inside the established process-manager boundary. [VERIFIED: crates/es-outbox/src/process_manager.rs] [VERIFIED: .planning/ROADMAP.md] |

**Key insight:** the idempotency key must encode the operation identity at the same granularity as command emission; since the current workflow emits one reserve/release command per line, the key needs line-level identity. [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/]

## Common Pitfalls

### Pitfall 1: Key Fix Only Applied to Reserve

**What goes wrong:** duplicate-line reserve commands are separated, but compensation release commands still collide on product ID. [VERIFIED: crates/app/src/commerce_process_manager.rs:115]  
**Why it happens:** release iterates `reserved_lines`, and that vector currently stores only `(product_id, quantity)`. [VERIFIED: crates/app/src/commerce_process_manager.rs:97]  
**How to avoid:** store `(line_index, product_id, quantity)` for successful reserves and use the same ordinal segment in release keys. [VERIFIED: crates/app/src/commerce_process_manager.rs:97]  
**Warning signs:** tests only cover all-success duplicate-line orders and do not force a later-line failure after duplicate same-product reservations. [VERIFIED: crates/app/src/commerce_process_manager.rs:852]

### Pitfall 2: Retrying With a Different Semantic Key

**What goes wrong:** process-manager retries append new commands instead of replaying original committed outcomes. [VERIFIED: crates/es-runtime/src/shard.rs:187]  
**Why it happens:** keys include nondeterministic data such as fresh UUIDs, timestamps, or a counter that depends on prior runtime state. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/]  
**How to avoid:** derive keys only from `ProcessManagerName`, source `event_id`, action, line ordinal from the source payload, and target aggregate ID. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: crates/example-commerce/src/order.rs:75]  
**Warning signs:** replay tests show `append_count() > distinct_follow_up_key_count` after processing the same `ProcessEvent` twice. [VERIFIED: crates/app/src/commerce_process_manager.rs:1114]

### Pitfall 3: Treating Exact Payload Equality as Idempotency

**What goes wrong:** two distinct same-product lines with equal product and quantity collapse into one idempotency key even though the workflow emitted two separate commands. [VERIFIED: crates/app/src/commerce_process_manager.rs:68]  
**Why it happens:** matching on product ID or payload content cannot distinguish caller intent when repeated operations are valid. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/]  
**How to avoid:** use explicit operation identity; line ordinal is explicit identity for separate per-line commands. [VERIFIED: crates/example-commerce/src/order.rs:75]  
**Warning signs:** expected key vectors contain duplicate strings for an order with two same-product lines. [VERIFIED: crates/app/src/commerce_process_manager.rs:512]

### Pitfall 4: Advancing Process-Manager Offset Before Replies

**What goes wrong:** a failed or dropped follow-up reply can be skipped forever if the offset advances early. [VERIFIED: crates/es-outbox/src/process_manager.rs:118]  
**Why it happens:** process-manager offset advancement is durable and monotonic. [VERIFIED: crates/es-outbox/src/process_manager.rs:118]  
**How to avoid:** keep all follow-up submission and reply waits inside `manager.process(event).await?` before `process_batch` advances the offset. [VERIFIED: crates/es-outbox/src/process_manager.rs:114]  
**Warning signs:** tests do not include `process_manager_waits_for_replies_before_success`. [VERIFIED: crates/app/src/commerce_process_manager.rs:1125]

## Code Examples

Verified patterns from local sources:

### Current Collision Source

```rust
// Source: crates/app/src/commerce_process_manager.rs:78 [VERIFIED]
format!(
    "pm:{}:{}:reserve:{}",
    self.name.as_str(),
    event.event_id,
    product_id.as_str()
)
```

This key cannot distinguish two lines with the same `product_id` in one `OrderPlaced` event. [VERIFIED: crates/app/src/commerce_process_manager.rs:68] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

### Recommended Reserve Key Helper

```rust
// Source: recommended Phase 10 local helper pattern. [VERIFIED: crates/app/src/commerce_process_manager.rs:78]
fn follow_up_line_key(
    manager: &ProcessManagerName,
    source_event_id: Uuid,
    action: &str,
    line_index: usize,
    product_id: &ProductId,
) -> String {
    format!(
        "pm:{}:{}:{}:{}:{}",
        manager.as_str(),
        source_event_id,
        action,
        line_index,
        product_id.as_str()
    )
}
```

### Duplicate-Line Test Shape

```rust
// Source: existing gateway-receiver test style. [VERIFIED: crates/app/src/commerce_process_manager.rs:969]
let same_product = product_id("product-1");
let event = process_event(OrderEvent::OrderPlaced {
    order_id: order_id(),
    user_id: UserId::new("user-1").expect("user id"),
    lines: vec![line(same_product.clone()), line(same_product.clone())],
});

let first_reserve = receive_product(&mut product_rx).await;
let second_reserve = receive_product(&mut product_rx).await;

assert_ne!(
    first_reserve.envelope.idempotency_key,
    second_reserve.envelope.idempotency_key
);
```

### Replay Test Shape for Duplicate Lines

```rust
// Source: existing replay-aware test style. [VERIFIED: crates/app/src/commerce_process_manager.rs:1037]
assert_eq!(2, product_store.append_count());
assert_eq!(
    vec![
        format!("pm:{}:{}:reserve:0:{}", manager_name.as_str(), event_id, product.as_str()),
        format!("pm:{}:{}:reserve:1:{}", manager_name.as_str(), event_id, product.as_str()),
    ],
    product_store.idempotency_keys()
);
```

The exact helper/store shape may need minor extension because the current `ReplayAwareProductStore` stores one replay record, not a map keyed by idempotency key. [VERIFIED: crates/app/src/commerce_process_manager.rs:462] [VERIFIED: crates/app/src/commerce_process_manager.rs:557]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Infer duplicate intent from exact or similar request parameters | Use an explicit caller/client request identifier to express idempotent operation identity | AWS Builders Library article is current and crawled recently by search during research. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/] | Product ID alone is not a safe operation identity when repeated same-product lines are valid. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] |
| Assume exactly-once infrastructure prevents handler duplicates | Make handlers/commands idempotent and replay safe | Idempotent Consumer pattern remains an established at-least-once messaging pattern. [CITED: https://microservices.io/post/microservices/patterns/2020/10/16/idempotent-consumer.html] | Runtime/store replay must remain the source of duplicate command truth. [VERIFIED: crates/es-runtime/src/shard.rs:187] |
| Undo failed workflow steps by mutating prior state | Append/issue compensating actions as idempotent commands | Azure Architecture Center documents compensation and saga retryable/idempotent steps. [CITED: https://learn.microsoft.com/en-us/azure/architecture/patterns/compensating-transaction] [CITED: https://learn.microsoft.com/en-us/azure/architecture/patterns/saga] | Release commands need collision-safe keys just like reserve commands. [VERIFIED: crates/app/src/commerce_process_manager.rs:109] |

**Deprecated/outdated:**
- Treating idempotency as duplicate detection over business fields is unsafe for repeated valid operations. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/]
- Treating process-manager offsets as the only retry guard is unsafe because offsets protect committed-event progress, not follow-up command-level replay. [VERIFIED: crates/es-outbox/src/process_manager.rs:97] [VERIFIED: crates/es-runtime/src/shard.rs:187]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Existing order-line vector order is stable enough to serve as source-event-local line identity. [ASSUMED] | Primary recommendation | If a future event upcaster reorders lines, retry keys could drift; current code has no upcaster and stores `Vec<OrderLine>` directly. [VERIFIED: crates/example-commerce/src/order.rs:75] |

## Open Questions (RESOLVED)

1. **RESOLVED: Should duplicate product lines be coalesced instead of line-keyed?**
   - What we know: Success criteria permit either distinguishing duplicate lines or coalescing before command emission. [VERIFIED: user prompt]
   - What's unclear: The project has not recorded a domain decision that repeated same-product lines should become one inventory reservation. [VERIFIED: .planning/STATE.md]
   - RESOLVED decision: Do not coalesce in Phase 10; use line ordinal keys because it preserves existing command and event granularity. [VERIFIED: crates/app/src/commerce_process_manager.rs:68]

2. **RESOLVED: Should line ordinals be one-based or zero-based in keys?**
   - What we know: Rust `enumerate()` produces zero-based `usize` indexes. [VERIFIED: Rust standard library behavior via local code compilation baseline]
   - What's unclear: No project convention for user-facing process-manager key ordinals exists. [VERIFIED: rg results in crates/app/src/commerce_process_manager.rs]
   - RESOLVED decision: Use zero-based ordinals because keys are internal and it maps directly to `enumerate()` without arithmetic. [VERIFIED: crates/app/src/commerce_process_manager.rs:68]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | Build/test Phase 10 | ✓ | `rustc 1.85.1` | None needed. [VERIFIED: `rustc --version`] |
| Cargo | Run app and workspace tests | ✓ | `cargo 1.85.1` | None needed. [VERIFIED: `cargo --version`] |
| PostgreSQL/Docker | Optional full store integration | Not required for recommended app tests | — | Use existing replay-aware in-memory store for Phase 10 quick coverage. [VERIFIED: crates/app/src/commerce_process_manager.rs:462] |

**Missing dependencies with no fallback:** None for the recommended Phase 10 implementation and app-level tests. [VERIFIED: cargo test -p app commerce_process_manager]

**Missing dependencies with fallback:** PostgreSQL/Docker are not required for the primary app-level regression because the existing replay-aware store exercises runtime/store replay contracts in process. [VERIFIED: crates/app/src/commerce_process_manager.rs:1037]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness plus `tokio::test` for async app tests. [VERIFIED: crates/app/src/commerce_process_manager.rs:683] |
| Config file | Workspace `Cargo.toml`; no separate app test config. [VERIFIED: Cargo.toml] |
| Quick run command | `cargo test -p app commerce_process_manager -- --nocapture` [VERIFIED: command passed during research] |
| Full suite command | `cargo test --workspace` [VERIFIED: Cargo workspace exists in Cargo.toml] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| STORE-03 | Reprocessing the same process-manager event replays committed follow-up outcomes instead of appending new commands. [VERIFIED: .planning/REQUIREMENTS.md] | integration-style unit with real `CommandEngine` and replay-aware store | `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` | ✅ [VERIFIED: crates/app/src/commerce_process_manager.rs:1037] |
| RUNTIME-05 | Process-manager waits for gateway replies before success. [VERIFIED: .planning/REQUIREMENTS.md] | async unit | `cargo test -p app process_manager_waits_for_replies_before_success -- --nocapture` | ✅ [VERIFIED: crates/app/src/commerce_process_manager.rs:1125] |
| DOM-04 | Duplicate same-product order lines remain representable in `OrderPlaced`. [VERIFIED: .planning/REQUIREMENTS.md] | app unit plus typed event payload | `cargo test -p app duplicate_product_lines_emit_distinct_reserve_keys -- --nocapture` | ❌ Wave 0 gap [VERIFIED: rg results] |
| DOM-05 | Failed duplicate-line reservation releases prior successful duplicate-line reservations with distinct keys. [VERIFIED: .planning/REQUIREMENTS.md] | app unit | `cargo test -p app duplicate_product_line_failure_releases_distinct_prior_lines -- --nocapture` | ❌ Wave 0 gap [VERIFIED: rg results] |
| INT-04 | Follow-up commands continue through `CommandGateway` with deterministic line-aware keys. [VERIFIED: .planning/REQUIREMENTS.md] | app unit/integration-style unit | `cargo test -p app commerce_process_manager -- --nocapture` | Partial: existing tests cover gateway path, missing duplicate-line assertions. [VERIFIED: cargo test -p app commerce_process_manager] |

### Sampling Rate

- **Per task commit:** `cargo test -p app commerce_process_manager -- --nocapture` [VERIFIED: command passed during research]
- **Per wave merge:** `cargo test -p app commerce_process_manager -- --nocapture && cargo test -p es-runtime runtime_duplicate -- --nocapture` [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- **Phase gate:** `cargo test --workspace` before `/gsd-verify-work`, with Docker-backed PostgreSQL suites allowed to be called out if environment blocks them. [VERIFIED: Cargo.toml] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

### Wave 0 Gaps

- [ ] `crates/app/src/commerce_process_manager.rs` — add `duplicate_product_lines_emit_distinct_reserve_keys`, covering REQ-DOM-04 / REQ-INT-04. [VERIFIED: rg results]
- [ ] `crates/app/src/commerce_process_manager.rs` — add `duplicate_product_line_failure_releases_distinct_prior_lines`, covering REQ-DOM-05 / REQ-INT-04. [VERIFIED: rg results]
- [ ] `crates/app/src/commerce_process_manager.rs` — extend replay-aware store to record replay records per idempotency key if needed for multi-line replay coverage. [VERIFIED: crates/app/src/commerce_process_manager.rs:557]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 10 adds no user authentication surface. [VERIFIED: crates/app/src/commerce_process_manager.rs] |
| V3 Session Management | no | Phase 10 adds no HTTP session or token handling. [VERIFIED: crates/app/src/commerce_process_manager.rs] |
| V4 Access Control | yes | Preserve tenant ID from `ProcessEvent` into follow-up `CommandMetadata`; do not derive tenant from payload. [VERIFIED: crates/app/src/commerce_process_manager.rs:191] |
| V5 Input Validation | yes | Continue typed `OrderEvent` JSON decode and typed domain command constructors. [VERIFIED: crates/app/src/commerce_process_manager.rs:183] [VERIFIED: crates/example-commerce/src/order.rs:58] |
| V6 Cryptography | no | Phase 10 does not introduce cryptographic operations. [VERIFIED: crates/app/src/commerce_process_manager.rs] |

### Known Threat Patterns for Process-Manager Idempotency

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Duplicate same-product lines replay the wrong prior follow-up result | Tampering | Include line ordinal in reserve/release idempotency keys and test duplicate lines. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| Retry emits new operations instead of replaying committed result | Repudiation / Tampering | Derive keys deterministically from source event and line identity; let runtime/store replay by tenant/key. [VERIFIED: crates/es-runtime/src/shard.rs:187] [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/] |
| Cross-tenant replay collision | Information Disclosure / Tampering | Preserve runtime/store tenant-scoped replay keying and source event tenant propagation. [VERIFIED: crates/es-store-postgres/src/sql.rs:354] [VERIFIED: crates/app/src/commerce_process_manager.rs:191] |
| Compensation step collision | Tampering | Use the same line ordinal identity for release keys as reserve keys. [VERIFIED: crates/app/src/commerce_process_manager.rs:106] |

## Sources

### Primary (HIGH confidence)
- `.planning/REQUIREMENTS.md` - Phase requirement IDs and v1 constraints. [VERIFIED: local file]
- `.planning/ROADMAP.md` - Phase 10 goal, dependencies, success criteria, and Phase 11 boundaries. [VERIFIED: local file]
- `.planning/STATE.md` - prior decisions for runtime/store replay, process-manager deterministic keys, and Phase 9 completion. [VERIFIED: local file]
- `.planning/v1.0-MILESTONE-AUDIT.md` - exact gap evidence for duplicate product-line idempotency collisions. [VERIFIED: local file]
- `crates/app/src/commerce_process_manager.rs` - current workflow, key formats, replay-aware test harness, and app tests. [VERIFIED: local file]
- `crates/es-runtime/src/shard.rs` and `crates/es-runtime/src/command.rs` - runtime replay and command idempotency boundary. [VERIFIED: local file]
- `crates/es-store-postgres/src/sql.rs` and `crates/es-store-postgres/src/event_store.rs` - durable dedupe/replay storage by tenant/idempotency key. [VERIFIED: local file]
- `crates/example-commerce/src/order.rs` and `crates/example-commerce/src/product.rs` - order-line payload shape and inventory command/event semantics. [VERIFIED: local file]
- `cargo test -p app commerce_process_manager -- --nocapture` - baseline 7 app process-manager tests passed during research. [VERIFIED: command output]

### Secondary (MEDIUM confidence)
- AWS Builders Library, "Making retries safe with idempotent APIs" - explicit client request identifier and semantic retry response guidance. [CITED: https://aws.amazon.com/jp/builders-library/making-retries-safe-with-idempotent-APIs/]
- Microsoft Learn, "Compensating Transaction pattern" - compensation steps should be resumable and idempotent. [CITED: https://learn.microsoft.com/en-us/azure/architecture/patterns/compensating-transaction]
- Microsoft Learn, "Saga distributed transactions pattern" - orchestration, compensation, and retryable idempotent transaction framing. [CITED: https://learn.microsoft.com/en-us/azure/architecture/patterns/saga]
- Microservices.io, "Idempotent Consumer pattern" - duplicate message handling remains required even with at-least-once messaging systems. [CITED: https://microservices.io/post/microservices/patterns/2020/10/16/idempotent-consumer.html]

### Tertiary (LOW confidence)
- None used.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new libraries are needed, and local workspace metadata was verified. [VERIFIED: cargo metadata] [VERIFIED: Cargo.toml]
- Architecture: HIGH - source code pinpoints the exact key collision and existing runtime/store replay boundaries. [VERIFIED: crates/app/src/commerce_process_manager.rs:78] [VERIFIED: crates/es-runtime/src/shard.rs:187]
- Pitfalls: HIGH - current tests and audit evidence directly expose missing duplicate-line coverage. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] [VERIFIED: crates/app/src/commerce_process_manager.rs:969]

**Research date:** 2026-04-20  
**Valid until:** 2026-05-20, or until `OrderLine` payload shape, process-manager workflow, or runtime/store idempotency contracts change. [VERIFIED: current local source]

# Phase 4: Commerce Fixture Domain - Research

**Researched:** 2026-04-17 [VERIFIED: environment current_date]
**Domain:** Rust event-sourced commerce fixture aggregates and property/state-sequence testing [VERIFIED: .planning/ROADMAP.md]
**Confidence:** HIGH for local architecture and dependency fit; MEDIUM for optional state-machine-test dependency because Phase 4 can satisfy TEST-01 without adding it. [VERIFIED: local code audit] [VERIFIED: cargo info proptest-state-machine]

## User Constraints

No `04-CONTEXT.md` exists for this phase, so there are no discuss-phase locked decisions to copy. [VERIFIED: `cat .planning/phases/04-commerce-fixture-domain/*-CONTEXT.md` returned no file]

### Locked Decisions

- Rust-first; prefer existing workspace patterns. [VERIFIED: user prompt]
- Domain logic must stay synchronous, deterministic, typed, and free of adapter/database/broker/network dependencies. [VERIFIED: user prompt]
- Event store is source of truth; disruptor is only in-process execution fabric. [VERIFIED: user prompt] [VERIFIED: .planning/STATE.md]
- Avoid shared mutable global business state and adapter-owned aggregate state. [VERIFIED: user prompt] [VERIFIED: .planning/STATE.md]
- Phase 4 should prepare for Phase 5 projections and Phase 6 process-manager/outbox workflows, but not implement those phases. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]
- Package manager preference is `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: AGENTS.md prompt]

### Claude's Discretion

- No separate discretion section exists for this phase. [VERIFIED: no `04-CONTEXT.md` present]

### Deferred Ideas (OUT OF SCOPE)

- Projection runtime, read models, query catch-up, outbox dispatch, and process-manager command issuance belong to later phases. [VERIFIED: .planning/ROADMAP.md]
- Distributed partition ownership is v2/out of scope. [VERIFIED: .planning/STATE.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DOM-01 | Example domain includes `User`, `Product`, and `Order` aggregates or entity models with explicit relationships. [VERIFIED: .planning/REQUIREMENTS.md] | Implement three aggregate roots in `example-commerce`, with order events carrying `UserId` and `ProductId` values by ID rather than object references. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] [VERIFIED: crates/example-commerce/src/lib.rs] |
| DOM-02 | User commands can register, activate/deactivate, and emit replayable user events. [VERIFIED: .planning/REQUIREMENTS.md] | Model user lifecycle as a state-machine aggregate with `Registered`, `Activated`, and `Deactivated` events and deterministic `apply`. [VERIFIED: crates/es-kernel/src/lib.rs] |
| DOM-03 | Product commands can create products, adjust inventory, reserve inventory, and release inventory. [VERIFIED: .planning/REQUIREMENTS.md] | Keep product inventory invariants inside `Product` because negative stock is an aggregate-local invariant. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] |
| DOM-04 | Order commands can place, confirm, reject, and cancel orders referencing user and product identifiers. [VERIFIED: .planning/REQUIREMENTS.md] | Treat order as a document/state-machine aggregate whose commands carry explicit IDs and relationship assumption snapshots supplied by a future process manager or adapter. [VERIFIED: user prompt] [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] |
| DOM-05 | Domain invariants prevent invalid orders, negative inventory, duplicate order placement, and operations against inactive users or unavailable products. [VERIFIED: .planning/REQUIREMENTS.md] | Express invalid states as typed domain errors via `thiserror`, not panics or generic strings. [VERIFIED: crates/example-commerce/src/lib.rs] [CITED: https://docs.rs/thiserror/2.0.18/thiserror/] |
| TEST-01 | Test suite verifies aggregate replay determinism and domain invariants with generated command sequences or equivalent coverage. [VERIFIED: .planning/REQUIREMENTS.md] | Use `proptest` strategies for generated aggregate command/event sequences and save regressions when failures appear. [VERIFIED: Cargo.toml] [CITED: https://docs.rs/proptest/1.11.0/proptest/] |

</phase_requirements>

## Summary

Phase 4 should replace the minimal `ProductDraft` fixture with a compact commerce domain made of three synchronous aggregate roots: `User`, `Product`, and `Order`. [VERIFIED: crates/example-commerce/src/lib.rs] [VERIFIED: .planning/ROADMAP.md] The established event-sourcing pattern is one aggregate instance per stream, commands validate preconditions, decisions emit events, and `apply` rebuilds state deterministically from ordered events. [VERIFIED: crates/es-kernel/src/lib.rs] [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates]

The main design hazard is cross-aggregate invariants. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] Phase 4 must prove relationship assumptions without implementing a process manager, so `OrderCommand::PlaceOrder` should include explicit `UserId`, `ProductId`, and small validated availability snapshots such as `user_active: bool` and per-item `product_available: bool`/quantity assumptions. [VERIFIED: user prompt] [ASSUMED] The order aggregate can reject inactive-user and unavailable-product commands based on those typed assumptions, while product inventory reservation remains owned by the `Product` aggregate and Phase 6 later coordinates the multi-aggregate workflow. [VERIFIED: .planning/ROADMAP.md] [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates]

**Primary recommendation:** Implement domain-owned newtypes, enums, events, replies, typed errors, and aggregate-specific property tests inside `crates/example-commerce`; keep all code synchronous and dependency-light, and use `proptest` command sequences to verify replay determinism plus invariant preservation. [VERIFIED: Cargo.toml] [VERIFIED: crates/es-kernel/src/lib.rs] [CITED: https://docs.rs/proptest/1.11.0/proptest/]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| User lifecycle decisions | Domain kernel / `example-commerce` | Runtime replay orchestration | `Aggregate::decide` and `Aggregate::apply` own deterministic command validation and state mutation. [VERIFIED: crates/es-kernel/src/lib.rs] |
| Product inventory invariants | Domain kernel / `example-commerce` | Runtime replay orchestration | Negative inventory prevention is an aggregate-local invariant and must not rely on adapter state. [VERIFIED: user prompt] [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] |
| Order lifecycle decisions | Domain kernel / `example-commerce` | Future process manager | Order command validation belongs in the aggregate, while cross-aggregate coordination is deferred to Phase 6. [VERIFIED: .planning/ROADMAP.md] |
| Cross-entity relationship references | Domain value objects | Future projections/process manager | Aggregates should reference other aggregates by ID, not object references. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] |
| Event serialization for runtime/storage | Runtime codec boundary | Storage DTOs | `RuntimeEventCodec` encodes typed events into `NewEvent` and decodes `StoredEvent`/snapshots for replay. [VERIFIED: crates/es-runtime/src/command.rs] [VERIFIED: crates/es-store-postgres/src/models.rs] |
| Generated command-sequence testing | Test tier in `example-commerce` | Proptest regression files | `proptest` is already a workspace dev dependency and supports generated inputs plus shrinking. [VERIFIED: Cargo.toml] [CITED: https://docs.rs/proptest/1.11.0/proptest/] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `es-core` | workspace local | Stream IDs, partition keys, expected revisions, tenant and command/event metadata. [VERIFIED: crates/es-core/src/lib.rs] | Reuse existing opaque newtypes and revision contracts instead of duplicating ID/revision types. [VERIFIED: crates/es-core/src/lib.rs] |
| `es-kernel` | workspace local | Synchronous aggregate trait, `Decision`, and replay helper. [VERIFIED: crates/es-kernel/src/lib.rs] | Phase 4 is a domain fixture and should implement the existing aggregate contract directly. [VERIFIED: crates/es-kernel/src/lib.rs] |
| `thiserror` | 2.0.18, published 2026-01-18. [VERIFIED: cargo info thiserror] [VERIFIED: crates.io API] | Typed domain error derives. [CITED: https://docs.rs/thiserror/2.0.18/thiserror/] | The current fixture already uses `thiserror::Error`, and the crate generates standard `Error`/`Display` implementations without making `thiserror` part of the public API contract. [VERIFIED: crates/example-commerce/src/lib.rs] [CITED: https://docs.rs/thiserror/2.0.18/thiserror/] |
| `proptest` | 1.11.0, published 2026-03-24. [VERIFIED: cargo info proptest] [VERIFIED: crates.io API] | Generated event/command sequence tests. [CITED: https://docs.rs/proptest/1.11.0/proptest/] | The workspace already depends on `proptest`, and the project already uses it in the current fixture replay test. [VERIFIED: Cargo.toml] [VERIFIED: crates/example-commerce/src/lib.rs] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `uuid` | Workspace dependency allows `1.23.0`; lockfile resolved `1.23.1`. [VERIFIED: Cargo.toml] [VERIFIED: Cargo.lock] | Test metadata IDs and future event IDs. [VERIFIED: crates/es-core/src/lib.rs] | Use in tests through existing metadata helpers, not inside pure domain decisions unless an ID is part of a command. [VERIFIED: crates/example-commerce/src/lib.rs] |
| `time` | Workspace pinned `=0.3.44`; `cargo info` reports latest `0.3.47`. [VERIFIED: Cargo.toml] [VERIFIED: cargo info time] | Test metadata timestamps. [VERIFIED: crates/example-commerce/src/lib.rs] | Keep only in tests for deterministic `CommandMetadata`; domain decisions should not call wall-clock time. [VERIFIED: crates/es-kernel/src/lib.rs] |
| `proptest-state-machine` | 0.8.0, published 2026-03-24. [VERIFIED: cargo info proptest-state-machine] [VERIFIED: crates.io API] | Optional generated state-machine tests. [CITED: https://proptest-rs.github.io/proptest/proptest/state-machine.html] | Use only if plain `Vec<Command>` strategies become unclear; Phase 4 can satisfy TEST-01 with the existing `proptest` dependency. [VERIFIED: Cargo.toml] [ASSUMED] |
| `serde` / `serde_json` | Workspace `serde` 1.0.228 and `serde_json` 1.0.149. [VERIFIED: Cargo.toml] | Future runtime codec payloads. [VERIFIED: crates/es-store-postgres/src/models.rs] | Do not require serde derives for aggregate tests unless Phase 4 also adds a commerce runtime codec fixture. [VERIFIED: crates/es-runtime/src/command.rs] [ASSUMED] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `proptest` sequences | `proptest-state-machine` | State-machine support provides reference-state transitions and invariant hooks, but it adds a new dev dependency while current Phase 4 requirements can be met with aggregate-local strategies. [CITED: https://proptest-rs.github.io/proptest/proptest/state-machine.html] [VERIFIED: Cargo.toml] [ASSUMED] |
| Domain newtypes/enums | Generic JSON command/event maps | Generic JSON/reflection in the hot path is explicitly out of scope and would undermine typed Rust domain logic. [VERIFIED: .planning/REQUIREMENTS.md] |
| Process manager now | Phase 6 process manager | Phase 6 owns cross-entity workflow reactions and outbox coordination, so Phase 4 should only prepare typed events and relationship IDs. [VERIFIED: .planning/ROADMAP.md] |

**Installation:**

No required dependency installation is needed for the recommended Phase 4 implementation because `thiserror`, `proptest`, `time`, and `uuid` are already workspace dependencies. [VERIFIED: Cargo.toml]

If planner chooses full state-machine tests, add only this dev dependency at workspace level: [VERIFIED: cargo info proptest-state-machine]

```bash
cargo add proptest-state-machine@0.8.0 --workspace --dev
```

**Version verification:**

```bash
cargo info proptest
cargo info proptest-state-machine
cargo info thiserror
cargo info uuid
cargo info time
```

`cargo info` verified `proptest` 1.11.0, `proptest-state-machine` 0.8.0, `thiserror` 2.0.18, `uuid` 1.23.1 as current crate metadata; `time` remains pinned to 0.3.44 although 0.3.47 is latest. [VERIFIED: cargo info proptest] [VERIFIED: cargo info proptest-state-machine] [VERIFIED: cargo info thiserror] [VERIFIED: cargo info uuid] [VERIFIED: cargo info time]

## Architecture Patterns

### System Architecture Diagram

```text
Typed command
  |
  v
Aggregate::stream_id / partition_key / expected_revision
  |
  v
Current state from replay or default
  |
  v
Aggregate::decide(state, command, metadata)
  |
  +--> typed domain error -> reject command
  |
  v
Decision { events, reply }
  |
  v
Aggregate::apply events in order
  |
  v
Replay determinism/property tests

Future integration path:
Committed commerce events -> Phase 5 projections / Phase 6 process manager -> follow-up commands
```

The diagram follows the existing kernel flow where `decide` returns typed events and reply, and `apply` is the sole replay mutation hook. [VERIFIED: crates/es-kernel/src/lib.rs]

### Recommended Project Structure

```text
crates/example-commerce/src/
|-- lib.rs              # public re-exports and crate docs
|-- ids.rs              # UserId, ProductId, OrderId, Sku, Quantity helpers
|-- user.rs             # User aggregate state/commands/events/replies/errors/tests
|-- product.rs          # Product aggregate state/commands/events/replies/errors/tests
|-- order.rs            # Order aggregate state/commands/events/replies/errors/tests
|-- tests.rs            # shared test metadata and property strategy helpers if kept internal
```

Splitting the current single `lib.rs` is appropriate because Phase 4 adds three aggregate roots and generated sequence tests. [VERIFIED: crates/example-commerce/src/lib.rs] [VERIFIED: .planning/ROADMAP.md]

### Pattern 1: Aggregate-Owned State Machine

**What:** Model lifecycle status as a small enum in state, validate command transitions in `decide`, and make `apply` the only state mutator. [VERIFIED: crates/es-kernel/src/lib.rs] [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates]

**When to use:** Use for user active/inactive lifecycle, product availability/inventory lifecycle, and order placed/confirmed/rejected/cancelled lifecycle. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: local kernel contract in crates/es-kernel/src/lib.rs
impl Aggregate for Order {
    type State = OrderState;
    type Command = OrderCommand;
    type Event = OrderEvent;
    type Reply = OrderReply;
    type Error = OrderError;

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        match command {
            OrderCommand::PlaceOrder { order_id, user_id, items, user_active } => {
                if state.placed {
                    return Err(OrderError::AlreadyPlaced);
                }
                if !user_active {
                    return Err(OrderError::InactiveUser { user_id });
                }
                if items.is_empty() {
                    return Err(OrderError::EmptyOrder);
                }
                Ok(Decision::new(
                    vec![OrderEvent::OrderPlaced { order_id: order_id.clone(), user_id, items }],
                    OrderReply::Placed { order_id },
                ))
            }
            _ => todo!("other transitions"),
        }
    }
}
```

The example intentionally keeps relationship facts as command inputs because Phase 4 cannot load user/product aggregates or projections. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]

### Pattern 2: Aggregate References by ID Only

**What:** Order events and state should hold `UserId`, `ProductId`, and line items, not `UserState` or `ProductState` objects. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates]

**When to use:** Use for all cross-entity relationships in this commerce fixture. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: eventsourcing.dev aggregate guidance and local es-core opaque ID style.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UserId(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderLine {
    pub product_id: ProductId,
    pub quantity: Quantity,
}
```

The existing `StreamId`, `PartitionKey`, and `TenantId` types prove the local pattern for validated string-backed newtypes. [VERIFIED: crates/es-core/src/lib.rs]

### Pattern 3: Property Tests for Command Sequences

**What:** Generate legal and illegal command sequences, execute `decide`/`apply` in order, and assert invariants after every accepted event. [CITED: https://docs.rs/proptest/1.11.0/proptest/] [CITED: https://proptest-rs.github.io/proptest/proptest/state-machine.html]

**When to use:** Use for product inventory never negative, duplicate placement rejected, order terminal states stable, and replay state equals online-applied state. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: docs.rs/proptest 1.11.0 and existing fixture test style.
proptest! {
    #[test]
    fn product_inventory_never_goes_negative(commands in product_command_sequence()) {
        let mut state = ProductState::default();
        let mut events = Vec::new();

        for command in commands {
            if let Ok(decision) = Product::decide(&state, command, &metadata()) {
                for event in &decision.events {
                    Product::apply(&mut state, event);
                    events.push(event.clone());
                }
                prop_assert!(state.available_quantity >= 0);
            }
        }

        prop_assert_eq!(state, es_kernel::replay::<Product>(events));
    }
}
```

The current fixture already uses `proptest!`, generated vectors, `replay`, and `prop_assert_eq!`. [VERIFIED: crates/example-commerce/src/lib.rs] [CITED: https://docs.rs/proptest/1.11.0/proptest/]

### Anti-Patterns to Avoid

- **Cross-aggregate object references:** Store IDs and assumption snapshots, not other aggregate states, because aggregate guidance says references should be by ID and the prompt requires explicit identifiers. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] [VERIFIED: user prompt]
- **Adapter/database lookups in domain decisions:** `Aggregate::decide` is synchronous and receives state, command, and metadata only. [VERIFIED: crates/es-kernel/src/lib.rs]
- **Global inventory/user maps in `example-commerce`:** Phase 3 decisions reject global mutable business-state locks, and Phase 4 must avoid adapter-owned aggregate state. [VERIFIED: .planning/STATE.md] [VERIFIED: user prompt]
- **Implementing process-manager behavior in Phase 4:** Phase 6 owns cross-entity workflow reactions and follow-up commands. [VERIFIED: .planning/ROADMAP.md]
- **Panics for business invalid states:** Invalid orders, negative inventory, duplicate placement, inactive users, and unavailable products must return typed domain errors. [VERIFIED: .planning/REQUIREMENTS.md]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Aggregate replay | A custom replay loop per aggregate | `es_kernel::replay::<A>` and each aggregate's `apply` | The kernel already defines ordered replay from default state. [VERIFIED: crates/es-kernel/src/lib.rs] |
| Domain error formatting | Manual `Display`/`Error` impls | `thiserror::Error` | The fixture already uses it, and docs confirm it derives standard error/display behavior. [VERIFIED: crates/example-commerce/src/lib.rs] [CITED: https://docs.rs/thiserror/2.0.18/thiserror/] |
| Property input shrinking | Custom random loops | `proptest` strategies/macros | Proptest provides strategies, macros, and shrinking support. [CITED: https://docs.rs/proptest/1.11.0/proptest/] [CITED: https://github.com/proptest-rs/proptest] |
| Cross-aggregate workflow coordination | Manual in-memory saga inside domain aggregate | Phase 6 process manager/outbox workflow | Roadmap assigns process managers and outbox to Phase 6. [VERIFIED: .planning/ROADMAP.md] |
| Durable idempotency/duplicate append handling | In-domain dedupe store | Runtime/storage idempotency key and event store dedupe | Runtime envelopes and storage append models already own idempotency keys and duplicate append outcomes. [VERIFIED: crates/es-runtime/src/command.rs] [VERIFIED: crates/es-store-postgres/src/models.rs] |
| Event payload schema enforcement | Ad hoc stringly events | Typed event enums now; future runtime codec maps to `NewEvent` | Storage DTOs already require stable `event_type`, positive `schema_version`, payload, and metadata. [VERIFIED: crates/es-store-postgres/src/models.rs] |

**Key insight:** Phase 4 should hand-roll the domain model because that is the point of the fixture, but it should not hand-roll testing infrastructure, replay mechanics, error traits, storage idempotency, projection, or process-manager coordination. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: crates/es-kernel/src/lib.rs] [VERIFIED: Cargo.toml]

## Common Pitfalls

### Pitfall 1: Treating Order as a Transaction Across User and Product

**What goes wrong:** The order aggregate attempts to mutate user status or product inventory directly. [ASSUMED]
**Why it happens:** Commerce workflows naturally span user, inventory, and order concepts, but event-sourced aggregate guidance treats each aggregate as a consistency boundary. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates]
**How to avoid:** Phase 4 should accept relationship assumptions as command data and emit explicit events that later projections/process managers can observe. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]
**Warning signs:** `OrderState` contains `UserState`, `ProductState`, or mutable inventory maps. [ASSUMED]

### Pitfall 2: Hiding Invalid States Behind Saturating Arithmetic

**What goes wrong:** Inventory adjustments use `saturating_sub` or clamp values to zero, masking invalid reservations/releases. [ASSUMED]
**Why it happens:** Arithmetic helpers can make negative inventory impossible by construction while losing the domain error. [ASSUMED]
**How to avoid:** Validate quantities before emitting events and return `ProductError::InsufficientInventory` or `InvalidQuantity` variants. [VERIFIED: .planning/REQUIREMENTS.md]
**Warning signs:** Tests only assert final quantity and do not assert the typed error for rejected operations. [ASSUMED]

### Pitfall 3: Replay Tests That Do Not Exercise Decisions

**What goes wrong:** Tests replay arbitrary events but never prove commands reject invalid states. [VERIFIED: crates/example-commerce/src/lib.rs]
**Why it happens:** Replaying event vectors is easier than generating command sequences. [ASSUMED]
**How to avoid:** Keep replay equivalence tests, but add command-sequence tests that call `decide`, apply accepted events, and assert errors for rejected commands. [CITED: https://docs.rs/proptest/1.11.0/proptest/] [VERIFIED: .planning/REQUIREMENTS.md]
**Warning signs:** `proptest` only generates `Event` values and no `Command` values. [VERIFIED: crates/example-commerce/src/lib.rs]

### Pitfall 4: Event Names Not Ready for Storage Codecs

**What goes wrong:** Events have Rust enum variants but no stable external event type names or schema versions for future codec work. [ASSUMED]
**Why it happens:** Phase 4 does not persist events directly, but Phase 3 runtime codecs and Phase 2 storage DTOs already expect stable event names and positive schema versions. [VERIFIED: crates/es-runtime/src/command.rs] [VERIFIED: crates/es-store-postgres/src/models.rs]
**How to avoid:** Define helper constants or methods like `OrderEvent::event_type()` and `schema_version()` if Phase 4 adds a commerce codec fixture; otherwise document exact variant names for Phase 5/6. [ASSUMED]
**Warning signs:** Event variant names change without tests or no test asserts event type mapping. [ASSUMED]

### Pitfall 5: Adding Async or Storage Dependencies to `example-commerce`

**What goes wrong:** Domain commands perform database reads or async calls. [VERIFIED: user prompt]
**Why it happens:** Relationship validation can tempt direct user/product lookups during order placement. [ASSUMED]
**How to avoid:** Keep `example-commerce` dependencies limited to `es-core`, `es-kernel`, and `thiserror`, with testing-only `proptest`/`uuid`/`time`. [VERIFIED: crates/example-commerce/Cargo.toml]
**Warning signs:** `tokio`, `sqlx`, `disruptor`, `axum`, or broker crates appear in `example-commerce` dependency tree. [VERIFIED: crates/example-commerce/tests/dependency_boundaries.rs]

## Code Examples

Verified patterns from official/local sources:

### Typed Aggregate Skeleton

```rust
// Source: crates/es-kernel/src/lib.rs
pub trait Aggregate {
    type State: Default + Clone + PartialEq;
    type Command;
    type Event: Clone;
    type Reply;
    type Error;

    fn stream_id(command: &Self::Command) -> es_core::StreamId;
    fn partition_key(command: &Self::Command) -> es_core::PartitionKey;
    fn expected_revision(command: &Self::Command) -> es_core::ExpectedRevision;
    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &es_core::CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error>;
    fn apply(state: &mut Self::State, event: &Self::Event);
}
```

This is the contract Phase 4 aggregates must implement. [VERIFIED: crates/es-kernel/src/lib.rs]

### Typed Error Variants

```rust
// Source: docs.rs/thiserror 2.0.18 and current ProductError fixture.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProductError {
    #[error("quantity must be positive")]
    InvalidQuantity,
    #[error("insufficient inventory: available {available}, requested {requested}")]
    InsufficientInventory { available: u32, requested: u32 },
}
```

`thiserror` supports enum error derives and field interpolation in messages. [CITED: https://docs.rs/thiserror/2.0.18/thiserror/]

### State Sequence Test Shape

```rust
// Source: docs.rs/proptest 1.11.0 and current fixture property test.
proptest! {
    #[test]
    fn order_sequence_is_replayable(commands in order_command_sequence()) {
        let mut state = OrderState::default();
        let mut events = Vec::new();

        for command in commands {
            if let Ok(decision) = Order::decide(&state, command, &metadata()) {
                for event in &decision.events {
                    Order::apply(&mut state, event);
                    events.push(event.clone());
                }
            }
        }

        prop_assert_eq!(state, es_kernel::replay::<Order>(events));
    }
}
```

The pattern extends the existing fixture's replay-vs-manual-application property from generated events to generated command sequences. [VERIFIED: crates/example-commerce/src/lib.rs]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| QuickCheck-style type-level generators for all property tests | `proptest` strategies with per-value generation/shrinking | `proptest` 1.11.0 is current crate metadata as of 2026-04-17. [VERIFIED: cargo info proptest] | Use explicit strategies for quantities, lifecycle commands, and valid/invalid order lines. [CITED: https://github.com/proptest-rs/proptest] |
| Hand-written stateful random test loops | `proptest-state-machine` reference-state-machine tests for complex stateful APIs | `proptest-state-machine` 0.8.0 was published 2026-03-24. [VERIFIED: cargo info proptest-state-machine] [VERIFIED: crates.io API] | Keep in reserve; plain `proptest` command sequences are sufficient for this compact fixture unless tests become hard to reason about. [ASSUMED] |
| Aggregate models designed around data relationships | Aggregate models designed around invariants and ID references | Current event-sourcing aggregate guidance. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] | Do not put user/product aggregate objects inside order state. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] |

**Deprecated/outdated:**

- Using generic JSON/reflection rules in the hot path is out of scope for this project. [VERIFIED: .planning/REQUIREMENTS.md]
- Making command success depend on projection freshness is out of scope because CQRS projections are eventually consistent by design in this roadmap. [VERIFIED: .planning/REQUIREMENTS.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `OrderCommand::PlaceOrder` should carry relationship assumption snapshots such as `user_active` and per-line product availability because Phase 4 cannot implement projections/process managers. | Summary / Architecture Patterns | Planner may instead choose separate prevalidated command types or fixture services; either still must keep domain synchronous and storage-free. |
| A2 | Phase 4 can satisfy TEST-01 with existing `proptest` command sequences without adding `proptest-state-machine`. | Standard Stack / State of the Art | Planner may add a dependency for more formal state-machine tests, increasing scope but improving structure. |
| A3 | If Phase 4 adds a commerce runtime codec fixture, events should expose stable event type/schema helpers now. | Common Pitfalls | Planner may defer codec mapping to Phase 5/6, leaving event type mapping untested until later. |
| A4 | Saturating arithmetic is a likely implementation mistake for inventory errors. | Common Pitfalls | Planner may not need explicit checks if a `Quantity` newtype makes invalid arithmetic unrepresentable. |

## Open Questions (RESOLVED)

1. **Should Phase 4 add `proptest-state-machine` now?** [VERIFIED: cargo info proptest-state-machine]
   - What we know: The crate is current at 0.8.0 and its docs describe reference-state-machine tests, transition generation, preconditions, postconditions, and invariant checks. [CITED: https://proptest-rs.github.io/proptest/proptest/state-machine.html]
   - RESOLVED: Phase 4 will use the existing workspace `proptest` dependency only. Do not add `proptest-state-machine` in Phase 4. [VERIFIED: .planning/phases/04-commerce-fixture-domain/04-04-PLAN.md] [VERIFIED: Cargo.toml]
   - Decision rationale: The current plans satisfy TEST-01 with generated command-sequence tests in `crates/example-commerce/src/tests.rs` and module-local product sequence tests, so the optional state-machine helper would add dependency and scope without being required by the planned implementation. [VERIFIED: .planning/phases/04-commerce-fixture-domain/04-03-PLAN.md] [VERIFIED: .planning/phases/04-commerce-fixture-domain/04-04-PLAN.md]

2. **Should commerce events derive `serde` in Phase 4?** [VERIFIED: Cargo.toml]
   - What we know: Storage DTOs persist JSON payloads and runtime codecs encode/decode typed events. [VERIFIED: crates/es-store-postgres/src/models.rs] [VERIFIED: crates/es-runtime/src/command.rs]
   - RESOLVED: Do not add `serde` derives to commerce commands/events in Phase 4 unless a commerce runtime codec fixture is explicitly added to a plan. [VERIFIED: .planning/phases/04-commerce-fixture-domain/04-01-PLAN.md] [VERIFIED: .planning/phases/04-commerce-fixture-domain/04-04-PLAN.md]
   - Decision rationale: The existing Phase 4 plans keep the fixture as synchronous domain logic and generated aggregate tests; persistence codecs remain at the runtime/storage boundary and are not part of this phase's planned artifacts. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: crates/es-runtime/src/command.rs]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain | Build/test Phase 4 workspace | yes [VERIFIED: rustc --version] | `rustc 1.85.1` [VERIFIED: rustc --version] | None needed because version meets workspace `rust-version = "1.85"`. [VERIFIED: Cargo.toml] |
| Cargo | Dependency metadata and tests | yes [VERIFIED: cargo --version] | `cargo 1.85.1` [VERIFIED: cargo --version] | None needed. [VERIFIED: cargo --version] |
| Crates.io network access | Version checks and optional dependency add | yes [VERIFIED: cargo info proptest] | n/a | Use existing workspace dependencies if network unavailable. [VERIFIED: Cargo.toml] |

**Missing dependencies with no fallback:** None found for Phase 4 research and current test execution. [VERIFIED: cargo test -p example-commerce]

**Missing dependencies with fallback:** `proptest-state-machine` is not in workspace dependencies, but it is optional and existing `proptest` is available. [VERIFIED: Cargo.toml] [VERIFIED: cargo info proptest-state-machine]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Cargo test with Rust unit/integration tests and `proptest` 1.11.0. [VERIFIED: Cargo.toml] [VERIFIED: cargo info proptest] |
| Config file | No separate test config; workspace uses root `Cargo.toml`, `rust-toolchain.toml`, and crate-local tests. [VERIFIED: Cargo.toml] [VERIFIED: rust-toolchain.toml] |
| Quick run command | `cargo test -p example-commerce` [VERIFIED: command executed] |
| Full suite command | `cargo test --workspace` [VERIFIED: Cargo.toml workspace members] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| DOM-01 | User/Product/Order aggregate roots and explicit relationships | unit + dependency boundary | `cargo test -p example-commerce` | partial: current fixture only has ProductDraft. [VERIFIED: crates/example-commerce/src/lib.rs] |
| DOM-02 | Register/activate/deactivate user emits replayable events | unit + property replay | `cargo test -p example-commerce user::` | no; Wave 0 should create `user.rs`. [VERIFIED: crates/example-commerce/src/lib.rs] |
| DOM-03 | Product create/adjust/reserve/release rejects negative inventory | unit + property command sequence | `cargo test -p example-commerce product::` | partial; current ProductDraft only creates product. [VERIFIED: crates/example-commerce/src/lib.rs] |
| DOM-04 | Order place/confirm/reject/cancel references user/product IDs | unit + property command sequence | `cargo test -p example-commerce order::` | no; Wave 0 should create `order.rs`. [VERIFIED: crates/example-commerce/src/lib.rs] |
| DOM-05 | Invalid orders and unavailable/inactive/duplicate cases return typed errors | unit + property command sequence | `cargo test -p example-commerce` | partial; current tests cover empty SKU/name and duplicate create only. [VERIFIED: crates/example-commerce/src/lib.rs] |
| TEST-01 | Replay determinism and invariants with generated sequences | property | `cargo test -p example-commerce replay` | partial; current property replays generated product events, not command sequences. [VERIFIED: crates/example-commerce/src/lib.rs] |

### Sampling Rate

- **Per task commit:** `cargo test -p example-commerce` [VERIFIED: command executed]
- **Per wave merge:** `cargo test --workspace` [VERIFIED: Cargo.toml workspace members]
- **Phase gate:** Full workspace suite green before `/gsd-verify-work`. [VERIFIED: GSD workflow prompt]

### Wave 0 Gaps

- [ ] `crates/example-commerce/src/user.rs` - covers DOM-01 and DOM-02. [VERIFIED: crates/example-commerce/src/lib.rs]
- [ ] `crates/example-commerce/src/product.rs` - expands ProductDraft into inventory/availability behavior for DOM-01, DOM-03, and DOM-05. [VERIFIED: crates/example-commerce/src/lib.rs]
- [ ] `crates/example-commerce/src/order.rs` - covers DOM-01, DOM-04, and DOM-05. [VERIFIED: crates/example-commerce/src/lib.rs]
- [ ] Command-sequence property tests for product and order invariants - covers TEST-01. [VERIFIED: .planning/REQUIREMENTS.md]
- [ ] Optional `ids.rs` newtypes for `UserId`, `ProductId`, `OrderId`, `Sku`, and positive `Quantity`. [VERIFIED: crates/es-core/src/lib.rs] [ASSUMED]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | No authentication boundary is implemented in Phase 4. [VERIFIED: .planning/ROADMAP.md] |
| V3 Session Management | no | No session handling is implemented in Phase 4. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | partial | Tenant metadata exists in `CommandMetadata`, but Phase 4 domain tests should not implement authorization. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: .planning/ROADMAP.md] |
| V5 Input Validation | yes | Validate domain newtypes and command preconditions with constructors plus typed errors. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: crates/example-commerce/src/lib.rs] |
| V6 Cryptography | no | Phase 4 does not implement cryptography. [VERIFIED: .planning/ROADMAP.md] |

### Known Threat Patterns for Rust Domain Fixture

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Invalid command payload creates impossible state | Tampering | Constructor validation, command precondition checks, and typed errors. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: crates/example-commerce/src/lib.rs] |
| Duplicate order placement | Tampering / Repudiation | Aggregate state rejects second placement; runtime/storage idempotency remains separate. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: crates/es-runtime/src/command.rs] |
| Cross-tenant state leakage | Information disclosure | Domain code should not store global maps; tenant lives in metadata handled by runtime/storage. [VERIFIED: crates/es-core/src/lib.rs] [VERIFIED: user prompt] |
| Non-deterministic decisions | Tampering / Repudiation | Keep `decide` synchronous and do not call clock/random/network/storage from domain decisions. [VERIFIED: crates/es-kernel/src/lib.rs] [VERIFIED: user prompt] |

## Sources

### Primary (HIGH confidence)

- `.planning/REQUIREMENTS.md` - Phase 4 requirements DOM-01 through DOM-05 and TEST-01. [VERIFIED: .planning/REQUIREMENTS.md]
- `.planning/ROADMAP.md` - Phase 4 goal, success criteria, dependencies, and later Phase 5/6 ownership. [VERIFIED: .planning/ROADMAP.md]
- `.planning/STATE.md` - prior decisions on source-of-truth, runtime, and deferred distributed ownership. [VERIFIED: .planning/STATE.md]
- `Cargo.toml` - workspace edition, Rust floor, dependencies, and lints. [VERIFIED: Cargo.toml]
- `crates/example-commerce/src/lib.rs` - current ProductDraft aggregate and existing tests. [VERIFIED: crates/example-commerce/src/lib.rs]
- `crates/es-kernel/src/lib.rs` - aggregate contract and replay helper. [VERIFIED: crates/es-kernel/src/lib.rs]
- `crates/es-core/src/lib.rs` - ID/revision/metadata contracts. [VERIFIED: crates/es-core/src/lib.rs]
- `crates/es-runtime/src/command.rs` - runtime command envelope and codec boundary. [VERIFIED: crates/es-runtime/src/command.rs]
- `crates/es-store-postgres/src/models.rs` - storage append/stored event DTOs. [VERIFIED: crates/es-store-postgres/src/models.rs]
- `cargo info proptest`, `cargo info proptest-state-machine`, `cargo info thiserror`, `cargo info uuid`, `cargo info time` - current crate metadata. [VERIFIED: cargo info]
- crates.io API version endpoints for `proptest`, `proptest-state-machine`, and `thiserror` - publish timestamps. [VERIFIED: crates.io API]

### Secondary (MEDIUM confidence)

- https://docs.rs/proptest/1.11.0/proptest/ - proptest reference docs and macro/strategy APIs. [CITED: https://docs.rs/proptest/1.11.0/proptest/]
- https://github.com/proptest-rs/proptest - proptest README, property testing overview, shrinking, limitations, and QuickCheck comparison. [CITED: https://github.com/proptest-rs/proptest]
- https://proptest-rs.github.io/proptest/proptest/state-machine.html - official proptest book state-machine testing chapter. [CITED: https://proptest-rs.github.io/proptest/proptest/state-machine.html]
- https://docs.rs/thiserror/2.0.18/thiserror/ - thiserror derive behavior. [CITED: https://docs.rs/thiserror/2.0.18/thiserror/]
- https://www.eventsourcing.dev/best-practices/designing-aggregates - aggregate design guidance. [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates]

### Tertiary (LOW confidence)

- None used as authoritative sources. [VERIFIED: source list audit]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH - workspace dependencies and crate metadata were verified locally and via registry metadata. [VERIFIED: Cargo.toml] [VERIFIED: cargo info]
- Architecture: HIGH - local kernel/runtime/storage boundaries directly define how Phase 4 aggregates should attach. [VERIFIED: crates/es-kernel/src/lib.rs] [VERIFIED: crates/es-runtime/src/command.rs] [VERIFIED: crates/es-store-postgres/src/models.rs]
- Pitfalls: MEDIUM - cross-aggregate and testing pitfalls are supported by local constraints and event-sourcing guidance, while some inventory-specific mistakes are inferred. [VERIFIED: user prompt] [CITED: https://www.eventsourcing.dev/best-practices/designing-aggregates] [ASSUMED]

**Research date:** 2026-04-17 [VERIFIED: environment current_date]
**Valid until:** 2026-05-17 for local architecture; 2026-04-24 for crate-version freshness. [ASSUMED]

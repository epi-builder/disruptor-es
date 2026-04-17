# Phase 04: Commerce Fixture Domain - Pattern Map

**Mapped:** 2026-04-17
**Files analyzed:** 6
**Analogs found:** 6 / 6

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/example-commerce/src/lib.rs` | config | transform | `crates/example-commerce/src/lib.rs` | role-match |
| `crates/example-commerce/src/ids.rs` | model | transform | `crates/es-core/src/lib.rs` | role-match |
| `crates/example-commerce/src/user.rs` | model | event-driven | `crates/example-commerce/src/lib.rs` | exact |
| `crates/example-commerce/src/product.rs` | model | event-driven | `crates/example-commerce/src/lib.rs` | exact |
| `crates/example-commerce/src/order.rs` | model | event-driven | `crates/example-commerce/src/lib.rs` | exact |
| `crates/example-commerce/src/tests.rs` | test | batch | `crates/example-commerce/src/lib.rs` | role-match |

## Pattern Assignments

### `crates/example-commerce/src/lib.rs` (config, transform)

**Analog:** `crates/example-commerce/src/lib.rs`

Use this file as the public module facade after splitting the existing single-file fixture into focused modules. Keep crate-level docs and public re-exports here; move aggregate implementation bodies into `user.rs`, `product.rs`, and `order.rs`.

**Imports/dependency boundary pattern** (`crates/example-commerce/src/lib.rs` lines 1-5):
```rust
//! Minimal commerce aggregate fixture for the typed event-sourcing kernel.

use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};
```

**Current aggregate colocated pattern to split** (`crates/example-commerce/src/lib.rs` lines 6-20):
```rust
/// Product draft aggregate marker.
pub struct ProductDraft;

/// Product draft state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductState {
    /// Created product SKU, if any.
    pub sku: Option<String>,
    /// Created product name, if any.
    pub name: Option<String>,
}

/// Commands accepted by the product draft aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductCommand {
```

**Recommended module facade shape to implement:**
```rust
//! Commerce fixture aggregates for the typed event-sourcing kernel.

mod ids;
mod order;
mod product;
mod user;

pub use ids::{OrderId, ProductId, Quantity, Sku, UserId};
pub use order::{Order, OrderCommand, OrderError, OrderEvent, OrderReply, OrderState};
pub use product::{Product, ProductCommand, ProductError, ProductEvent, ProductReply, ProductState};
pub use user::{User, UserCommand, UserError, UserEvent, UserReply, UserState};

#[cfg(test)]
mod tests;
```

**Dependency boundary test to preserve** (`crates/example-commerce/tests/dependency_boundaries.rs` lines 6-15):
```rust
const FORBIDDEN_DEPENDENCIES: &[&str] = &[
    "tokio",
    "sqlx",
    "axum",
    "tonic",
    "async-nats",
    "rdkafka",
    "postgres",
    "disruptor",
];
```

### `crates/example-commerce/src/ids.rs` (model, transform)

**Analog:** `crates/es-core/src/lib.rs`

Create domain ID/value-object newtypes with the same string-backed constructor pattern as `StreamId`, `PartitionKey`, and `TenantId`. Use these for `UserId`, `ProductId`, `OrderId`, and `Sku`. Use an integer-backed `Quantity` with checked constructors so inventory invariants cannot be bypassed by raw signed values.

**Imports pattern** (`crates/es-core/src/lib.rs` lines 3-5):
```rust
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
```

For `ids.rs`, do not copy `serde`, `time`, or `uuid` unless implementation needs serialization in this phase. The existing commerce crate currently imports only kernel/core traits and `thiserror`; research says no new dependency is required.

**String-backed newtype pattern** (`crates/es-core/src/lib.rs` lines 18-37):
```rust
/// Durable event stream identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct StreamId(String);

impl StreamId {
    /// Creates a stream identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "StreamId").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}
```

**Shared constructor helper/error pattern** (`crates/es-core/src/lib.rs` lines 7-16 and 81-87):
```rust
/// Errors returned by core value constructors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CoreError {
    /// A required string-backed value was empty.
    #[error("{type_name} cannot be empty")]
    EmptyValue {
        /// Name of the value type that rejected the empty input.
        type_name: &'static str,
    },
}

fn string_value(value: impl Into<String>, type_name: &'static str) -> Result<String, CoreError> {
    let value = value.into();
    if value.is_empty() {
        return Err(CoreError::EmptyValue { type_name });
    }
    Ok(value)
}
```

**Expected local adaptation:**
```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UserId(String);

impl UserId {
    pub fn new(value: impl Into<String>) -> Result<Self, CommerceIdError> {
        string_value(value, "UserId").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

### `crates/example-commerce/src/user.rs` (model, event-driven)

**Analog:** `crates/example-commerce/src/lib.rs`

Copy the existing aggregate shape: marker struct, state, command enum, event enum, reply enum, typed `thiserror` error enum, `Aggregate` implementation, and focused tests. User commands should cover register, activate, and deactivate. State should record registered identity and lifecycle status. `apply` must be the only state mutator.

**Imports pattern** (`crates/example-commerce/src/lib.rs` lines 3-4):
```rust
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};
```

**Typed command/event/reply/error pattern** (`crates/example-commerce/src/lib.rs` lines 18-66):
```rust
/// Commands accepted by the product draft aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductCommand {
    /// Creates a new product draft.
    CreateProduct {
        /// Stream that owns the product draft.
        stream_id: StreamId,
        /// Product SKU.
        sku: String,
        /// Product display name.
        name: String,
    },
}

/// Events emitted by the product draft aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {
    /// Product draft was created.
    ProductCreated {
        /// Product SKU.
        sku: String,
        /// Product display name.
        name: String,
    },
}

/// Product command validation errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProductError {
    /// SKU must not be empty.
    #[error("product SKU cannot be empty")]
    EmptySku,
}
```

**Aggregate routing/concurrency pattern** (`crates/example-commerce/src/lib.rs` lines 68-91):
```rust
impl Aggregate for ProductDraft {
    type State = ProductState;
    type Command = ProductCommand;
    type Event = ProductEvent;
    type Reply = ProductReply;
    type Error = ProductError;

    fn stream_id(command: &Self::Command) -> StreamId {
        match command {
            ProductCommand::CreateProduct { stream_id, .. } => stream_id.clone(),
        }
    }

    fn partition_key(command: &Self::Command) -> PartitionKey {
        match command {
            ProductCommand::CreateProduct { stream_id, .. } => {
                PartitionKey::new(stream_id.as_str()).expect("stream id is a valid partition key")
            }
        }
    }

    fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
        ExpectedRevision::NoStream
    }
}
```

**Core decide/apply pattern** (`crates/example-commerce/src/lib.rs` lines 93-130):
```rust
fn decide(
    state: &Self::State,
    command: Self::Command,
    _metadata: &CommandMetadata,
) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
    if state.sku.is_some() {
        return Err(ProductError::AlreadyCreated);
    }

    match command {
        ProductCommand::CreateProduct {
            stream_id,
            sku,
            name,
        } => {
            if sku.is_empty() {
                return Err(ProductError::EmptySku);
            }
            if name.is_empty() {
                return Err(ProductError::EmptyName);
            }

            Ok(Decision::new(
                vec![ProductEvent::ProductCreated { sku, name }],
                ProductReply::Created { stream_id },
            ))
        }
    }
}

fn apply(state: &mut Self::State, event: &Self::Event) {
    match event {
        ProductEvent::ProductCreated { sku, name } => {
            state.sku = Some(sku.clone());
            state.name = Some(name.clone());
        }
    }
}
```

**User-specific adaptation notes:**
- `RegisterUser` should use `ExpectedRevision::NoStream`.
- `ActivateUser` and `DeactivateUser` should use `ExpectedRevision::Any` unless commands carry an exact revision.
- Reject duplicate registration, activation before registration, duplicate activation, deactivation before registration, and duplicate deactivation with typed `UserError` variants.
- Use `StreamId`/`PartitionKey` derived from `UserId` consistently.

### `crates/example-commerce/src/product.rs` (model, event-driven)

**Analog:** `crates/example-commerce/src/lib.rs`

This file is the direct successor to the current `ProductDraft` fixture. Expand it from create-only product drafts to product lifecycle plus inventory changes. Preserve the existing command validation, typed error, `Decision::new`, and replay patterns.

**Current product state/command/event pattern** (`crates/example-commerce/src/lib.rs` lines 6-42):
```rust
/// Product draft aggregate marker.
pub struct ProductDraft;

/// Product draft state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductState {
    /// Created product SKU, if any.
    pub sku: Option<String>,
    /// Created product name, if any.
    pub name: Option<String>,
}

/// Events emitted by the product draft aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {
    /// Product draft was created.
    ProductCreated {
        /// Product SKU.
        sku: String,
        /// Product display name.
        name: String,
    },
}
```

**Validation/error handling pattern** (`crates/example-commerce/src/lib.rs` lines 54-66 and 98-113):
```rust
/// Product command validation errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProductError {
    /// SKU must not be empty.
    #[error("product SKU cannot be empty")]
    EmptySku,
    /// Product draft has already been created.
    #[error("product draft already exists")]
    AlreadyCreated,
}

if state.sku.is_some() {
    return Err(ProductError::AlreadyCreated);
}

if sku.is_empty() {
    return Err(ProductError::EmptySku);
}
```

**Product-specific adaptation notes:**
- Commands: `CreateProduct`, `AdjustInventory`, `ReserveInventory`, `ReleaseInventory`.
- Events: `ProductCreated`, `InventoryAdjusted`, `InventoryReserved`, `InventoryReleased`.
- State should track at least `product_id`, `sku`, `name`, `available_quantity`, and `reserved_quantity`.
- Reject creation with empty SKU/name, inventory adjustment that would make available inventory negative, reserve above available inventory, release above reserved inventory, and inventory operations before creation.
- Use typed `Quantity` rather than raw signed values at public command boundaries; convert deltas explicitly where needed.

### `crates/example-commerce/src/order.rs` (model, event-driven)

**Analog:** `crates/example-commerce/src/lib.rs`; relationship IDs use `crates/es-core/src/lib.rs`

Implement order as a state-machine aggregate with explicit cross-aggregate references. Do not store `UserState` or `ProductState` inside the order. Commands should carry relationship assumption snapshots needed by later process-manager work: user active, product available, requested quantity, and product IDs.

**Aggregate trait contract to copy** (`crates/es-kernel/src/lib.rs` lines 19-49):
```rust
/// Deterministic aggregate contract implemented by domain aggregate types.
pub trait Aggregate {
    /// Aggregate state type.
    type State: Default + Clone + PartialEq;
    /// Command input type.
    type Command;
    /// Event output type.
    type Event: Clone;
    /// Reply type returned after a successful decision.
    type Reply;
    /// Domain error type returned by decisions.
    type Error;

    /// Returns the stream identifier affected by the command.
    fn stream_id(command: &Self::Command) -> es_core::StreamId;

    /// Returns the ordered partition key for the command.
    fn partition_key(command: &Self::Command) -> es_core::PartitionKey;

    /// Returns the expected stream revision for optimistic concurrency.
    fn expected_revision(command: &Self::Command) -> es_core::ExpectedRevision;

    /// Decides events and reply for a command against current state.
    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &es_core::CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error>;

    /// Applies one event to aggregate state.
    fn apply(state: &mut Self::State, event: &Self::Event);
}
```

**ID reference pattern** (`crates/es-core/src/lib.rs` lines 18-31 and 39-52):
```rust
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct StreamId(String);

impl StreamId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "StreamId").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PartitionKey(String);

impl PartitionKey {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        string_value(value, "PartitionKey").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

**Order-specific adaptation notes:**
- Commands: `PlaceOrder`, `ConfirmOrder`, `RejectOrder`, `CancelOrder`.
- Events: `OrderPlaced`, `OrderConfirmed`, `OrderRejected`, `OrderCancelled`.
- `PlaceOrder` should include `OrderId`, `UserId`, non-empty line items, and relationship assumptions such as `user_active` and per-line product availability.
- Reject empty orders, inactive user assumptions, unavailable product assumptions, duplicate placement, and invalid terminal-state transitions with typed `OrderError` variants.
- Apply events into a compact status enum, for example `Draft`/`Placed`/`Confirmed`/`Rejected`/`Cancelled`.

### `crates/example-commerce/src/tests.rs` (test, batch)

**Analog:** `crates/example-commerce/src/lib.rs`

Use this file for shared test metadata helpers and property strategy helpers if tests are split out of aggregate modules. It should stay behind `#[cfg(test)]` through the `lib.rs` module declaration. The closest current pattern is the in-file `aggregate_contract` module.

**Test imports and deterministic metadata helper** (`crates/example-commerce/src/lib.rs` lines 133-149):
```rust
#[cfg(test)]
mod aggregate_contract {
    use super::*;
    use es_core::TenantId;
    use proptest::prelude::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: None,
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        }
    }
}
```

**Unit test pattern for decision outputs** (`crates/example-commerce/src/lib.rs` lines 159-178):
```rust
#[test]
fn decide_valid_create_returns_event_and_reply() {
    let command = create_command("SKU-1", "Keyboard");
    let decision =
        ProductDraft::decide(&ProductState::default(), command, &metadata()).expect("decision");

    assert_eq!(
        vec![ProductEvent::ProductCreated {
            sku: "SKU-1".to_owned(),
            name: "Keyboard".to_owned(),
        }],
        decision.events
    );
    assert_eq!(
        ProductReply::Created {
            stream_id: StreamId::new("product-1").expect("stream id"),
        },
        decision.reply
    );
}
```

**Unit test pattern for typed rejections** (`crates/example-commerce/src/lib.rs` lines 180-200):
```rust
#[test]
fn decide_rejects_empty_sku_and_name() {
    assert_eq!(
        ProductError::EmptySku,
        ProductDraft::decide(
            &ProductState::default(),
            create_command("", "Keyboard"),
            &metadata()
        )
        .expect_err("empty sku")
    );

    assert_eq!(
        ProductError::EmptyName,
        ProductDraft::decide(
            &ProductState::default(),
            create_command("SKU-1", ""),
            &metadata()
        )
        .expect_err("empty name")
    );
}
```

**Property replay pattern** (`crates/example-commerce/src/lib.rs` lines 221-237):
```rust
proptest! {
    #[test]
    fn replay_matches_manual_application(events in prop::collection::vec(("[A-Z0-9]{1,8}", "[A-Za-z0-9 ]{1,24}"), 1..16)) {
        let events: Vec<ProductEvent> = events
            .into_iter()
            .map(|(sku, name)| ProductEvent::ProductCreated { sku, name })
            .collect();

        let replayed = es_kernel::replay::<ProductDraft>(events.clone());
        let mut manually_applied = ProductState::default();
        for event in &events {
            ProductDraft::apply(&mut manually_applied, event);
        }

        prop_assert_eq!(manually_applied, replayed);
    }
}
```

**Required Phase 4 test coverage adaptation:**
- Add generated command/event sequence tests for `User`, `Product`, and `Order`.
- Assert replay state equals manually applied state.
- Assert product available/reserved inventory never goes negative after accepted events.
- Assert duplicate order placement and invalid terminal transitions are rejected.
- Assert inactive user and unavailable product assumptions reject `PlaceOrder`.

## Shared Patterns

### Aggregate Contract

**Source:** `crates/es-kernel/src/lib.rs`
**Apply to:** `user.rs`, `product.rs`, `order.rs`

```rust
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

### Decision Construction

**Source:** `crates/es-kernel/src/lib.rs`
**Apply to:** All aggregate `decide` implementations

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision<E, R> {
    pub events: Vec<E>,
    pub reply: R,
}

impl<E, R> Decision<E, R> {
    pub fn new(events: Vec<E>, reply: R) -> Self {
        Self { events, reply }
    }
}
```

### Replay Determinism

**Source:** `crates/es-kernel/src/lib.rs`
**Apply to:** `tests.rs` and aggregate module tests

```rust
pub fn replay<A: Aggregate>(events: impl IntoIterator<Item = A::Event>) -> A::State {
    let mut state = A::State::default();
    for event in events {
        A::apply(&mut state, &event);
    }
    state
}
```

### Domain Error Handling

**Source:** `crates/example-commerce/src/lib.rs`
**Apply to:** `ids.rs`, `user.rs`, `product.rs`, `order.rs`

```rust
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProductError {
    #[error("product SKU cannot be empty")]
    EmptySku,
    #[error("product name cannot be empty")]
    EmptyName,
    #[error("product draft already exists")]
    AlreadyCreated,
}
```

### Routing and Expected Revision

**Source:** `crates/example-commerce/src/lib.rs`
**Apply to:** `user.rs`, `product.rs`, `order.rs`

```rust
fn stream_id(command: &Self::Command) -> StreamId {
    match command {
        ProductCommand::CreateProduct { stream_id, .. } => stream_id.clone(),
    }
}

fn partition_key(command: &Self::Command) -> PartitionKey {
    match command {
        ProductCommand::CreateProduct { stream_id, .. } => {
            PartitionKey::new(stream_id.as_str()).expect("stream id is a valid partition key")
        }
    }
}

fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
    ExpectedRevision::NoStream
}
```

Use `ExpectedRevision::NoStream` for first-create/register/place commands. Use `ExpectedRevision::Any` for follow-up lifecycle/inventory commands unless the command API is expanded to carry exact revisions.

### Metadata Helpers in Tests

**Source:** `crates/example-commerce/src/lib.rs`
**Apply to:** `tests.rs` and aggregate module tests

```rust
fn metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(1),
        correlation_id: Uuid::from_u128(2),
        causation_id: None,
        tenant_id: TenantId::new("tenant-a").expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}
```

### Dependency Boundary

**Source:** `crates/example-commerce/Cargo.toml` and `crates/example-commerce/tests/dependency_boundaries.rs`
**Apply to:** Entire `example-commerce` crate

```toml
[dependencies]
es-core = { path = "../es-core" }
es-kernel = { path = "../es-kernel" }
thiserror.workspace = true

[dev-dependencies]
proptest.workspace = true
time.workspace = true
uuid.workspace = true
```

Do not add Tokio, SQLx, adapter, broker, or disruptor dependencies to the domain fixture.

## No Analog Found

No files are fully without analog. Two planned files are adaptations rather than exact copies:

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/example-commerce/src/ids.rs` | model | transform | Exact commerce ID objects do not exist yet; copy the `es-core` opaque string newtype pattern. |
| `crates/example-commerce/src/tests.rs` | test | batch | Existing tests are embedded in `lib.rs`; split only if shared helpers would reduce duplication. |

## Metadata

**Analog search scope:** `crates/example-commerce`, `crates/es-kernel`, `crates/es-core`, with supplemental runtime aggregate examples in `crates/es-runtime`.
**Files scanned:** 4 primary Rust files in target scope; supplemental `rg` search covered all Rust files under `crates/`.
**Pattern extraction date:** 2026-04-17

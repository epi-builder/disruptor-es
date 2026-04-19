# Phase 10: Duplicate-Safe Process Manager Follow-Up Keys - Pattern Map

**Mapped:** 2026-04-20
**Files analyzed:** 3
**Analogs found:** 3 / 3

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/app/src/commerce_process_manager.rs` | process manager, test | event-driven, request-response | `crates/app/src/commerce_process_manager.rs` | exact |
| `crates/example-commerce/src/order.rs` | model | CRUD, event-driven | `crates/example-commerce/src/order.rs` | exact, read-only |
| `crates/example-commerce/src/product.rs` | model | CRUD, event-driven | `crates/example-commerce/src/product.rs` | exact, read-only |

## Pattern Assignments

### `crates/app/src/commerce_process_manager.rs` (process manager, event-driven/request-response)

**Analog:** `crates/app/src/commerce_process_manager.rs`

**Imports pattern** (lines 1-10):
```rust
use std::future::Future;
use std::pin::Pin;

use es_core::CommandMetadata;
use es_outbox::{
    OutboxError, OutboxResult, ProcessEvent, ProcessManager, ProcessManagerName, ProcessOutcome,
};
use es_runtime::{CommandEnvelope, CommandGateway};
use example_commerce::{Order, OrderCommand, OrderEvent, Product, ProductCommand};
use uuid::Uuid;
```

**Process-manager dispatch pattern** (lines 34-58):
```rust
impl ProcessManager for CommerceOrderProcessManager {
    fn name(&self) -> &ProcessManagerName {
        &self.name
    }

    fn handles(&self, event_type: &str, schema_version: i32) -> bool {
        event_type == "OrderPlaced" && schema_version == 1
    }

    fn process<'a>(
        &'a self,
        event: &'a ProcessEvent,
    ) -> Pin<Box<dyn Future<Output = OutboxResult<ProcessOutcome>> + Send + 'a>> {
        Box::pin(async move {
            if !self.handles(&event.event_type, event.schema_version) {
                return Ok(ProcessOutcome::Skipped {
                    global_position: event.global_position,
                });
            }

            let OrderEvent::OrderPlaced {
                order_id,
                user_id: _,
                lines,
            } = decode_order_placed(event)?
```

**Reserve follow-up pattern to modify** (lines 65-90):
```rust
let mut command_count = 0;
let mut inventory_reserved = true;
let mut reserved_lines = Vec::new();
for line in lines {
    let (reply, receiver) = tokio::sync::oneshot::channel();
    let product_id = line.product_id.clone();
    let quantity = line.quantity;
    let envelope = CommandEnvelope::<Product>::new(
        ProductCommand::ReserveInventory {
            product_id: product_id.clone(),
            quantity,
        },
        follow_up_metadata(event),
        format!(
            "pm:{}:{}:reserve:{}",
            self.name.as_str(),
            event.event_id,
            product_id.as_str()
        ),
        reply,
    )
    .map_err(command_submit_error)?;
    self.product_gateway
        .try_submit(envelope)
        .map_err(command_submit_error)?;
```

Planner guidance: keep this gateway/reply shape, but enumerate source lines and include the stable line ordinal in the reserve key. Store the ordinal with successful reservations:
```rust
for (line_index, line) in lines.into_iter().enumerate() {
    // key shape: pm:{manager}:{source_event_id}:reserve:{line_index}:{product_id}
}

reserved_lines.push((line_index, product_id, quantity));
```

**Release compensation pattern to modify** (lines 106-131):
```rust
if !inventory_reserved {
    for (product_id, quantity) in reserved_lines {
        let (reply, receiver) = tokio::sync::oneshot::channel();
        let envelope = CommandEnvelope::<Product>::new(
            ProductCommand::ReleaseInventory {
                product_id: product_id.clone(),
                quantity,
            },
            follow_up_metadata(event),
            format!(
                "pm:{}:{}:release:{}",
                self.name.as_str(),
                event.event_id,
                product_id.as_str()
            ),
            reply,
        )
        .map_err(command_submit_error)?;
        self.product_gateway
            .try_submit(envelope)
            .map_err(command_submit_error)?;
        command_count += 1;
        receiver
            .await
            .map_err(|_| OutboxError::CommandReplyDropped)?
            .map_err(command_submit_error)?;
    }
}
```

Planner guidance: keep release routed through `CommandGateway<Product>` and keep reply waiting inside `process`. Change only the reserved-line tuple/key shape so release uses the original line ordinal:
```rust
// key shape: pm:{manager}:{source_event_id}:release:{line_index}:{product_id}
for (line_index, product_id, quantity) in reserved_lines {
    // submit ProductCommand::ReleaseInventory through product_gateway
}
```

**Order outcome pattern, unchanged** (lines 135-176):
```rust
let (reply, receiver) = tokio::sync::oneshot::channel();
let (command, idempotency_key) = if inventory_reserved {
    (
        OrderCommand::ConfirmOrder {
            order_id: order_id.clone(),
        },
        format!(
            "pm:{}:{}:confirm:{}",
            self.name.as_str(),
            event.event_id,
            order_id.as_str()
        ),
    )
} else {
    (
        OrderCommand::RejectOrder {
            order_id: order_id.clone(),
            reason: "inventory reservation failed".to_owned(),
        },
        format!(
            "pm:{}:{}:reject:{}",
            self.name.as_str(),
            event.event_id,
            order_id.as_str()
        ),
    )
};
```

**Metadata and error pattern** (lines 186-206):
```rust
fn decode_order_placed(event: &ProcessEvent) -> OutboxResult<OrderEvent> {
    serde_json::from_value(event.payload.clone()).map_err(|_| OutboxError::PayloadDecode {
        event_type: event.event_type.clone(),
        schema_version: event.schema_version,
    })
}

fn follow_up_metadata(event: &ProcessEvent) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::now_v7(),
        correlation_id: event.correlation_id,
        causation_id: Some(event.event_id),
        tenant_id: event.tenant_id.clone(),
        requested_at: time::OffsetDateTime::now_utc(),
    }
}

fn command_submit_error(error: impl std::fmt::Display) -> OutboxError {
    OutboxError::CommandSubmit {
        message: error.to_string(),
    }
}
```

**Test helper pattern** (lines 248-316):
```rust
fn line(product: ProductId) -> OrderLine {
    OrderLine {
        product_id: product,
        sku: Sku::new("SKU-1").expect("sku"),
        quantity: Quantity::new(2).expect("quantity"),
        product_available: true,
    }
}

fn process_event(event: OrderEvent) -> ProcessEvent {
    ProcessEvent {
        global_position: 42,
        event_id: Uuid::from_u128(42),
        event_type: "OrderPlaced".to_owned(),
        schema_version: 1,
        payload: serde_json::to_value(event).expect("order event payload"),
        metadata: json!({ "source": "commerce-process-manager-test" }),
        tenant_id: tenant(),
        command_id: Uuid::from_u128(100),
        correlation_id: Uuid::from_u128(101),
        causation_id: Some(Uuid::from_u128(102)),
    }
}

async fn receive_product(
    product_rx: &mut mpsc::Receiver<RoutedCommand<Product>>,
) -> RoutedCommand<Product> {
    timeout(Duration::from_millis(100), product_rx.recv())
        .await
        .expect("product command received before timeout")
        .expect("product command")
}
```

**Existing deterministic-key test pattern** (lines 969-1033):
```rust
#[tokio::test]
async fn process_manager_uses_deterministic_idempotency_keys() -> OutboxResult<()> {
    let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
    let manager = CommerceOrderProcessManager::new(
        process_manager_name(),
        product_gateway,
        order_gateway,
    );
    let product = product_id("product-1");
    let event = process_event(OrderEvent::OrderPlaced {
        order_id: order_id(),
        user_id: UserId::new("user-1").expect("user id"),
        lines: vec![line(product.clone())],
    });

    let process_event = event.clone();
    let task = tokio::spawn(async move { manager.process(&process_event).await });

    let reserve = receive_product(&mut product_rx).await;
    assert_eq!(
        format!(
            "pm:{}:{}:reserve:{}",
            process_manager_name().as_str(),
            event.event_id,
            product.as_str()
        ),
        reserve.envelope.idempotency_key
    );
```

Planner guidance: update this assertion to the new reserve key format with `:reserve:0:{product}`. Add `duplicate_product_lines_emit_distinct_reserve_keys` beside this test, using the same gateway receiver style and two `line(same_product.clone())` entries.

**Existing compensation test pattern** (lines 847-923):
```rust
#[tokio::test]
async fn multi_line_reserve_failure_releases_prior_reservations_before_rejecting()
-> OutboxResult<()> {
    let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
    let manager = CommerceOrderProcessManager::new(
        process_manager_name(),
        product_gateway,
        order_gateway,
    );
    let first_product = product_id("product-1");
    let second_product = product_id("product-2");
    let event = process_event(OrderEvent::OrderPlaced {
        order_id: order_id(),
        user_id: UserId::new("user-1").expect("user id"),
        lines: vec![line(first_product.clone()), line(second_product.clone())],
    });
```

```rust
let release = receive_product(&mut product_rx).await;
assert_eq!(
    ProductCommand::ReleaseInventory {
        product_id: first_product.clone(),
        quantity: Quantity::new(2).expect("quantity")
    },
    release.envelope.command
);
assert_eq!(
    format!(
        "pm:{}:{}:release:{}",
        process_manager_name().as_str(),
        event.event_id,
        first_product.as_str()
    ),
    release.envelope.idempotency_key
);
```

Planner guidance: add `duplicate_product_line_failure_releases_distinct_prior_lines` by adapting this test to duplicate same-product lines where the first duplicate succeeds and a later reserve fails. Assert release key includes the original successful line index.

**Replay-aware store pattern to extend if needed** (lines 476-606):
```rust
struct ReplayAwareStoreInner {
    global_position: i64,
    event_id: Uuid,
    rehydration_events: Vec<StoredEvent>,
    append_requests: Mutex<Vec<AppendRequest>>,
    replay_record: Mutex<Option<CommandReplayRecord>>,
    lookup_count: Mutex<usize>,
}
```

```rust
fn append(&self, request: AppendRequest) -> StoreResult<AppendOutcome> {
    let committed = es_store_postgres::CommittedAppend {
        stream_id: request.stream_id.clone(),
        first_revision: StreamRevision::new(1),
        last_revision: StreamRevision::new(1),
        global_positions: vec![self.global_position],
        event_ids: vec![self.event_id],
    };
    if let Some(reply) = request.command_reply_payload.clone() {
        *self.replay_record.lock().expect("replay record") = Some(CommandReplayRecord {
            append: committed.clone(),
            reply,
        });
    }
    self.append_requests
        .lock()
        .expect("append requests")
        .push(request);
    Ok(AppendOutcome::Committed(committed))
}

fn lookup_command_replay(&self) -> StoreResult<Option<CommandReplayRecord>> {
    *self.lookup_count.lock().expect("lookup count") += 1;
    Ok(self.replay_record.lock().expect("replay record").clone())
}
```

Planner guidance: for duplicate-line replay coverage, change this store to return replay records by requested idempotency key rather than a single last record. Keep the `RuntimeEventStore` delegate shape from lines 624-630 and 649-655, but pass/use `_idempotency_key`.

**Replay-through-real-engine test pattern** (lines 1037-1119):
```rust
#[tokio::test]
async fn process_manager_replayed_followups_return_original_outcomes() -> OutboxResult<()> {
    let process_manager_name =
        ProcessManagerName::new("commerce-order-pm").expect("process manager name");
    let product = product_id("product-1");
    let source_order_event = OrderEvent::OrderPlaced {
        order_id: order_id(),
        user_id: UserId::new("user-1").expect("user id"),
        lines: vec![line(product.clone())],
    };
    let event = process_event(source_order_event.clone());
    let product_store = ReplayAwareProductStore::new(product.clone());
    let order_store = ReplayAwareOrderStore::new(&source_order_event);
```

```rust
let second_event = event.clone();
let second_manager = manager.clone();
let second_task = tokio::spawn(async move { second_manager.process(&second_event).await });
assert!(product_engine.process_one().await.expect("second reserve"));
assert!(order_engine.process_one().await.expect("second confirm"));
assert_eq!(
    ProcessOutcome::CommandsSubmitted {
        global_position: event.global_position,
        command_count: 2
    },
    second_task.await.expect("second process")?
);

assert_eq!(1, product_store.append_count());
assert_eq!(1, order_store.append_count());
assert_eq!(vec![expected_reserve_key], product_store.idempotency_keys());
assert_eq!(vec![expected_confirm_key], order_store.idempotency_keys());
assert_eq!(product_store.replay_global_positions(), vec![20]);
assert_eq!(order_store.replay_global_positions(), vec![21]);
```

Planner guidance: extend this test or add companion coverage for duplicate product lines. Expected product append count should equal distinct line-aware reserve keys after first process and remain unchanged after retry.

---

### `crates/example-commerce/src/order.rs` (model, CRUD/event-driven)

**Analog:** `crates/example-commerce/src/order.rs`

**Imports pattern** (lines 1-4):
```rust
use crate::{OrderId, ProductId, Quantity, Sku, UserId};
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};
use serde::{Deserialize, Serialize};
```

**Order-line identity source** (lines 26-37):
```rust
/// Product line captured by an order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OrderLine {
    /// Product identity referenced by the order.
    pub product_id: ProductId,
    /// SKU referenced by the order.
    pub sku: Sku,
    /// Quantity requested for the line.
    pub quantity: Quantity,
    /// Whether the product is available when the order command is decided.
    pub product_available: bool,
}
```

**Event payload shape** (lines 69-81):
```rust
/// Events emitted by the order aggregate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum OrderEvent {
    /// Order was placed.
    OrderPlaced { order_id: OrderId, user_id: UserId, lines: Vec<OrderLine> },
    /// Order was confirmed.
    OrderConfirmed { order_id: OrderId },
    /// Order was rejected.
    OrderRejected { order_id: OrderId, reason: String },
    /// Order was cancelled.
    OrderCancelled { order_id: OrderId },
}
```

**Validation/core decision pattern** (lines 168-196):
```rust
OrderCommand::PlaceOrder {
    order_id,
    user_id,
    user_active,
    lines,
} => {
    if state.status != OrderStatus::Draft {
        return Err(OrderError::AlreadyPlaced);
    }
    if lines.is_empty() {
        return Err(OrderError::EmptyOrder);
    }
    if !user_active {
        return Err(OrderError::InactiveUser { user_id });
    }
    if let Some(line) = lines.iter().find(|line| !line.product_available) {
        return Err(OrderError::UnavailableProduct {
            product_id: line.product_id.clone(),
        });
    }

    Ok(Decision::new(
        vec![OrderEvent::OrderPlaced {
            order_id: order_id.clone(),
            user_id,
            lines,
        }],
        OrderReply::Placed { order_id },
    ))
}
```

Planner guidance: do not modify order schema for this phase. The ordered `Vec<OrderLine>` in `OrderPlaced` is the stable source for `enumerate()` ordinals.

---

### `crates/example-commerce/src/product.rs` (model, CRUD/event-driven)

**Analog:** `crates/example-commerce/src/product.rs`

**Imports pattern** (lines 1-4):
```rust
use crate::{ProductId, Quantity, Sku};
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};
use serde::{Deserialize, Serialize};
```

**Reserve/release command shape** (lines 25-59):
```rust
pub enum ProductCommand {
    /// Moves available inventory into reserved inventory.
    ReserveInventory {
        /// Product identity.
        product_id: ProductId,
        /// Quantity to reserve.
        quantity: Quantity,
    },
    /// Releases reserved inventory back to available inventory.
    ReleaseInventory {
        /// Product identity.
        product_id: ProductId,
        /// Quantity to release.
        quantity: Quantity,
    },
}
```

**Reserve decision semantics** (lines 269-296):
```rust
ProductCommand::ReserveInventory {
    product_id,
    quantity,
} => {
    ensure_created(state)?;
    let requested = quantity.value();
    let requested_i32 = quantity_to_i32(quantity);
    if requested_i32 > state.available_quantity {
        return Err(ProductError::InsufficientInventory {
            available: state.available_quantity,
            requested,
        });
    }
    state.reserved_quantity.checked_add(requested_i32).ok_or(
        ProductError::InventoryWouldOverflow {
            available: state.available_quantity,
            reserved: state.reserved_quantity,
            requested,
        },
    )?;

    Ok(Decision::new(
        vec![ProductEvent::InventoryReserved {
            product_id: product_id.clone(),
            quantity,
        }],
        ProductReply::InventoryReserved { product_id },
    ))
}
```

**Release decision semantics** (lines 298-325):
```rust
ProductCommand::ReleaseInventory {
    product_id,
    quantity,
} => {
    ensure_created(state)?;
    let requested = quantity.value();
    let requested_i32 = quantity_to_i32(quantity);
    if requested_i32 > state.reserved_quantity {
        return Err(ProductError::InsufficientReservedInventory {
            reserved: state.reserved_quantity,
            requested,
        });
    }
    state.available_quantity.checked_add(requested_i32).ok_or(
        ProductError::InventoryWouldOverflow {
            available: state.available_quantity,
            reserved: state.reserved_quantity,
            requested,
        },
    )?;

    Ok(Decision::new(
        vec![ProductEvent::InventoryReleased {
            product_id: product_id.clone(),
            quantity,
        }],
        ProductReply::InventoryReleased { product_id },
    ))
}
```

Planner guidance: do not change product command/event semantics. Phase 10 preserves one command per order line and only changes process-manager key identity.

## Shared Patterns

### Runtime/Store Duplicate Replay

**Source:** `crates/es-runtime/src/shard.rs`
**Apply to:** process-manager replay tests and any replay-aware store extension

**Runtime lookup before decide** (lines 170-208):
```rust
let dedupe_key = DedupeKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    idempotency_key: envelope.idempotency_key.clone(),
};

if let Some(record) = self.dedupe.get(&dedupe_key) {
    let outcome = replay_command_outcome::<A, C>(codec, &record.replay);
    let _ = envelope.reply.send(outcome);
    return Ok(true);
}

match store
    .lookup_command_replay(&envelope.metadata.tenant_id, &envelope.idempotency_key)
    .await
{
    Ok(Some(replay)) => {
        let outcome = replay_command_outcome::<A, C>(codec, &replay);
        if outcome.is_ok() {
            self.dedupe
                .record(dedupe_key.clone(), DedupeRecord { replay });
        }
        let _ = envelope.reply.send(outcome);
        return Ok(true);
    }
    Ok(None) => {}
    Err(error) => {
        let _ = envelope
            .reply
            .send(Err(RuntimeError::from_store_error(error)));
        return Ok(true);
    }
}
```

**Append stores command reply for replay** (lines 294-328):
```rust
let append_request = match es_store_postgres::AppendRequest::new(
    envelope.stream_id.clone(),
    envelope.expected_revision,
    envelope.metadata.clone(),
    envelope.idempotency_key.clone(),
    new_events,
) {
    Ok(request) => request.with_command_reply_payload(command_reply_payload.clone()),
    Err(error) => {
        let _ = envelope
            .reply
            .send(Err(RuntimeError::from_store_error(error)));
        return Ok(true);
    }
};

match store.append(append_request).await {
    Ok(es_store_postgres::AppendOutcome::Committed(committed)) => {
        self.dedupe.record(
            dedupe_key,
            DedupeRecord {
                replay: es_store_postgres::CommandReplayRecord {
                    append: committed.clone(),
                    reply: command_reply_payload.clone(),
                },
            },
        );
```

**Replay response decode** (lines 406-416):
```rust
fn replay_command_outcome<A, C>(
    codec: &C,
    replay: &es_store_postgres::CommandReplayRecord,
) -> RuntimeResult<CommandOutcome<A::Reply>>
where
    A: Aggregate,
    C: RuntimeEventCodec<A>,
{
    let reply = codec.decode_reply(&replay.reply)?;
    Ok(CommandOutcome::new(reply, replay.append.clone()))
}
```

### PostgreSQL Dedupe Shape

**Source:** `crates/es-store-postgres/src/sql.rs`
**Apply to:** no schema changes; app keys are opaque text scoped by tenant

**Insert by tenant and idempotency key** (lines 354-388):
```rust
INSERT INTO command_dedup (
    tenant_id,
    idempotency_key,
    stream_id,
    first_revision,
    last_revision,
    first_global_position,
    last_global_position,
    event_ids,
    response_payload
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
RETURNING 1::bigint
```

**Lookup by tenant and idempotency key** (lines 410-432):
```rust
pub(crate) async fn lookup_command_replay(
    pool: &PgPool,
    tenant_id: &TenantId,
    idempotency_key: &str,
) -> StoreResult<Option<CommandReplayRecord>> {
    let response_payload = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT response_payload
        FROM command_dedup
        WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?;
```

### Runtime Test Store Sequence Pattern

**Source:** `crates/es-runtime/tests/runtime_flow.rs`
**Apply to:** optional inspiration if `ReplayAwareProductStore` needs multi-result replay behavior

**Replay sequence setup** (lines 278-284):
```rust
fn set_command_replay(&self, replay: CommandReplayRecord) {
    self.set_command_replay_sequence(vec![Some(replay)]);
}

fn set_command_replay_sequence(&self, replay: Vec<Option<CommandReplayRecord>>) {
    *self.inner.command_replay.lock().expect("command replay") = replay.into();
}
```

**Duplicate append test shape** (lines 926-945):
```rust
#[tokio::test]
async fn duplicate_append_returns_successful_command_outcome() {
    let store = FakeStore::duplicate();
    store.set_command_replay_sequence(vec![None, Some(command_replay_record(1, 3))]);
    let codec = CounterCodec::default();
    let mut state = ShardState::<CounterAggregate>::new(ShardId::new(0));
    let (envelope, receiver) = envelope(3);
    record_handoff(&mut state, envelope);

    assert!(
        state
            .process_next_handoff(&store, &codec)
            .await
            .expect("processed")
    );

    let outcome = receiver.await.expect("reply").expect("success");
    assert_eq!(3, outcome.reply);
    assert_eq!(vec![1], outcome.append.global_positions);
```

## No Analog Found

None. All expected files have local exact or read-only analogs.

## Implementation Boundaries

- Do not add new dependencies.
- Do not change `OrderLine`, `OrderEvent`, `ProductCommand`, or `ProductEvent` schemas for this phase.
- Do not change runtime/store dedupe semantics. Runtime/store already replay by `(tenant_id, idempotency_key)`.
- Keep all process-manager follow-up commands routed through `CommandGateway`.
- Keep reply waits inside `CommerceOrderProcessManager::process` before returning `ProcessOutcome::CommandsSubmitted`.
- Use zero-based ordinals from `enumerate()` for internal line-aware keys.

## Metadata

**Analog search scope:** `crates/app`, `crates/example-commerce`, `crates/es-runtime`, `crates/es-store-postgres`
**Files scanned:** 49 Rust/Cargo/migration files from the relevant crates
**Pattern extraction date:** 2026-04-20

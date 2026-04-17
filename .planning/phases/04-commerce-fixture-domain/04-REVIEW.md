---
phase: 04-commerce-fixture-domain
reviewed: 2026-04-17T08:29:24Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - crates/example-commerce/src/ids.rs
  - crates/example-commerce/src/lib.rs
  - crates/example-commerce/src/order.rs
  - crates/example-commerce/src/product.rs
  - crates/example-commerce/src/tests.rs
  - crates/example-commerce/src/user.rs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-04-17T08:29:24Z
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Reviewed the commerce fixture domain aggregates, value objects, exports, and property tests. The phase mostly matches the event-sourced domain constraints: errors are typed, decisions are deterministic over command plus aggregate state, mutations are isolated to `apply`, there are no runtime/storage/adapter dependencies, and no shared mutable global state was found.

The main correctness gap is that existing aggregate state only proves that an aggregate exists or has a lifecycle status; subsequent commands do not verify that the command ID matches the ID already held in that state. If callers accidentally pass a state from one stream into a command for another stream, `decide` emits an event for the wrong ID, and `apply` then overwrites the state's identity. This violates aggregate-local identity invariants and can make replayed state inconsistent with the stream being processed.

## Warnings

### WR-01: User lifecycle commands do not verify aggregate identity

**File:** `crates/example-commerce/src/user.rs:157`
**Issue:** `ActivateUser` and `DeactivateUser` branch only on `state.status`. Once a user is registered, a lifecycle command for a different `UserId` is accepted and emits an event carrying that different ID. Replaying that event overwrites `state.user_id`, so a stream/state mix-up can silently mutate one aggregate into another.
**Fix:** Add a typed identity mismatch error and check the command ID against `state.user_id` before accepting registered-user lifecycle transitions.

```rust
#[error("command user {command_user_id:?} does not match state user {state_user_id:?}")]
UserIdMismatch {
    state_user_id: UserId,
    command_user_id: UserId,
}

fn ensure_user_id(state: &UserState, command_user_id: &UserId) -> Result<(), UserError> {
    match &state.user_id {
        Some(state_user_id) if state_user_id == command_user_id => Ok(()),
        Some(state_user_id) => Err(UserError::UserIdMismatch {
            state_user_id: state_user_id.clone(),
            command_user_id: command_user_id.clone(),
        }),
        None => Err(UserError::NotRegistered),
    }
}
```

Then call `ensure_user_id(state, &user_id)?` before emitting `UserActivated` or `UserDeactivated`, while preserving the existing status-specific errors after the identity check.

### WR-02: Product inventory commands do not verify aggregate identity

**File:** `crates/example-commerce/src/product.rs:232`
**Issue:** `AdjustInventory`, `ReserveInventory`, and `ReleaseInventory` only call `ensure_created(state)`, which checks that some product exists. A command for a different `ProductId` is then accepted and the emitted event carries the command ID, while `apply` updates inventory on the existing state without preserving any proof that the event belongs to the same product. This can corrupt aggregate-local state when an incorrect state/command pair is supplied.
**Fix:** Replace `ensure_created(state)?` in inventory command branches with an identity-aware helper.

```rust
#[error("command product {command_product_id:?} does not match state product {state_product_id:?}")]
ProductIdMismatch {
    state_product_id: ProductId,
    command_product_id: ProductId,
}

fn ensure_product_id(state: &ProductState, command_product_id: &ProductId) -> Result<(), ProductError> {
    match &state.product_id {
        Some(state_product_id) if state_product_id == command_product_id => Ok(()),
        Some(state_product_id) => Err(ProductError::ProductIdMismatch {
            state_product_id: state_product_id.clone(),
            command_product_id: command_product_id.clone(),
        }),
        None => Err(ProductError::NotCreated),
    }
}
```

Call `ensure_product_id(state, &product_id)?` before calculating inventory deltas, and add tests that a created product rejects inventory commands for a different product ID.

### WR-03: Order lifecycle commands do not verify aggregate identity

**File:** `crates/example-commerce/src/order.rs:197`
**Issue:** `ConfirmOrder`, `RejectOrder`, and `CancelOrder` only call `ensure_placed(state)`. A command for another `OrderId` can therefore produce a terminal event for that other ID, and `apply` overwrites `state.order_id` on lines 245-256. This breaks the invariant that all events applied to an order state belong to the same order stream.
**Fix:** Add an `OrderIdMismatch` error and validate command ID against `state.order_id` before terminal transitions.

```rust
#[error("command order {command_order_id:?} does not match state order {state_order_id:?}")]
OrderIdMismatch {
    state_order_id: OrderId,
    command_order_id: OrderId,
}

fn ensure_order_id(state: &OrderState, command_order_id: &OrderId) -> Result<(), OrderError> {
    match &state.order_id {
        Some(state_order_id) if state_order_id == command_order_id => Ok(()),
        Some(state_order_id) => Err(OrderError::OrderIdMismatch {
            state_order_id: state_order_id.clone(),
            command_order_id: command_order_id.clone(),
        }),
        None => Err(OrderError::NotPlaced),
    }
}
```

Call `ensure_order_id(state, &order_id)?` before `ensure_placed(state)?` or fold identity and lifecycle checks into a single helper. Add a regression test that a placed order rejects confirm/reject/cancel commands for a different order ID.

---

_Reviewed: 2026-04-17T08:29:24Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

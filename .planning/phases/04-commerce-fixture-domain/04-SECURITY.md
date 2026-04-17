---
phase: 04-commerce-fixture-domain
secured: 2026-04-18
asvs_level: 1
block_on: open_threats
threats_total: 18
threats_closed: 18
threats_open: 0
status: secured
---

# Phase 04 Security Verification

Phase 04 threat mitigations were verified against the declared register in the Phase 04 plans and the supplied implementation files. Implementation files were inspected read-only; only this report was written.

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-04-01 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/ids.rs:18`, `:39`, `:60`, and `:81` construct string IDs through `string_value`; `:102-105` rejects zero quantity. |
| T-04-02 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/ids.rs:96-111` exposes `Quantity(u32)`, `Quantity::new(u32)`, and `value()`; no signed public quantity constructor exists. |
| T-04-03 | Information Disclosure | accept | CLOSED | Accepted risk documented below. `crates/example-commerce/src/lib.rs:3-18` is a module facade only; shared mutable business-state scans returned no matches. |
| T-04-04 | Elevation of Privilege | mitigate | CLOSED | `crates/example-commerce/Cargo.toml:8-16` limits dependencies to `es-core`, `es-kernel`, `thiserror`, and test-only `proptest`/`time`/`uuid`; `cargo tree -p example-commerce --prefix none` contained no forbidden runtime/storage/adapter/broker/disruptor package names. |
| T-04-05 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/user.rs:138-143` rejects empty email and display name with `UserError::EmptyEmail` and `UserError::EmptyDisplayName`. |
| T-04-06 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/user.rs:144-145`, `:157-175` reject duplicate registration, activation before registration, duplicate activation, and duplicate deactivation. |
| T-04-07 | Repudiation | mitigate | CLOSED | `crates/example-commerce/src/user.rs:148-174` emits explicit user lifecycle events; `:260-332` replays registration, activation, and deactivation through `es_kernel::replay::<User>`. |
| T-04-08 | Information Disclosure | accept | CLOSED | Accepted risk documented below. User state is aggregate-local in `crates/example-commerce/src/user.rs:22-31`; shared mutable state scans returned no matches. |
| T-04-09 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/product.rs:232-245` checks adjusted inventory before emitting `InventoryAdjusted`; `:519-533` tests the negative adjustment rejection. |
| T-04-10 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/product.rs:255-266` rejects reservations above available inventory with `InsufficientInventory`; `:487-501` tests the path. |
| T-04-11 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/product.rs:276-287` rejects releases above reserved inventory with `InsufficientReservedInventory`; `:503-517` tests the path. |
| T-04-12 | Denial of Service | mitigate | CLOSED | Scan for `saturating_sub|saturating_add` returned no matches; `crates/example-commerce/src/product.rs:536-575` property-tests nonnegative available/reserved quantities after accepted events. |
| T-04-13 | Information Disclosure | accept | CLOSED | Accepted risk documented below. Product state is per aggregate instance in `crates/example-commerce/src/product.rs:10-28`; static/global inventory map scans returned no matches. |
| T-04-14 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/order.rs:173-186` rejects duplicate/non-draft placement, empty orders, inactive users, and unavailable products before `OrderPlaced`; `:371-422` tests these paths. |
| T-04-15 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/order.rs:173-174` returns `AlreadyPlaced` when placing from non-draft state; `:414-422` tests duplicate placement. |
| T-04-16 | Tampering | mitigate | CLOSED | `crates/example-commerce/src/order.rs:273-280` returns `AlreadyTerminal` after confirmed/rejected/cancelled states. |
| T-04-17 | Information Disclosure | mitigate | CLOSED | `rg "UserState|ProductState" crates/example-commerce/src/order.rs` returned no matches; `OrderState` stores IDs and order lines only in `crates/example-commerce/src/order.rs:35-47`. |
| T-04-18 | Elevation of Privilege | mitigate | CLOSED | `cargo test --workspace` passed; `crates/example-commerce/tests/dependency_boundaries.rs:6-15` defines forbidden dependency names for boundary tests, and `crates/example-commerce/Cargo.toml:8-16` plus `cargo tree -p example-commerce --prefix none` confirm no runtime/storage/adapter/broker/disruptor dependency in `example-commerce`. |

## Accepted Risks Log

| Threat ID | Accepted Risk | Rationale | Compensating Boundary |
|-----------|---------------|-----------|-----------------------|
| T-04-03 | The Phase 04 module facade does not implement tenant isolation. | Phase 04 has no tenant datastore, query API, adapter, or externally visible data access surface. | Tenant isolation remains a runtime/store metadata boundary for later phases; this facade avoids global mutable business state. |
| T-04-08 | User aggregate tenant isolation is not enforced inside the aggregate. | User commands receive deterministic domain fields only; the aggregate has no shared storage or static state. | Tenant routing and storage scoping remain outside the domain aggregate at runtime/store boundaries. |
| T-04-13 | Product inventory tenant isolation is not enforced inside the aggregate. | Product inventory state is held in per-aggregate `ProductState`, not a global/static inventory map. | Tenant routing and durable store scoping remain outside the domain aggregate at runtime/store boundaries. |

## Threat Flags

No `## Threat Flags` sections were present in `04-01-SUMMARY.md`, `04-02-SUMMARY.md`, `04-03-SUMMARY.md`, or `04-04-SUMMARY.md`; no unregistered flags were logged.

## Verification Commands

| Command | Result |
|---------|--------|
| `cargo test -p example-commerce` | Passed: 17 unit/property tests, 3 integration tests, 0 failures. |
| `cargo test --workspace` | Passed. One existing missing-doc warning appeared in `crates/es-runtime/tests/shard_disruptor.rs`. |
| `cargo tree -p example-commerce --prefix none` | Passed forbidden dependency review: no `tokio`, `sqlx`, `axum`, `tonic`, `async-nats`, `rdkafka`, `postgres`, or `disruptor` package names. |
| `rg "saturating_sub|saturating_add|Arc<Mutex|static mut|lazy_static" crates/example-commerce/src/user.rs crates/example-commerce/src/product.rs crates/example-commerce/src/order.rs crates/example-commerce/src/tests.rs` | No matches. |
| `rg "UserState|ProductState" crates/example-commerce/src/order.rs` | No matches. |

## Result

All registered Phase 04 threats are closed. There are no open threats and no unregistered threat flags from executor summaries.

---
phase: 05-cqrs-projection-and-query-catch-up
verified: 2026-04-18T01:13:39Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 5: CQRS Projection and Query Catch-Up Verification Report

**Phase Goal:** Committed events drive eventually consistent read models through checkpointed projectors that can restart, rebuild, catch up, and optionally satisfy read-your-own-write queries.
**Verified:** 2026-04-18T01:13:39Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Projectors apply committed events to read models and persist projector offsets in the same transaction. | VERIFIED | `PostgresProjectionStore::catch_up` reads committed events via `PostgresEventStore::read_global`, opens one SQLx transaction, applies read-model updates, upserts `projector_offsets`, and commits only after both succeed. On apply/upsert error it explicitly rolls back before returning the original projection error. |
| 2 | Developer can query order summary and product inventory read models derived only from committed events. | VERIFIED | Migration defines `order_summary_read_models` and `product_inventory_read_models`; `projection.rs` decodes `OrderEvent`/`ProductEvent` from stored event payloads with `serde_json::from_value` and query methods return public DTOs. Integration coverage verifies derived order status/totals and product available/reserved quantities. |
| 3 | After restart, a projector resumes from its saved global-position checkpoint and catches up without duplicating read-model effects. | VERIFIED | `catch_up` loads `projector_offset`, calls `read_global` after the saved offset, and read-model update guards use `last_applied_global_position` to prevent duplicate effects. `projections_resume_without_duplicate_effects` constructs a new store over the same pool and verifies unchanged rows plus saved offset. |
| 4 | Query callers can request a minimum global position to support read-your-own-write behavior without making projection completion part of command success. | VERIFIED | `MinimumGlobalPosition`, `WaitPolicy`, `FreshnessCheck`, and `wait_for_minimum_position` are query-side contracts; `order_summary` and `product_inventory` optionally call the bounded wait helper and return `ProjectionError::ProjectionLag` on timeout. `PHASE_BOUNDARY` states projection catch-up must not gate command success. |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/es-projection/src/error.rs` | `ProjectionError` and `ProjectionResult` including lag/payload/store errors | VERIFIED | Exists, substantive, exported from `lib.rs`; contains `InvalidProjectorName`, `InvalidGlobalPosition`, `InvalidBatchLimit`, `ProjectionLag`, `PayloadDecode`, and `Store`. |
| `crates/es-projection/src/checkpoint.rs` | Validated projector names, offsets, minimum positions, and batch limits | VERIFIED | Constructors reject empty names, negative positions, and invalid limits; `ProjectorOffset` carries `TenantId`. |
| `crates/es-projection/src/projector.rs` | Storage-neutral `ProjectionEvent`, `CatchUpOutcome`, and projector trait | VERIFIED | No PostgreSQL dependency; DTO carries durable `global_position`, payload, metadata, and tenant. |
| `crates/es-projection/src/query.rs` | Bounded wait policy and minimum-position wait helper | VERIFIED | Uses `tokio::time::Instant` deadline and `sleep`; returns `ProjectionLag` rather than looping indefinitely. |
| `crates/example-commerce/src/ids.rs` | Serde-compatible commerce IDs and quantity | VERIFIED | String IDs derive serde; `Quantity` serializes and validates deserialization through constructor bounds. |
| `crates/example-commerce/src/order.rs` | Serde-compatible order projection payload events | VERIFIED | `OrderEvent`, `OrderLine`, and `OrderStatus` derive serde; payload round-trip tests cover placed and rejected events. |
| `crates/example-commerce/src/product.rs` | Serde-compatible product projection payload events | VERIFIED | `ProductEvent` derives serde; payload round-trip tests cover created and inventory-reserved events. |
| `crates/es-store-postgres/migrations/20260418000000_projection_read_models.sql` | Tenant-scoped projector offsets and read-model tables | VERIFIED | Defines `projector_offsets`, `order_summary_read_models`, and `product_inventory_read_models` with tenant-scoped primary keys and position checks. |
| `crates/es-store-postgres/src/projection.rs` | PostgreSQL projection store, catch-up, DTOs, and query methods | VERIFIED | Implements `PostgresProjectionStore`, `catch_up`, `order_summary`, `product_inventory`, event decoding, SQLx bound queries, and explicit transaction rollback on projection errors. |
| `crates/es-store-postgres/tests/projections.rs` | PostgreSQL integration coverage for PROJ-01 through PROJ-04 | VERIFIED | Contains tests for atomic offset/read-model commits, derived commerce views, restart/idempotence, bounded minimum-position queries, tenant isolation, malformed payload rollback, and stale offset monotonicity. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/es-projection/src/query.rs` | `crates/es-projection/src/checkpoint.rs` | `MinimumGlobalPosition` freshness comparison | VERIFIED | `FreshnessCheck::compare` accepts `MinimumGlobalPosition`; `wait_for_minimum_position` uses the typed required position. |
| `crates/es-projection/src/projector.rs` | `crates/es-projection/src/checkpoint.rs` | `ProjectorName` and `ProjectionEvent` contracts | VERIFIED | `Projector::name` returns `ProjectorName`; projection event remains storage neutral. |
| `crates/example-commerce/src/order.rs` | `crates/es-store-postgres/src/projection.rs` | `serde_json::from_value::<OrderEvent>` | VERIFIED | Handled order event types are decoded from stored payloads and mapped to order summary rows. |
| `crates/example-commerce/src/product.rs` | `crates/es-store-postgres/src/projection.rs` | `serde_json::from_value::<ProductEvent>` | VERIFIED | Handled product event types are decoded from stored payloads and mapped to product inventory rows. |
| `crates/es-store-postgres/src/projection.rs` | `crates/es-store-postgres/src/event_store.rs` | `PostgresEventStore::read_global(tenant_id, offset, limit)` | VERIFIED | `catch_up` reads committed global events after the saved checkpoint before applying projections. |
| `crates/es-store-postgres/src/projection.rs` | `projector_offsets` | Same SQLx transaction as read-model writes | VERIFIED | Offset upsert is executed in the same transaction block as `apply_projection_event`; commit follows both. |
| `crates/es-store-postgres/src/projection.rs` | `order_summary_read_models` | Order event payload decoding and upsert/update | VERIFIED | `OrderPlaced` upserts summary; lifecycle events update status with position guards. |
| `crates/es-store-postgres/src/projection.rs` | `product_inventory_read_models` | Product event payload decoding and upsert/update | VERIFIED | Product creation and inventory events update denormalized inventory with position guards. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `PostgresProjectionStore::catch_up` | `stored_events` / `events` | `PostgresEventStore::read_global(tenant_id, current_offset, limit.value())` | Yes - reads durable committed `events` rows through the event store global reader. | FLOWING |
| `order_summary_read_models` | `OrderSummaryReadModel` fields | Decoded `OrderEvent` payloads from `ProjectionEvent.payload` | Yes - `OrderPlaced`, `OrderConfirmed`, `OrderRejected`, and `OrderCancelled` drive row values. | FLOWING |
| `product_inventory_read_models` | `ProductInventoryReadModel` fields | Decoded `ProductEvent` payloads from `ProjectionEvent.payload` | Yes - product creation and inventory events drive SKU/name/available/reserved quantities. | FLOWING |
| Query freshness waits | `last_applied_global_position` | Row-specific SQL reads in `order_summary_position` / `product_inventory_position` | Yes - reads persisted read-model freshness and returns typed lag when below requested minimum. | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Projection contracts validate minimum-position behavior | `cargo test -p es-projection minimum_position -- --nocapture` | 8 tests passed. | PASS |
| Commerce payload DTOs round-trip through JSON | `cargo test -p example-commerce projection_payload -- --nocapture` | 4 tests passed. | PASS |
| PostgreSQL projection integration behavior | Not rerun during verification to avoid starting Testcontainers as a spot-check; final execution context reports `cargo test --workspace` passed after fixes. Static verification found all integration tests present. | Reported passed by final phase context; tests are container-backed and covered in review/fix artifacts. | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PROJ-01 | 05-01, 05-03 | Projector runtime applies committed events to read models and updates projector offsets in the same transaction. | SATISFIED | `catch_up` reads global committed events, applies read-model writes and offset upsert inside one transaction, and tests `projections_offset_commits_with_read_models`. |
| PROJ-02 | 05-02, 05-03 | Example read models expose order summary and product inventory views derived from events. | SATISFIED | Public DTO/query methods exist; `projections_build_commerce_read_models` verifies order summary and product inventory values derived from serialized commerce events. Note: `.planning/REQUIREMENTS.md` still marks PROJ-02 Pending, which is metadata drift rather than an implementation gap. |
| PROJ-03 | 05-01, 05-03 | Projection runtime can catch up from a saved global-position checkpoint after restart. | SATISFIED | `projector_offset` is tenant/projector scoped; restarted store resumes after saved offset; idempotence test verifies no duplicate read-model effects. |
| PROJ-04 | 05-01, 05-03 | Query path can optionally wait for a minimum global position to support read-your-own-write behavior. | SATISFIED | `MinimumGlobalPosition`, `WaitPolicy`, and query methods implement bounded waits; unit and integration tests verify fresh and lagging paths. |

No orphaned Phase 5 requirements were found. The plan frontmatter accounts for all Phase 5 requirement IDs listed in `.planning/REQUIREMENTS.md`.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `.planning/REQUIREMENTS.md` | PROJ-02 status | Requirement metadata still says Pending even though implementation evidence satisfies it. | Info | Does not block the phase goal; update requirement status during orchestration/state reconciliation. |
| `.planning/ROADMAP.md` / roadmap data | 05-03 plan checkbox | Roadmap data still reports 05-03 unchecked while summaries and code exist. | Info | Does not block goal achievement; likely orchestration metadata pending after verification. |

### Human Verification Required

None. This phase is backend/storage/query behavior with automated and static verification coverage; no visual or manual UX behavior is required.

### Gaps Summary

No goal-blocking gaps found. The phase delivers storage-neutral projection contracts, serde-backed commerce event payloads, tenant-scoped PostgreSQL projection schema, atomic catch-up, restart-safe checkpoints, derived read-model queries, and bounded read-your-own-write waits.

---

_Verified: 2026-04-18T01:13:39Z_
_Verifier: Claude (gsd-verifier)_

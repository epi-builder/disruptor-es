---
phase: 05-cqrs-projection-and-query-catch-up
fixed_at: 2026-04-18T00:54:30Z
review_path: .planning/phases/05-cqrs-projection-and-query-catch-up/05-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 05: Code Review Fix Report

**Fixed at:** 2026-04-18T00:54:30Z, extended after re-review
**Source review:** `.planning/phases/05-cqrs-projection-and-query-catch-up/05-REVIEW.md`
**Iteration:** 1

**Summary:**
- Initial findings in scope: 2
- Findings in scope after first re-review: 1
- Findings in scope after second re-review: 1
- Fixed: 4
- Skipped: 0

## Fixed Issues

### WR-01: concurrent catch-up can move projector offset backward

**Files modified:** `crates/es-store-postgres/src/projection.rs`, `crates/es-store-postgres/tests/projections.rs`
**Commit:** f1a2c56
**Applied fix:** Changed the projector offset conflict update to keep the greater of the existing and incoming global positions, and added a regression test proving a stale lower offset cannot move an existing higher offset backward.

### WR-02: Quantity accepts values that overflow product inventory state

**Files modified:** `crates/example-commerce/src/ids.rs`, `crates/example-commerce/src/product.rs`
**Commit:** 8b2d858
**Applied fix:** Bounded `Quantity` to the signed inventory storage range through construction and deserialization, updated the error documentation/message, added the requested boundary test, and replaced unchecked inventory casts with an explicit invariant conversion.

### WR-03: Product inventory can still overflow through valid command sequences

**Files modified:** `crates/example-commerce/src/product.rs`, `crates/example-commerce/src/order.rs`
**Commit:** 76fc6de
**Applied fix:** Added destination-counter overflow validation for reserve and release decisions, added max-bound regression coverage for both movement directions, and applied rustfmt cleanup to the example-commerce crate.

### WR-04: Malformed payload errors leave transaction cleanup implicit

**Files modified:** `crates/es-store-postgres/src/projection.rs`
**Commit:** 5503fb7
**Applied fix:** Wrapped projection event application and projector offset upsert in an explicit transaction result path, rolling back the SQLx transaction before returning the original projection error.

## Verification

- `cargo test -p es-store-postgres --test projections projector_offset_does_not_move_backward_on_stale_upsert`
- `cargo test -p example-commerce quantity_rejects_values_above_signed_inventory_range`
- `cargo test -p example-commerce product_projection_payload_roundtrips_product_created`
- `cargo test -p es-store-postgres --test projections`
- `cargo test -p example-commerce`
- `cargo test --workspace`
- `cargo test -p example-commerce product_rejects_inventory_movements_that_overflow_destination_counter -- --nocapture`
- `cargo test -p es-store-postgres --test projections projections_malformed_payload_does_not_advance_offset -- --exact --nocapture`
- `cargo test -p es-store-postgres --test projections -- --test-threads=1 --nocapture`

---

_Fixed: 2026-04-18T00:54:30Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

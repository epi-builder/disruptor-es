---
phase: 05-cqrs-projection-and-query-catch-up
fixed_at: 2026-04-18T00:54:30Z
review_path: .planning/phases/05-cqrs-projection-and-query-catch-up/05-REVIEW.md
iteration: 1
findings_in_scope: 2
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 05: Code Review Fix Report

**Fixed at:** 2026-04-18T00:54:30Z
**Source review:** `.planning/phases/05-cqrs-projection-and-query-catch-up/05-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 2
- Fixed: 2
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

## Verification

- `cargo test -p es-store-postgres --test projections projector_offset_does_not_move_backward_on_stale_upsert`
- `cargo test -p example-commerce quantity_rejects_values_above_signed_inventory_range`
- `cargo test -p example-commerce product_projection_payload_roundtrips_product_created`
- `cargo test -p es-store-postgres --test projections`
- `cargo test -p example-commerce`
- `cargo test --workspace`

---

_Fixed: 2026-04-18T00:54:30Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

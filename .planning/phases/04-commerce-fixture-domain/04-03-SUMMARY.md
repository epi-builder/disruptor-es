---
phase: 04-commerce-fixture-domain
plan: 03
subsystem: domain
tags: [rust, commerce, product, inventory, event-sourcing, proptest]

requires:
  - phase: 04-01
    provides: Commerce fixture IDs, Quantity value object, and compile-visible product module contract
provides:
  - Product aggregate implementation with create, adjust, reserve, and release decisions
  - Replayable product inventory events and nonnegative inventory invariants
  - Module-local unit and generated command-sequence tests for product inventory behavior
affects: [commerce-fixture-domain, example-commerce, cqrs-projections, process-manager-workflows]

tech-stack:
  added: []
  patterns:
    - Synchronous es_kernel::Aggregate implementation for product inventory decisions
    - Explicit inventory arithmetic validation before event emission
    - Proptest command-sequence invariant testing with replay comparison

key-files:
  created:
    - .planning/phases/04-commerce-fixture-domain/04-03-SUMMARY.md
  modified:
    - crates/example-commerce/src/product.rs

key-decisions:
  - "Product inventory uses signed i32 state for available/reserved counts while public quantity inputs remain positive Quantity values."
  - "Invalid inventory paths are rejected with typed ProductError variants before any replayable event is emitted."
  - "Product stream IDs and partition keys are derived from product IDs using the product-{id} format."

patterns-established:
  - "Product aggregate pattern: decide validates creation and inventory preconditions, then apply is the only state mutator."
  - "Inventory invariant testing pattern: generated command sequences apply only accepted events and compare manual state to es_kernel replay."

requirements-completed: [DOM-01, DOM-03, DOM-05]

duration: 5min 5s
completed: 2026-04-17
---

# Phase 04 Plan 03: Product Inventory Aggregate Summary

**Product aggregate with replayable inventory adjustment, reservation, release, and generated nonnegative-state checks**

## Performance

- **Duration:** 5min 5s
- **Started:** 2026-04-17T08:13:40Z
- **Completed:** 2026-04-17T08:18:45Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Implemented `Product` as an `es_kernel::Aggregate` with typed commands, events, replies, errors, stream IDs, partition keys, expected revisions, decisions, and replay application.
- Added product inventory tests for creation, reserve/release state transitions, explicit negative-path errors, and generated command sequences.
- Preserved inventory correctness by rejecting invalid arithmetic paths before event emission instead of clamping or using saturating arithmetic.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Add product inventory decision tests** - `85b6817` (test)
2. **Task 2 GREEN: Implement product aggregate inventory rules** - `377f098` (feat)
3. **Refactor: Document product inventory contracts** - `b8740ab` (refactor)

**Plan metadata:** docs commit created after this summary.

_Note: This plan used the TDD RED -> GREEN -> REFACTOR sequence._

## Files Created/Modified

- `crates/example-commerce/src/product.rs` - Product aggregate state machine, inventory commands/events/replies/errors, replay application, and module-local tests.
- `.planning/phases/04-commerce-fixture-domain/04-03-SUMMARY.md` - Execution summary and self-check record.

## Decisions Made

- Product inventory state stores `available_quantity` and `reserved_quantity` as `i32` to support signed inventory adjustments while continuing to accept positive `Quantity` values at reserve/release boundaries.
- Invalid adjustment, reservation, and release commands return typed `ProductError` values and emit no events.
- `CreateProduct` uses `ExpectedRevision::NoStream`; inventory commands use `ExpectedRevision::Any` to match the plan contract.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope changes.

## Issues Encountered

- During the RED run, concurrent in-progress `user.rs` work also produced compile errors. That file was outside this plan's ownership, so it was not modified or staged.
- Public enum payload fields initially emitted missing-docs warnings. A small refactor commit added field documentation while preserving the plan's acceptance-grep shapes.

## Verification

- `cargo test -p example-commerce product -- --nocapture` passed.
- `cargo test -p example-commerce product` passed.
- `rg "saturating_sub|saturating_add|Arc<Mutex|static mut|lazy_static" crates/example-commerce/src/product.rs` returned no matches.
- Acceptance greps for required tests, aggregate implementation, inventory fields, command variants, error variants, and `es_kernel::replay::<Product>` passed.
- Stub scan found no placeholder/TODO/FIXME/stub patterns in `product.rs`.

## TDD Gate Compliance

- RED gate: `85b6817` added failing product inventory tests before implementation.
- GREEN gate: `377f098` implemented the aggregate and made the targeted product tests pass.
- REFACTOR gate: `b8740ab` added documentation cleanup with tests still passing.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Product inventory behavior is ready for the order aggregate and later process-manager workflows to rely on typed product availability semantics. Phase 04 Plan 04 can reference `ProductId`, `Quantity`, and product inventory events without adding storage, adapter, broker, or shared mutable state concerns to the domain crate.

## Self-Check: PASSED

- Confirmed `.planning/phases/04-commerce-fixture-domain/04-03-SUMMARY.md` exists.
- Confirmed `crates/example-commerce/src/product.rs` exists.
- Confirmed task commits `85b6817`, `377f098`, and `b8740ab` exist in git history.
- Confirmed final product verification and forbidden-pattern scan passed.

---
*Phase: 04-commerce-fixture-domain*
*Completed: 2026-04-17*

---
phase: 04-commerce-fixture-domain
plan: 04
subsystem: domain
tags: [rust, commerce, order, event-sourcing, proptest]

requires:
  - phase: 04-02
    provides: User aggregate lifecycle and replayable user events
  - phase: 04-03
    provides: Product aggregate inventory state machine and generated invariant tests
provides:
  - Order aggregate implementation with place, confirm, reject, and cancel lifecycle decisions
  - Relationship-assumption validation for active users and available product lines by ID
  - Phase-level generated command-sequence tests for user, product, and order replay invariants
affects: [commerce-fixture-domain, example-commerce, process-manager-workflows, cqrs-projections]

tech-stack:
  added: []
  patterns:
    - Synchronous es_kernel::Aggregate implementation for order lifecycle decisions
    - Relationship validation by IDs plus command-supplied assumptions
    - Cross-aggregate generated command-sequence replay tests

key-files:
  created:
    - .planning/phases/04-commerce-fixture-domain/04-04-SUMMARY.md
  modified:
    - crates/example-commerce/src/order.rs
    - crates/example-commerce/src/tests.rs

key-decisions:
  - "Order stores UserId, ProductId, SKU, quantity, and product availability assumptions, not UserState or ProductState objects."
  - "PlaceOrder uses ExpectedRevision::NoStream; confirm, reject, and cancel use ExpectedRevision::Any."
  - "Generated Phase 04 tests use plain proptest command sequences rather than adding proptest-state-machine."

patterns-established:
  - "Order lifecycle pattern: decide rejects invalid placement and terminal transitions before emitting one typed event."
  - "Phase-level invariant pattern: generated commands apply only accepted events, compare manual state to es_kernel::replay, and assert invariants after each accepted event."

requirements-completed: [DOM-01, DOM-04, DOM-05, TEST-01]

duration: 4min 9s
completed: 2026-04-17
---

# Phase 04 Plan 04: Order Lifecycle and Generated Tests Summary

**Order lifecycle aggregate with ID-only relationship assumptions and generated replay/invariant coverage across commerce aggregates**

## Performance

- **Duration:** 4min 9s
- **Started:** 2026-04-17T08:21:23Z
- **Completed:** 2026-04-17T08:25:32Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Implemented `Order` as an `es_kernel::Aggregate` with typed placement, confirmation, rejection, cancellation, replies, events, and errors.
- Added relationship validation for inactive users, unavailable product lines, empty orders, duplicate placement, not-placed transitions, empty rejection reasons, and terminal-state transitions.
- Added generated command-sequence tests for user replay determinism, product nonnegative inventory, and order replay determinism.
- Verified `cargo test -p example-commerce && cargo test --workspace` passes.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Add failing order lifecycle tests** - `610c007` (test)
2. **Task 1 GREEN: Implement order aggregate lifecycle** - `bf60e13` (feat)
3. **Task 2: Add generated commerce command-sequence tests** - `1ec079c` (test)

**Plan metadata:** final docs commit created after this summary.

_Note: Task 2 is test-only coverage over already-implemented aggregate behavior. Its generated tests passed immediately because Task 1 and prior Phase 04 plans had already supplied the behavior under test._

## Files Created/Modified

- `crates/example-commerce/src/order.rs` - Order state machine, relationship-assumption validation, routing keys, expected revisions, replay application, and module-local lifecycle tests.
- `crates/example-commerce/src/tests.rs` - Shared deterministic metadata and proptest command-sequence tests for user, product, and order invariants.
- `.planning/phases/04-commerce-fixture-domain/04-04-SUMMARY.md` - Execution summary and self-check record.

## Decisions Made

- Kept order relationship state ID-only: `OrderState` stores `OrderId`, `UserId`, and `OrderLine` values containing `ProductId`, `Sku`, `Quantity`, and `product_available`.
- Used `OrderStatus::Draft` as the default pre-placement state, with `Placed`, `Confirmed`, `Rejected`, and `Cancelled` as lifecycle states.
- Kept generated testing dependency-light by using the existing `proptest` workspace dependency and not adding `proptest-state-machine`.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope changes; implementation stayed inside owned `example-commerce` files and planning summary output.

## Issues Encountered

- Task 2's TDD red gate could not produce a meaningful failing test after Task 1 because the generated tests validate behavior that already existed. The tests were committed as a test-only coverage task, and the full suite passed.
- The final worktree contains unrelated dirty files: `.planning/config.json`, `crates/es-kernel/src/lib.rs`, `crates/es-runtime/src/error.rs`, and `crates/es-runtime/tests/common/mod.rs`. They were not modified or staged by this plan.

## Verification

- `cargo test -p example-commerce order` passed.
- Task 1 acceptance greps for aggregate implementation, order status, `product_available`, exact command/error shapes, `AlreadyTerminal`, and no `UserState|ProductState` in `order.rs` passed.
- Task 2 acceptance greps for `user_command_sequence_is_replayable`, `product_generated_sequences_keep_inventory_nonnegative`, `order_command_sequence_is_replayable`, `proptest!`, product nonnegative assertions, and `OrderError::UnavailableProduct` passed.
- `cargo test -p example-commerce && cargo test --workspace` passed.
- Forbidden-pattern scan found no matches in `order.rs`. Matches in `tests.rs` were limited to intentional `UserState` and `ProductState` test imports/usages for generated replay checks.

## TDD Gate Compliance

- RED gate: `610c007` added failing order lifecycle tests before order implementation.
- GREEN gate: `bf60e13` implemented the order aggregate and made targeted order tests pass.
- Task 2 warning: generated phase-level tests passed on first run because required behavior was already implemented by Task 1 and prior plans.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 04 now has user, product, and order aggregates with generated replay and invariant coverage. Later projection and process-manager phases can consume committed commerce events while preserving the rule that cross-aggregate workflow coordination stays outside individual aggregates.

## Self-Check: PASSED

- Confirmed `.planning/phases/04-commerce-fixture-domain/04-04-SUMMARY.md` exists.
- Confirmed `crates/example-commerce/src/order.rs` exists.
- Confirmed `crates/example-commerce/src/tests.rs` exists.
- Confirmed task commits `610c007`, `bf60e13`, and `1ec079c` exist in git history.
- Confirmed final example-commerce and workspace verification passed.

---
*Phase: 04-commerce-fixture-domain*
*Completed: 2026-04-17*

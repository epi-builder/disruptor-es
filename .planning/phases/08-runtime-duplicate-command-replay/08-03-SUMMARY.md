---
phase: 08-runtime-duplicate-command-replay
plan: 03
subsystem: runtime
tags: [rust, http, process-manager, idempotency, replay]

requires:
  - phase: 08-runtime-duplicate-command-replay
    provides: durable command replay records and runtime pre-decision replay from Plans 08-01 and 08-02
provides:
  - HTTP duplicate retry regression coverage through the real order CommandEngine
  - Process-manager duplicate follow-up retry coverage through real product and order CommandEngines
  - Phase 08 Nyquist validation metadata marked green with requirement-level sampling commands
affects: [adapter-http, app, example-commerce, phase-08, milestone-audit]

tech-stack:
  added: []
  patterns:
    - Real gateway/runtime/store replay tests for external retry consumers
    - Test-local RuntimeEventStore scaffolding that records append and replay outcomes
    - Requirement-level validation sampling map for Phase 08

key-files:
  created:
    - .planning/phases/08-runtime-duplicate-command-replay/08-03-SUMMARY.md
  modified:
    - Cargo.lock
    - crates/adapter-http/Cargo.toml
    - crates/adapter-http/tests/commerce_api.rs
    - crates/app/Cargo.toml
    - crates/app/src/commerce_process_manager.rs
    - crates/example-commerce/src/product.rs
    - .planning/phases/08-runtime-duplicate-command-replay/08-VALIDATION.md

key-decisions:
  - "HTTP duplicate retry coverage uses the real order CommandEngine and a test RuntimeEventStore instead of adapter-local idempotency or manual reply injection."
  - "Process-manager retry coverage reuses deterministic pm:{manager}:{source_event_id}:... keys through real product/order CommandEngines instead of process-manager-local dedupe state."
  - "Phase 08 validation is recorded as requirement-level sampling because each plan contributes cross-cutting replay coverage."

patterns-established:
  - "External replay proof: duplicate external requests are verified at the adapter/process-manager boundary while replay correctness remains owned by runtime/store contracts."
  - "Replay-aware test store: test RuntimeEventStore implementations record append requests and expose stored CommandReplayRecord data for duplicate assertions."
  - "Validation sign-off: Phase validation maps plan waves to requirements and exact cargo commands once coverage exists."

requirements-completed: [STORE-03, RUNTIME-05, INT-04, API-01, API-03]

duration: 10min 35s
completed: 2026-04-19
---

# Phase 08 Plan 03: External Duplicate Replay Coverage Summary

**HTTP and process-manager duplicate retries now have regression coverage proving real runtime replay returns original committed outcomes without local dedupe caches.**

## Performance

- **Duration:** 10min 35s
- **Started:** 2026-04-19T14:36:46Z
- **Completed:** 2026-04-19T14:47:21Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `duplicate_place_order_retry_returns_original_response`, driving two identical HTTP requests through `router(HttpState { order_gateway: order_engine.gateway(), ... })` and `CommandEngine<Order, _, _>::process_one`.
- Added `process_manager_replayed_followups_return_original_outcomes`, processing the same source event twice before offset advancement through real product and order runtime engines.
- Asserted duplicate HTTP/process-manager retries do not append twice and preserve committed stream/global/event/reply data.
- Marked Phase 08 validation as Nyquist-compliant with requirement-level automated sampling across storage, runtime, HTTP, and process-manager replay paths.

## Task Commits

Each task was committed atomically:

1. **Task 08-03-01: Add HTTP duplicate retry response contract coverage** - `67fcd64` (test)
2. **Task 08-03-02: Add process-manager deterministic follow-up retry replay coverage** - `bdd0f62` (test)
3. **Task 08-03-03: Mark Phase 08 validation Nyquist-compliant after tests exist** - `9cac1da` (docs)

**Plan metadata:** created by final docs commit

## Files Created/Modified

- `crates/adapter-http/tests/commerce_api.rs` - Added real-engine duplicate HTTP retry coverage plus test order codec/store scaffolding.
- `crates/adapter-http/Cargo.toml` - Added test-only direct dependencies needed by adapter integration tests.
- `crates/app/src/commerce_process_manager.rs` - Added real-engine process-manager retry coverage plus test product/order codecs and replay-aware stores.
- `crates/app/Cargo.toml` - Added direct dependencies used by app test scaffolding.
- `crates/example-commerce/src/product.rs` - Added serde derives to `ProductReply` so product command replies can be encoded for durable replay.
- `Cargo.lock` - Reflected direct crate dependency graph changes.
- `.planning/phases/08-runtime-duplicate-command-replay/08-VALIDATION.md` - Marked Phase 08 validation green and replaced Wave 0 placeholders with automated sampling evidence.

## Decisions Made

- Kept idempotency replay out of the HTTP adapter and process manager; tests prove those consumers receive runtime replayed outcomes through `CommandGateway`.
- Used deterministic request metadata in the HTTP duplicate helper so full response-body equality also proves correlation ID preservation.
- Kept validation rows at requirement/plan level rather than expanding every individual task row, because the replay guarantees are cross-cutting.

## Verification

- `cargo test -p adapter-http duplicate_place_order_retry_returns_original_response -- --nocapture` - passed.
- `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` - passed.
- `cargo test -p es-runtime runtime_duplicate -- --nocapture` - passed.
- `cargo test -p es-store-postgres command_replay -- --nocapture` - passed.
- `cargo test --workspace duplicate -- --nocapture` - passed; existing missing-docs warning remains in `crates/es-runtime/tests/shard_disruptor.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added adapter integration-test direct dependencies**
- **Found during:** Task 08-03-01 (HTTP duplicate retry coverage)
- **Issue:** The adapter test needed `es-store-postgres`, `futures`, and `serde_json` directly for test-local runtime store and codec scaffolding.
- **Fix:** Added those dependencies under `crates/adapter-http` dev-dependencies.
- **Files modified:** `crates/adapter-http/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo test -p adapter-http duplicate_place_order_retry_returns_original_response -- --nocapture`
- **Committed in:** `67fcd64`

**2. [Rule 3 - Blocking] Added app test direct dependencies**
- **Found during:** Task 08-03-02 (process-manager retry coverage)
- **Issue:** The app crate needed direct `futures` and `serde` dependencies for test-local `RuntimeEventStore` futures and generic stored-event helpers.
- **Fix:** Added direct workspace dependencies to `crates/app/Cargo.toml`.
- **Files modified:** `crates/app/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture`
- **Committed in:** `bdd0f62`

**3. [Rule 3 - Blocking] Added serde derives to ProductReply**
- **Found during:** Task 08-03-02 (process-manager retry coverage)
- **Issue:** The plan required product reply replay payloads to use `serde_json::to_value(reply)`, but `ProductReply` did not implement serde traits.
- **Fix:** Added `Serialize` and `Deserialize` derives to `ProductReply`, matching `OrderReply`.
- **Files modified:** `crates/example-commerce/src/product.rs`
- **Verification:** `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture`
- **Committed in:** `bdd0f62`

---

**Total deviations:** 3 auto-fixed (3 Rule 3 blocking issues).
**Impact on plan:** All fixes were limited to enabling the planned replay tests and reply payload contract. No adapter or process-manager-local idempotency logic was added.

## Issues Encountered

- The TDD-marked tasks were regression coverage on top of Plan 08-02 behavior, so the new tests passed once compiled instead of requiring production GREEN changes in this plan.
- Cargo emitted an existing missing-docs warning for `tests/shard_disruptor.rs`; it did not fail verification.

## Known Stubs

None. Stub scan found no TODO/FIXME/placeholder/empty-data patterns in files created or modified by this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 08 is complete. The milestone audit replay gap is closed with storage, runtime, HTTP, and process-manager automated coverage proving duplicate retries return original committed outcomes.

## Self-Check: PASSED

- Created/modified files exist on disk.
- Task commits exist in git history: `67fcd64`, `bdd0f62`, `9cac1da`.
- Required plan verification commands passed, including `cargo test --workspace duplicate -- --nocapture`.
- Stub scan found no unimplemented stubs in plan-modified source or validation files.

---
*Phase: 08-runtime-duplicate-command-replay*
*Completed: 2026-04-19*

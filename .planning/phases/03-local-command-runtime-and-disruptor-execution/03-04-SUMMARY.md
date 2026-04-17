---
phase: 03-local-command-runtime-and-disruptor-execution
plan: 04
subsystem: runtime
tags: [rust, tokio, disruptor, event-store, command-engine, cqrs]
requires:
  - phase: 03-local-command-runtime-and-disruptor-execution
    provides: Runtime contracts, bounded gateway ingress, partition routing, shard-local cache, and disruptor handoff from Plans 03-01 through 03-03
provides:
  - Commit-gated shard command processing with cache miss rehydration
  - Duplicate append handling that preserves durable cached state
  - Typed conflict/domain/codec/rehydration error replies without cache corruption
  - Production CommandEngine wiring gateway receive, shard handoff, durable append, codec, and replies
  - Phase 03 Nyquist validation marked green
affects: [runtime, adapter-http, adapter-grpc, storage, phase-04-api]
tech-stack:
  added: []
  patterns: [commit-gated replies, cache-after-commit, duplicate-preserves-cache, production engine process_one loop]
key-files:
  created:
    - crates/es-runtime/src/engine.rs
    - crates/es-runtime/tests/runtime_flow.rs
    - .planning/phases/03-local-command-runtime-and-disruptor-execution/03-04-SUMMARY.md
  modified:
    - crates/es-runtime/src/command.rs
    - crates/es-runtime/src/shard.rs
    - crates/es-runtime/src/gateway.rs
    - crates/es-runtime/src/lib.rs
    - .planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md
key-decisions:
  - "Successful command replies are built from es-store-postgres CommittedAppend only after RuntimeEventStore::append returns Committed or Duplicate."
  - "Duplicate append outcomes refresh shard-local dedupe records but never apply newly decided events into AggregateCache."
  - "CommandEngine::process_one is the production integration point for gateway receive, shard release drain, durable append, codec conversion, and reply delivery."
patterns-established:
  - "Cache misses rehydrate from RuntimeEventStore::load_rehydration, decode optional snapshots, replay stored events, then decide."
  - "Failure branches send typed RuntimeError replies and preserve the pre-call cache value after any legitimate rehydration."
requirements-completed: [RUNTIME-01, RUNTIME-03, RUNTIME-05, RUNTIME-06]
duration: 12 min
completed: 2026-04-17
---

# Phase 03 Plan 04: Commit-Gated Runtime Engine Summary

**Durable append-gated command processing with cache-safe conflict handling and production CommandEngine wiring**

## Performance

- **Duration:** 12 min
- **Started:** 2026-04-17T04:25:45Z
- **Completed:** 2026-04-17T04:37:45Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `ShardState::process_next_handoff` so cache misses rehydrate from durable storage before `Aggregate::decide`, fresh commits update cache only after append succeeds, and duplicate append outcomes preserve the current durable cache state.
- Added runtime flow coverage for commit-gated replies, duplicate handling, dropped reply receivers, conflicts, domain errors, codec errors, rehydration failures, overload, and end-to-end engine processing.
- Added `CommandEngine` and `CommandEngineConfig` as the production loop that owns bounded gateway receive, shard handoff, disruptor release drain, store append, codec conversion, and one-shot reply delivery.
- Updated Phase 03 validation to `nyquist_compliant: true` and `wave_0_complete: true` after runtime, storage, forbidden-pattern, and workspace verification passed.

## Task Commits

1. **Task 1 RED: Add failing commit-gated shard flow tests** - `7d0c2e4` (`test`)
2. **Task 1 GREEN: Implement commit-gated shard processing** - `8786fd6` (`feat`)
3. **Task 2: Cover conflict and failure cache preservation** - `5651d0a` (`test`)
4. **Task 3: Wire production command engine flow** - `78e2319` (`feat`)

## Files Created/Modified

- `crates/es-runtime/src/engine.rs` - Production command engine and validated engine configuration.
- `crates/es-runtime/src/command.rs` - Added `CommandOutcome::new` and snapshot decode to the runtime event codec boundary.
- `crates/es-runtime/src/shard.rs` - Added commit-gated handoff processing, cache miss rehydration, append outcome handling, dedupe recording, and typed error replies.
- `crates/es-runtime/src/gateway.rs` - Added a manual `Clone` implementation so gateways can be cloned without requiring `A: Clone`.
- `crates/es-runtime/src/lib.rs` - Re-exported `CommandEngine` and `CommandEngineConfig`.
- `crates/es-runtime/tests/runtime_flow.rs` - Added end-to-end runtime flow coverage across commit, duplicate, conflict, failure, overload, and engine paths.
- `.planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md` - Marked Phase 03 validation green after verification.

## Decisions Made

- Duplicate durable append outcomes are successful command outcomes, but newly decided events from that repeated command are not applied to cache because PostgreSQL dedupe is authoritative.
- Cache miss rehydration commits the storage-supplied state before decision, so later failure branches preserve durable state rather than deleting a legitimate rehydration.
- Gateway cloning is implemented manually to avoid imposing an unnecessary `A: Clone` bound on aggregate implementations.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Removed unnecessary aggregate Clone bound from gateway handles**
- **Found during:** Task 3 (Implement production command engine loop and complete validation)
- **Issue:** `#[derive(Clone)]` on `CommandGateway<A>` required `A: Clone`, preventing `CommandEngine::gateway()` from returning a clone for normal aggregate types.
- **Fix:** Replaced the derive with a manual `Clone` implementation that clones only the router and Tokio sender.
- **Files modified:** `crates/es-runtime/src/gateway.rs`
- **Verification:** `cargo test -p es-runtime runtime_engine -- --nocapture`
- **Committed in:** `78e2319`

---

**Total deviations:** 1 auto-fixed (1 missing critical API bound)
**Impact on plan:** The fix preserves the planned public API and avoids leaking test-only aggregate trait bounds into production adapters.

## TDD Gate Compliance

- Task 1 RED gate: `7d0c2e4`
- Task 1 GREEN gate: `8786fd6`
- Task 2 added explicit failure-branch coverage in `5651d0a`; the tests passed immediately because Task 1's processor implementation already covered the shared error-handling machinery.

## Verification

- `cargo test -p es-runtime reply_is_sent_after_append_commit && cargo test -p es-runtime cache_miss_rehydrates_before_decide && cargo test -p es-runtime duplicate_append_returns_successful_command_outcome && cargo test -p es-runtime duplicate_after_warmed_cache_does_not_apply_newly_decided_events && cargo test -p es-runtime reply_drop_after_append_still_advances_cache_and_dedupe` - passed
- `cargo test -p es-runtime conflict_does_not_mutate_cache && cargo test -p es-runtime domain_error_does_not_append_or_mutate_cache && cargo test -p es-runtime codec_error_does_not_append_or_mutate_cache && cargo test -p es-runtime rehydration_error_does_not_decide_append_or_mutate_cache` - passed
- `cargo test -p es-runtime runtime_engine -- --nocapture && cargo test -p es-runtime runtime_flow -- --nocapture` - passed
- `cargo test -p es-runtime` - passed
- `cargo test -p es-store-postgres` - passed
- `! rg 'Arc<Mutex<.*(State|Cache|HashMap)|RwLock<.*(State|Cache|HashMap)' crates/es-runtime/src` - passed
- `! rg 'block_on|Runtime::new|Handle::current' crates/es-runtime/src` - passed
- `cargo test --workspace` - passed

## Known Stubs

- `crates/es-runtime/src/shard.rs:49` - `ShardHandoffToken::placeholder` remains the intentional internal sentinel factory introduced in Plan 03-03 for preallocated disruptor ring slots. Published slots are overwritten before release and it does not flow to user-facing output.

## Issues Encountered

- `cargo fmt --package es-runtime` reformatted a few unrelated existing files; those unrelated formatting diffs were discarded before commits so this plan stayed within its write scope.
- The existing `missing documentation for the crate` warning still appears for `crates/es-runtime/tests/shard_disruptor.rs`; tests pass and the warning predates this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 03 is complete. Runtime command execution now has durable source-of-truth commit semantics and is ready for adapter phases to submit commands through `CommandGateway`/`CommandEngine`.

## Threat Flags

None.

## Self-Check: PASSED

- Summary file exists: `.planning/phases/03-local-command-runtime-and-disruptor-execution/03-04-SUMMARY.md`.
- Required runtime files exist: `engine.rs`, `command.rs`, `shard.rs`, `gateway.rs`, `lib.rs`, and `tests/runtime_flow.rs`.
- Task commits exist: `7d0c2e4`, `8786fd6`, `5651d0a`, `78e2319`.
- Phase 03 validation contains `nyquist_compliant: true` and `wave_0_complete: true`.

---
*Phase: 03-local-command-runtime-and-disruptor-execution*
*Completed: 2026-04-17*

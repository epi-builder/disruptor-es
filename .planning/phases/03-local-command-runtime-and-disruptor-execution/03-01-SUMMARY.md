---
phase: 03-local-command-runtime-and-disruptor-execution
plan: 01
subsystem: runtime
tags: [rust, tokio, disruptor, futures, event-store, command-runtime]
requires:
  - phase: 02-durable-event-store-source-of-truth
    provides: PostgreSQL append, conflict, dedupe, committed append, and rehydration contracts
provides:
  - RuntimeError and RuntimeResult public error contract
  - CommandEnvelope, CommandReply, CommandOutcome, and RuntimeEventCodec contracts
  - RuntimeEventStore trait, PostgresRuntimeEventStore adapter, and fake-store test harness
affects: [runtime, adapter-http, adapter-grpc, shard-runtime, command-engine]
tech-stack:
  added: [disruptor 4.0.0, futures 0.3.32, tracing 0.1.44, twox-hash 2.1.2]
  patterns: [typed runtime errors, aggregate-derived command envelopes, boxed-future store seam, commit-gated command outcomes]
key-files:
  created:
    - crates/es-runtime/src/error.rs
    - crates/es-runtime/src/command.rs
    - crates/es-runtime/src/store.rs
    - crates/es-runtime/tests/common/mod.rs
    - crates/es-runtime/tests/store.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/es-runtime/Cargo.toml
    - crates/es-runtime/src/lib.rs
    - .planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md
key-decisions:
  - "Expose runtime errors as typed variants for overload, unavailable, invalid capacity, conflicts, domain, codec, and store failures."
  - "Keep CommandOutcome tied to CommittedAppend so successful replies carry durable event-store positions instead of disruptor sequence state."
  - "Use a boxed-future RuntimeEventStore trait to test runtime behavior without PostgreSQL while preserving the Phase 2 PostgresEventStore boundary."
patterns-established:
  - "Runtime command envelopes precompute stream ID, partition key, and expected revision through the Aggregate contract."
  - "Runtime storage is accessed through a Clone + Send + Sync trait with test-only Arc<Mutex<Vec<AppendRequest>>> observation confined to tests."
requirements-completed: [RUNTIME-01, RUNTIME-02, RUNTIME-03, RUNTIME-04, RUNTIME-05, RUNTIME-06]
duration: 11 min
completed: 2026-04-17
---

# Phase 03 Plan 01: Runtime Contract Foundation Summary

**Typed local command runtime contracts with commit-aware replies, storage-test seam, and Wave 0 validation markers**

## Performance

- **Duration:** 11 min
- **Started:** 2026-04-17T03:50:03Z
- **Completed:** 2026-04-17T04:01:24Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- Added exact Phase 03 runtime dependencies to the workspace and exposed the `es-runtime` contract facade.
- Created typed runtime errors, command envelopes, one-shot reply outcomes, and the event codec boundary.
- Added the runtime store trait, PostgreSQL adapter, fake-store harness, and store contract tests.
- Updated Phase 03 validation metadata to record the Wave 0 contract artifacts while keeping phase-wide Nyquist and Wave 0 completion flags false.

## Task Commits

1. **Task 1: Add runtime dependencies and facade exports** - `3e0790a` (`chore`)
2. **Task 2 RED: Define runtime errors and command contracts** - `ad4ab3b` (`test`)
3. **Task 2 GREEN: Define runtime errors and command contracts** - `ee7d4a1` (`feat`)
4. **Task 3 RED: Add runtime store trait and fake-store test harness** - `5c136c4` (`test`)
5. **Task 3 GREEN: Add runtime store trait and fake-store test harness** - `9ac1588` (`feat`)

## Files Created/Modified

- `Cargo.toml` - Added runtime workspace dependencies and Tokio `sync`.
- `Cargo.lock` - Locked the new runtime dependency graph.
- `crates/es-runtime/Cargo.toml` - Added runtime production and dev dependencies.
- `crates/es-runtime/src/lib.rs` - Re-exported runtime contract modules from the public facade.
- `crates/es-runtime/src/error.rs` - Added `RuntimeError`, `RuntimeResult`, and conflict-aware store error mapping.
- `crates/es-runtime/src/command.rs` - Added command envelope, reply, outcome, and codec contracts.
- `crates/es-runtime/src/store.rs` - Added `RuntimeEventStore` and `PostgresRuntimeEventStore`.
- `crates/es-runtime/tests/common/mod.rs` - Added test-only `FakeRuntimeEventStore`.
- `crates/es-runtime/tests/store.rs` - Added runtime store contract tests.
- `.planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md` - Marked Wave 0 contract files as present.

## Decisions Made

- Used structured `RuntimeError::Conflict` for `StoreError::StreamConflict` so later shard code can preserve cache state on OCC conflicts.
- Stored aggregate-derived stream, partition, and revision fields directly in `CommandEnvelope` so later routing does not recompute caller-visible metadata.
- Kept fake-store observation mutexes in integration tests only; production runtime source has no global business-state mutex.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added temporary store module during Task 2 TDD**
- **Found during:** Task 2 (Define runtime errors and command contracts)
- **Issue:** Task 1 required `lib.rs` to re-export `store`, but Task 3 was the first planned task to create `store.rs`. That made Task 2 filtered tests fail before reaching the intended error/envelope assertions.
- **Fix:** Added a minimal store module placeholder in the RED commit, then replaced it with the full `RuntimeEventStore` and `PostgresRuntimeEventStore` implementation in Task 3.
- **Files modified:** `crates/es-runtime/src/store.rs`
- **Verification:** `cargo test -p es-runtime runtime_error && cargo test -p es-runtime command_envelope && cargo test -p es-runtime store`
- **Committed in:** `ad4ab3b`, replaced by `9ac1588`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The deviation only resolved task ordering caused by the facade export requirement. The final store module matches Task 3's planned contract.

## TDD Gate Compliance

- RED gate for Task 2: `ad4ab3b`
- GREEN gate for Task 2: `ee7d4a1`
- RED gate for Task 3: `5c136c4`
- GREEN gate for Task 3: `9ac1588`

## Verification

- `cargo metadata --format-version 1 --no-deps` - passed
- `cargo test -p es-runtime runtime_error` - passed
- `cargo test -p es-runtime command_envelope` - passed
- `cargo test -p es-runtime store` - passed
- `! rg 'Arc<Mutex<.*(State|Cache|HashMap)' crates/es-runtime/src` - passed

## Known Stubs

None.

## Issues Encountered

None beyond the auto-fixed task ordering blocker documented above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 03-02 to implement bounded ingress and stable partition routing on top of these runtime contracts.

## Self-Check: PASSED

- Summary file created.
- Task commits exist: `3e0790a`, `ad4ab3b`, `ee7d4a1`, `5c136c4`, `9ac1588`.
- Required contract files exist: `error.rs`, `command.rs`, `store.rs`, and fake-store test harness.

---
*Phase: 03-local-command-runtime-and-disruptor-execution*
*Completed: 2026-04-17*

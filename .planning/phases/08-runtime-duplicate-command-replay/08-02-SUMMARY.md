---
phase: 08-runtime-duplicate-command-replay
plan: 02
subsystem: runtime
tags: [rust, event-sourcing, idempotency, replay, shard-cache]

requires:
  - phase: 08-runtime-duplicate-command-replay
    provides: durable CommandReplyPayload and CommandReplayRecord lookup from Plan 08-01
provides:
  - RuntimeEventCodec reply encode/decode contract
  - Shard-local CommandReplayRecord dedupe cache records
  - RuntimeEventStore durable command replay lookup boundary
  - Pre-decision duplicate replay from cache and PostgreSQL
  - Runtime tests proving duplicates skip rehydrate, decide, encode, and append
affects: [es-runtime, es-store-postgres, app, example-commerce, phase-08]

tech-stack:
  added: []
  patterns:
    - Typed reply payload codec hooks on RuntimeEventCodec
    - Pre-decision local and durable idempotency replay in ShardState
    - Duplicate append race replay through durable lookup instead of fresh decision reply

key-files:
  created:
    - .planning/phases/08-runtime-duplicate-command-replay/08-02-SUMMARY.md
  modified:
    - crates/es-runtime/src/command.rs
    - crates/es-runtime/src/cache.rs
    - crates/es-runtime/src/store.rs
    - crates/es-runtime/src/shard.rs
    - crates/es-runtime/tests/runtime_flow.rs
    - crates/es-runtime/tests/common/mod.rs
    - crates/es-runtime/tests/shard_disruptor.rs
    - crates/app/src/stress.rs
    - crates/example-commerce/src/order.rs

key-decisions:
  - "Runtime duplicate replay now checks shard-local dedupe first, then durable tenant/idempotency lookup, before aggregate rehydration or decision."
  - "Runtime codecs own typed reply payload validation so stored replay records are decoded without calling aggregate decide."
  - "Duplicate append races require a durable CommandReplayRecord lookup and return a codec error when no typed replay row exists."

patterns-established:
  - "Replay helper: replay_command_outcome decodes CommandReplayRecord.reply through RuntimeEventCodec and pairs it with the original committed append."
  - "Runtime store lookup: RuntimeEventStore exposes lookup_command_replay while PostgresRuntimeEventStore remains a thin PostgresEventStore delegate."
  - "First execution persistence: command replies are encoded before append and attached via AppendRequest::with_command_reply_payload."

requirements-completed: [STORE-03, RUNTIME-03, RUNTIME-05]

duration: 10min 5s
completed: 2026-04-19
---

# Phase 08 Plan 02: Runtime Duplicate Command Replay Summary

**Runtime duplicate commands now replay original committed replies from shard-local or durable idempotency records before aggregate state is read or domain logic runs.**

## Performance

- **Duration:** 10min 5s
- **Started:** 2026-04-19T14:23:52Z
- **Completed:** 2026-04-19T14:33:57Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments

- Added `RuntimeEventCodec::encode_reply` and `decode_reply` so runtime replay can round-trip typed replies without calling `A::decide`.
- Changed `DedupeRecord` to carry full `CommandReplayRecord` data and added runtime durable lookup through `RuntimeEventStore::lookup_command_replay`.
- Added pre-decision duplicate branches in `ShardState::process_next_handoff`: shard-local cache first, PostgreSQL replay lookup second, then normal rehydrate/decide only on misses.
- Persisted encoded command replies on first execution with `AppendRequest::with_command_reply_payload`.
- Replaced duplicate append race replies with durable replay decoding, including a typed codec error when no replay record exists.
- Added runtime tests for warm cache replay, durable cache-miss replay, state-mutation replay, and duplicate append race replay.

## Task Commits

Each task was committed atomically:

1. **Task 08-02-01 RED: Add failing runtime reply codec contract** - `38524e6` (test)
2. **Task 08-02-01 GREEN: Add runtime command replay contracts** - `4b86611` (feat)
3. **Task 08-02-02 RED: Add failing duplicate replay ordering tests** - `30b9d34` (test)
4. **Task 08-02-02 GREEN: Replay duplicates before aggregate execution** - `808a52d` (feat)

**Plan metadata:** created by final docs commit

## Files Created/Modified

- `crates/es-runtime/src/command.rs` - Added runtime reply encode/decode codec hooks.
- `crates/es-runtime/src/cache.rs` - Changed dedupe records to store full replay records.
- `crates/es-runtime/src/store.rs` - Added durable command replay lookup to the runtime store trait and PostgreSQL adapter.
- `crates/es-runtime/src/shard.rs` - Added pre-decision replay branches, first-execution reply payload persistence, and duplicate append replay decoding.
- `crates/es-runtime/tests/runtime_flow.rs` - Added reply codec, warm replay, durable replay, and append-race replay coverage.
- `crates/es-runtime/tests/common/mod.rs` - Updated fake runtime store for the new lookup method.
- `crates/es-runtime/tests/shard_disruptor.rs` - Updated dedupe cache assertions for full replay records.
- `crates/app/src/stress.rs` - Added order reply replay codec support and measured store lookup delegation.
- `crates/example-commerce/src/order.rs` - Added serde derives to `OrderReply` for durable replay payload serialization.
- `.planning/phases/08-runtime-duplicate-command-replay/08-02-SUMMARY.md` - Captures plan outcome and verification evidence.

## Decisions Made

- Kept duplicate replay inside shard-owned runtime state instead of adding adapter-side or global idempotency maps.
- Used codec-owned reply validation for replay payloads to keep storage generic and runtime/domain typed.
- Treated missing durable replay data in the duplicate append race branch as a codec error because returning a fresh decision reply would violate idempotency.

## Verification

- `cargo test -p es-runtime command_replay_contract -- --nocapture` - passed.
- `cargo test -p es-runtime runtime_duplicate -- --nocapture` - passed.
- `cargo test -p es-runtime duplicate_replay_returns_original_reply_after_state_mutation -- --nocapture` - passed.
- `cargo test -p app single_service_stress_smoke -- --nocapture` - passed.
- `cargo test -p es-runtime` - passed; 29 tests passed. Existing missing-docs warning remains in `tests/shard_disruptor.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added serde derives to OrderReply**
- **Found during:** Task 08-02-01 (runtime reply codec and durable lookup contracts)
- **Issue:** The plan required `OrderCodec` to use `serde_json::to_value(reply)` and `serde_json::from_value::<OrderReply>`, but `OrderReply` did not implement `Serialize` or `Deserialize`.
- **Fix:** Added serde derives to `OrderReply`.
- **Files modified:** `crates/example-commerce/src/order.rs`
- **Verification:** `cargo test -p app single_service_stress_smoke -- --nocapture`
- **Committed in:** `4b86611`

**2. [Rule 3 - Blocking] Updated runtime test helpers for new replay contracts**
- **Found during:** Task 08-02-01 and Task 08-02-02
- **Issue:** Changing `RuntimeEventStore` and `DedupeRecord` broke fake stores and existing dedupe cache assertions outside the primary task files.
- **Fix:** Added no-op/sequence replay lookup support to fake stores and updated dedupe cache tests to store full replay records.
- **Files modified:** `crates/es-runtime/tests/common/mod.rs`, `crates/es-runtime/tests/shard_disruptor.rs`, `crates/es-runtime/tests/runtime_flow.rs`
- **Verification:** `cargo test -p es-runtime`
- **Committed in:** `4b86611`, `808a52d`

---

**Total deviations:** 2 auto-fixed (2 Rule 3 blocking issues).
**Impact on plan:** Both fixes were required to compile and verify the planned runtime replay contract. No architectural scope change.

## Issues Encountered

Cargo emitted an existing missing-docs warning for the `tests/shard_disruptor.rs` integration test crate. It did not fail builds or plan verification.

## Known Stubs

No new stubs were introduced. The scan found the pre-existing `ShardHandoffToken::placeholder` used to initialize the disruptor path; it is functional test/runtime scaffolding, not an unimplemented feature.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 08-03. Runtime duplicate replay is now available to HTTP and process-manager flows through the same command gateway, with cache-hit, durable-hit, and duplicate-append race behavior covered in runtime tests.

## Self-Check: PASSED

- Created/modified files exist on disk.
- Task commits exist in git history: `38524e6`, `4b86611`, `30b9d34`, `808a52d`.
- Required plan verification commands passed.
- Stub scan found no new unimplemented stubs.

---
*Phase: 08-runtime-duplicate-command-replay*
*Completed: 2026-04-19*

---
phase: 08-runtime-duplicate-command-replay
plan: 01
subsystem: storage
tags: [rust, postgres, sqlx, idempotency, event-store, replay]

requires:
  - phase: 02-durable-event-store-source-of-truth
    provides: command_dedup table, append transaction, committed append payloads
  - phase: 07-adapters-observability-stress-and-template-guidance
    provides: milestone audit gap requiring duplicate command replay closure
provides:
  - Store-owned CommandReplyPayload and CommandReplayRecord DTOs
  - AppendRequest support for optional durable command reply payloads
  - PostgreSQL command_dedup persistence of typed replay records
  - Public tenant-scoped PostgresEventStore::lookup_command_replay API
affects: [es-store-postgres, es-runtime, adapter-http, app, phase-08]

tech-stack:
  added: []
  patterns:
    - Backward-compatible command_dedup response_payload decoding
    - Tenant-scoped durable idempotency replay lookup
    - TDD RED/GREEN commits for storage replay behavior

key-files:
  created:
    - .planning/phases/08-runtime-duplicate-command-replay/08-01-SUMMARY.md
  modified:
    - crates/es-store-postgres/src/error.rs
    - crates/es-store-postgres/src/models.rs
    - crates/es-store-postgres/src/lib.rs
    - crates/es-store-postgres/src/sql.rs
    - crates/es-store-postgres/src/event_store.rs
    - crates/es-store-postgres/tests/dedupe.rs

key-decisions:
  - "Store typed command replies as CommandReplayRecord { append, reply } in command_dedup.response_payload for new appends."
  - "Keep legacy CommittedAppend response_payload rows readable for append dedupe while typed replay lookup returns None for legacy rows."
  - "Use tenant_id plus idempotency_key for durable replay lookup, matching the existing command_dedup primary key and tenant boundary."

patterns-established:
  - "Durable replay wrapper: new command dedupe rows with reply payloads serialize CommandReplayRecord, preserving the original append summary and typed reply payload together."
  - "Compatibility decode: append duplicate paths decode CommandReplayRecord first, then fall back to legacy CommittedAppend JSON."
  - "Typed replay lookup: lookup_command_replay returns Some only for typed replay rows, None for missing or legacy append-only rows, and DedupeResultDecode for corrupt payloads."

requirements-completed: [STORE-03, RUNTIME-05]

duration: 6min
completed: 2026-04-19
---

# Phase 08 Plan 01: Durable Command Replay Substrate Summary

**PostgreSQL command dedupe now stores typed command reply replay records beside committed append summaries, with tenant-scoped lookup for runtime cache misses.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-04-19T14:13:51Z
- **Completed:** 2026-04-19T14:19:51Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added `CommandReplyPayload` validation for reply type, schema version, and payload size using existing store error conventions.
- Added `CommandReplayRecord { append, reply }` and optional `AppendRequest::command_reply_payload` support.
- Persisted typed replay wrappers in `command_dedup.response_payload` when an append request includes a reply payload.
- Added `PostgresEventStore::lookup_command_replay`, scoped by tenant and idempotency key, with restart/cache-miss integration coverage.
- Preserved existing duplicate append behavior for legacy `CommittedAppend` payloads.

## Task Commits

Each task was committed atomically:

1. **Task 08-01-01 RED: Add failing command reply payload tests** - `fe5a5bf` (test)
2. **Task 08-01-01 GREEN: Add command reply replay DTOs** - `284eac0` (feat)
3. **Task 08-01-02 RED: Add failing durable replay lookup tests** - `73f910a` (test)
4. **Task 08-01-02 GREEN: Persist durable command replay records** - `5d88689` (feat)

**Plan metadata:** created by final docs commit

## Files Created/Modified

- `crates/es-store-postgres/src/error.rs` - Added `StoreError::InvalidReplyType`.
- `crates/es-store-postgres/src/models.rs` - Added replay DTOs, append request reply payload support, and model unit tests.
- `crates/es-store-postgres/src/lib.rs` - Exported `CommandReplyPayload` and `CommandReplayRecord`.
- `crates/es-store-postgres/src/sql.rs` - Added wrapper encoding, compatibility decoding, and durable replay lookup SQL.
- `crates/es-store-postgres/src/event_store.rs` - Added public `lookup_command_replay` facade method.
- `crates/es-store-postgres/tests/dedupe.rs` - Added durable replay round-trip, restart, and tenant-scope integration tests.
- `.planning/phases/08-runtime-duplicate-command-replay/08-01-SUMMARY.md` - Captures plan outcome and verification evidence.

## Decisions Made

- Persisted typed command reply data in the existing `command_dedup.response_payload` JSONB column instead of adding a new table or deriving replies from events.
- Kept append duplicate replay backward-compatible by decoding `CommandReplayRecord` first and falling back to legacy `CommittedAppend`.
- Kept typed replay lookup intentionally stricter: missing rows and legacy append-only rows return `None`, while corrupt payloads return `StoreError::DedupeResultDecode`.

## Verification

- `cargo test -p es-store-postgres command_reply_payload -- --nocapture` - passed; 3 matching unit tests passed.
- `cargo test -p es-store-postgres command_replay -- --nocapture` - passed; 3 matching integration tests passed.
- `cargo test -p es-store-postgres duplicate_idempotency_key_returns_original_result -- --nocapture` - passed; existing duplicate append behavior still returns the original result.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope changes.

## Issues Encountered

None. Cargo briefly waited on a build lock when two verification commands were launched together; both completed successfully, and final verification was rerun sequentially.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 08-02. Runtime code can now persist reply payloads with append requests and call `PostgresEventStore::lookup_command_replay` on cache misses without re-running aggregate decisions.

## Self-Check: PASSED

- Created/modified files exist on disk.
- Task commits exist in git history: `fe5a5bf`, `284eac0`, `73f910a`, `5d88689`.
- Required plan verification commands passed.

---
*Phase: 08-runtime-duplicate-command-replay*
*Completed: 2026-04-19*

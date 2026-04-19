---
phase: 09-tenant-scoped-runtime-aggregate-cache
plan: 01
subsystem: runtime
tags: [rust, event-sourcing, tenant-isolation, aggregate-cache, shard-runtime]

requires:
  - phase: 08-runtime-duplicate-command-replay
    provides: pre-decision shard-local and durable duplicate replay ordering
provides:
  - Tenant-scoped AggregateCacheKey public runtime API
  - Shard processing cache hit/fill/commit keyed by tenant and stream
  - Same-stream different-tenant runtime regression coverage
  - Phase 09 Nyquist validation evidence
affects: [es-runtime, tenant-isolation, phase-09, phase-10]

tech-stack:
  added: []
  patterns:
    - Typed composite HashMap key for shard-local aggregate cache state
    - One AggregateCacheKey constructed per handoff after duplicate replay misses
    - Validation evidence updated only after targeted and full runtime tests pass

key-files:
  created:
    - .planning/phases/09-tenant-scoped-runtime-aggregate-cache/09-01-SUMMARY.md
  modified:
    - crates/es-runtime/src/cache.rs
    - crates/es-runtime/src/lib.rs
    - crates/es-runtime/src/shard.rs
    - crates/es-runtime/tests/shard_disruptor.rs
    - crates/es-runtime/tests/runtime_flow.rs
    - .planning/phases/09-tenant-scoped-runtime-aggregate-cache/09-VALIDATION.md

key-decisions:
  - "Aggregate cache identity is a first-class AggregateCacheKey containing TenantId and StreamId, not a string concat or nested global map."
  - "ShardState constructs one AggregateCacheKey after duplicate replay misses and reuses it for cache hit, rehydration fill, and committed cache replacement."
  - "Phase 09 validation is marked Nyquist-compliant only after targeted tenant-isolation tests and the full es-runtime suite pass."

patterns-established:
  - "Tenant-scoped cache key: cache APIs accept AggregateCacheKey so callers cannot use StreamId alone."
  - "Commit-gated cache mutation: append duplicate/error branches do not replace cached aggregate state."

requirements-completed: [STORE-04, RUNTIME-03, RUNTIME-05, RUNTIME-06, DOM-05]

duration: 6min 29s
completed: 2026-04-19
---

# Phase 09 Plan 01: Tenant-Scoped Runtime Aggregate Cache Summary

**Shard-local aggregate cache entries now require tenant plus stream identity, preventing same-stream tenants from sharing hot aggregate state before rehydration.**

## Performance

- **Duration:** 6min 29s
- **Started:** 2026-04-19T21:09:36Z
- **Completed:** 2026-04-19T21:16:05Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `AggregateCacheKey { tenant_id, stream_id }` and changed `AggregateCache` APIs to require it.
- Updated `ShardState::process_next_handoff` to construct one tenant-scoped cache key after duplicate replay misses and reuse it for cache hit, rehydration fill, and committed state replacement.
- Added cache unit coverage and runtime regressions proving tenants sharing `counter-1` rehydrate and decide independently.
- Marked Phase 09 validation green after targeted and full `es-runtime` test suites passed.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Add failing tenant cache key tests** - `27d2422` (test)
2. **Task 1 GREEN: Implement tenant aggregate cache key** - `87fa277` (feat)
3. **Task 2 RED: Add failing tenant runtime cache regressions** - `2dcd351` (test)
4. **Task 2 GREEN: Verify tenant-scoped shard cache flow** - `c972d00` (feat)
5. **Task 3: Mark tenant cache validation complete** - `d94dfa9` (docs)

**Plan metadata:** created by final docs commit.

## Files Created/Modified

- `crates/es-runtime/src/cache.rs` - Added `AggregateCacheKey` and changed aggregate cache storage/API from stream-only to tenant-scoped keys.
- `crates/es-runtime/src/lib.rs` - Re-exported `AggregateCacheKey`.
- `crates/es-runtime/src/shard.rs` - Uses one tenant-scoped cache key for lookup, rehydration fill, and committed cache replacement.
- `crates/es-runtime/tests/shard_disruptor.rs` - Added unit coverage for same-stream cross-tenant aggregate cache isolation.
- `crates/es-runtime/tests/runtime_flow.rs` - Added tenant-specific fake rehydration support and same-stream different-tenant runtime regressions.
- `.planning/phases/09-tenant-scoped-runtime-aggregate-cache/09-VALIDATION.md` - Marked Nyquist and Wave 0 evidence passed.

## Decisions Made

- Used an owned typed key instead of string concatenation or nested maps, matching the existing `DedupeKey` pattern.
- Kept duplicate replay before aggregate cache lookup; durable replay still returns before rehydration, decide, append, or cache mutation.
- Kept this phase local to runtime cache identity and tests; no dependencies, database schema, storage SQL, adapter guards, or production locks were added.

## Verification

- `cargo test -p es-runtime shard_cache -- --nocapture` - passed.
- `cargo test -p es-runtime same_stream_different_tenant_rehydrates_independently -- --nocapture` - passed.
- `cargo test -p es-runtime same_stream_different_tenant_preserves_domain_state -- --nocapture` - passed.
- `cargo test -p es-runtime runtime_duplicate -- --nocapture` - passed.
- `cargo test -p es-runtime conflict_does_not_mutate_cache -- --nocapture` - passed.
- `cargo test -p es-runtime -- --nocapture` - passed; 42 tests passed across lib, integration, and doc-test targets.
- Plan grep checks passed; no stream-only aggregate cache lookup or commit call sites remain.

Cargo still emits the pre-existing missing-docs warning for `tests/shard_disruptor.rs`; it does not fail verification.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated shard and existing runtime cache call sites during Task 1**
- **Found during:** Task 1 (Add tenant-scoped AggregateCacheKey and unit coverage)
- **Issue:** Removing stream-only `AggregateCache` APIs made existing `ShardState` and `runtime_flow` cache assertions fail to compile before Task 1 verification could run.
- **Fix:** Applied the planned `AggregateCacheKey` usage in `ShardState::process_next_handoff` and updated existing runtime cache assertions/helpers to use `cache_key_for`.
- **Files modified:** `crates/es-runtime/src/shard.rs`, `crates/es-runtime/tests/runtime_flow.rs`
- **Verification:** `cargo test -p es-runtime shard_cache -- --nocapture`
- **Committed in:** `87fa277`

---

**Total deviations:** 1 auto-fixed (1 Rule 3 blocking issue).
**Impact on plan:** The fix was required to compile after intentionally removing stream-only cache APIs. It matched Task 2's planned production behavior and did not add scope.

## Issues Encountered

Cargo emitted the existing missing-docs warning for the `tests/shard_disruptor.rs` integration test crate. No test failed because of it.

## Known Stubs

No new stubs were introduced. The scan found the pre-existing `ShardHandoffToken::placeholder` runtime placeholder used to initialize the disruptor path; it is functional scaffolding, not unimplemented behavior.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 09 is complete. Runtime aggregate cache identity is tenant-scoped and Phase 10 can build on the preserved duplicate replay ordering without cross-tenant cache bleed.

## Self-Check: PASSED

- Created/modified files exist on disk.
- Task commits exist in git history: `27d2422`, `87fa277`, `2dcd351`, `c972d00`, `d94dfa9`.
- Required plan verification commands passed.
- Stub scan found no new unimplemented stubs.

---
*Phase: 09-tenant-scoped-runtime-aggregate-cache*
*Completed: 2026-04-19*

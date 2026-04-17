---
phase: 03-local-command-runtime-and-disruptor-execution
plan: 02
subsystem: runtime
tags: [rust, tokio, routing, bounded-ingress, twox-hash]
requires:
  - phase: 03-local-command-runtime-and-disruptor-execution
    provides: Runtime contracts, typed errors, command envelopes, and runtime store facade from Plan 03-01
provides:
  - Stable fixed-seed tenant-aware partition routing
  - ShardId and PartitionRouter runtime facade exports
  - Bounded CommandGateway using Tokio mpsc try_send
  - RoutedCommand handoff shape for later shard execution
  - Golden route and overload/unavailable tests
affects: [runtime, adapter-http, adapter-grpc, shard-runtime, command-engine]
tech-stack:
  added: []
  patterns: [fixed-seed tenant-aware routing, bounded nonblocking ingress, typed overload mapping]
key-files:
  created:
    - crates/es-runtime/src/router.rs
    - crates/es-runtime/src/gateway.rs
    - crates/es-runtime/tests/router_gateway.rs
  modified:
    - crates/es-runtime/src/lib.rs
    - .planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md
key-decisions:
  - "Use twox_hash::XxHash64 with ROUTING_HASH_SEED and an explicit separator byte so shard routing is stable and tenant-aware."
  - "Expose CommandGateway::try_submit as a synchronous, nonblocking admission API that maps full ingress to RuntimeError::Overloaded."
  - "Return RoutedCommand values carrying the computed ShardId so later engine/shard work does not reroute accepted envelopes."
patterns-established:
  - "Router inputs are tenant ID, separator byte 0, and partition key before modulo shard count."
  - "Adapter-facing runtime admission uses bounded Tokio mpsc channels and never awaits send capacity."
requirements-completed: [RUNTIME-01, RUNTIME-02]
duration: 9 min
completed: 2026-04-17
---

# Phase 03 Plan 02: Router and Gateway Summary

**Fixed-seed tenant-aware routing with bounded nonblocking command ingress and explicit overload errors**

## Performance

- **Duration:** 9 min
- **Started:** 2026-04-17T04:04:06Z
- **Completed:** 2026-04-17T04:12:58Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `PartitionRouter`, `ShardId`, and `ROUTING_HASH_SEED` with deterministic tenant-aware hashing.
- Added `CommandGateway` and `RoutedCommand` with bounded Tokio ingress and `try_send`-based overload behavior.
- Added integration tests for zero shard count, golden tenant/key routes, invalid ingress capacity, full ingress, and closed ingress.
- Updated the Phase 03 validation map to point at the concrete router/gateway verification command and files.

## Task Commits

1. **Task 1 RED: Add failing partition router tests** - `72b346d` (`test`)
2. **Task 1 GREEN: Implement stable tenant-aware partition routing** - `d371bc5` (`feat`)
3. **Task 2 RED: Add failing command gateway tests** - `201970e` (`test`)
4. **Task 2 RED: Cover invalid command gateway capacity** - `08336ec` (`test`)
5. **Task 2 GREEN: Implement bounded command gateway ingress** - `795591f` (`feat`)

## Files Created/Modified

- `crates/es-runtime/src/router.rs` - Stable tenant-aware partition router using fixed-seed xxHash64.
- `crates/es-runtime/src/gateway.rs` - Bounded command gateway and routed command handoff.
- `crates/es-runtime/tests/router_gateway.rs` - Router golden tests and gateway overload/unavailable tests.
- `crates/es-runtime/src/lib.rs` - Public facade exports for router and gateway types.
- `.planning/phases/03-local-command-runtime-and-disruptor-execution/03-VALIDATION.md` - Validation map and Wave 0 artifact status updates.

## Decisions Made

- Golden route values are fixed at shard `5` for `(tenant-a, order-123, 8)` and shard `2` for `(tenant-b, order-123, 8)` under `ROUTING_HASH_SEED`.
- `CommandGateway::new` owns channel creation and returns the receiver with the gateway so ingress capacity validation is centralized.
- `try_submit` computes the shard before `try_send`, preserving the accepted command handoff shape needed by later shard execution.

## Deviations from Plan

None - plan executed exactly as written.

## TDD Gate Compliance

- RED gate for Task 1: `72b346d`
- GREEN gate for Task 1: `d371bc5`
- RED gate for Task 2: `201970e`, `08336ec`
- GREEN gate for Task 2: `795591f`

## Verification

- `cargo test -p es-runtime partition_router` - passed
- `cargo test -p es-runtime bounded_ingress` - passed
- `cargo test -p es-runtime closed_ingress_returns_unavailable` - passed
- `! rg '\.send\(\)\.await|send\(\s*.*\)\.await' crates/es-runtime/src/gateway.rs` - passed
- `cargo test -p es-runtime` - passed

## Known Stubs

None.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 03-03 to connect routed commands to shard-local execution and disruptor publication without changing the adapter-facing gateway contract.

## Threat Flags

None.

## Self-Check: PASSED

- Summary file exists: `.planning/phases/03-local-command-runtime-and-disruptor-execution/03-02-SUMMARY.md`.
- Required runtime files exist: `router.rs`, `gateway.rs`, and `tests/router_gateway.rs`.
- Task commits exist: `72b346d`, `d371bc5`, `201970e`, `08336ec`, `795591f`.

---
*Phase: 03-local-command-runtime-and-disruptor-execution*
*Completed: 2026-04-17*

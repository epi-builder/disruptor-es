---
phase: 01-workspace-and-typed-kernel-contracts
plan: 04
subsystem: example-domain
tags: [rust, proptest, cargo-tree, aggregate, dependency-boundaries]
requires:
  - phase: 01-02
    provides: Typed core and aggregate kernel contracts
  - phase: 01-03
    provides: Workspace boundary crate topology
provides:
  - Minimal typed commerce aggregate fixture
  - Replay determinism tests using the kernel
  - Dependency-boundary integration tests for core and kernel crates
  - Full Phase 01 workspace verification evidence
affects: [examples, validation, dependency-boundaries]
tech-stack:
  added: [example-commerce, proptest]
  patterns: [typed-example-aggregate, cargo-tree-boundary-tests, package-name-dependency-filtering]
key-files:
  created:
    - crates/example-commerce/Cargo.toml
    - crates/example-commerce/src/lib.rs
    - crates/example-commerce/tests/dependency_boundaries.rs
  modified:
    - Cargo.lock
key-decisions:
  - "Dependency-boundary tests compare Cargo package names instead of raw tree text to avoid false positives from workspace path names."
  - "Full verification is represented by an empty commit because the lockfile was already updated during prior task verification."
patterns-established:
  - "Example aggregates prove kernel contracts without implementing future business workflows."
  - "Boundary tests shell out to Cargo from the virtual workspace root."
requirements-completed: [CORE-01, CORE-02, CORE-03, CORE-04]
duration: 4min
completed: 2026-04-16
---

# Phase 01: Example Commerce Fixture Summary

**Typed ProductDraft aggregate with replay determinism tests and package-name dependency boundary checks**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-16T14:05:59Z
- **Completed:** 2026-04-16T14:09:36Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Created `example-commerce` with a compact `ProductDraft` aggregate using the `es-kernel::Aggregate` contract.
- Added tests for successful product creation, empty SKU/name validation, duplicate creation rejection, and replay determinism.
- Added integration tests that verify required workspace member directories and inspect `es-core`/`es-kernel` Cargo dependency trees.
- Ran the full Phase 01 verification gate across the workspace.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement minimal typed commerce aggregate fixture** - `585e7b5` (feat)
2. **Task 2: Add dependency-boundary integration tests** - `b1b14da` (test)
3. **Task 3: Run full Phase 01 verification** - `c530ba8` (test)

## Files Created/Modified

- `crates/example-commerce/Cargo.toml` - Defines the example domain crate and test dependencies.
- `crates/example-commerce/src/lib.rs` - Implements `ProductDraft`, typed commands/events/replies/errors, and aggregate tests.
- `crates/example-commerce/tests/dependency_boundaries.rs` - Checks Cargo dependency trees and workspace topology.
- `Cargo.lock` - Adds the example crate and test dependency graph.

## Decisions Made

- Compared package-name tokens in `cargo tree --prefix none` output rather than matching raw text, because the repository path contains `disruptor-es` and raw matching would falsely report the forbidden `disruptor` package.
- Used an empty verification commit for Task 3 because full verification changed no files after the lockfile was already committed with Task 1.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Avoid path-sensitive dependency false positives**
- **Found during:** Task 2 (dependency boundary tests)
- **Issue:** Raw `cargo tree` text contains the workspace path `/Users/epikem/dev/projects/disruptor-es`, so matching the substring `disruptor` reports a forbidden dependency even when no such package is present.
- **Fix:** Boundary tests parse each tree line's package-name token and compare that token to the forbidden dependency list.
- **Files modified:** `crates/example-commerce/tests/dependency_boundaries.rs`
- **Verification:** `cargo test -p example-commerce --test dependency_boundaries` passes.
- **Committed in:** `b1b14da`

**Total deviations:** 1 auto-fixed (1 blocking).
**Impact on plan:** The dependency boundary remains stricter and more accurate; no architecture scope changed.

## Issues Encountered

- The literal grep form of the final dependency check is path-sensitive in this workspace because the repo directory name includes `disruptor`. The committed integration test verifies package names instead and passed for both `es-core` and `es-kernel`.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p example-commerce aggregate_contract`
- `cargo test -p example-commerce --test dependency_boundaries`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo tree -p es-core --prefix none`
- `cargo tree -p es-kernel --prefix none`

## Next Phase Readiness

Phase 01 now has a buildable Rust workspace, typed core/kernel contracts, visible future service boundaries, an example aggregate, and automated tests that protect dependency direction.

## Self-Check: PASSED

---
*Phase: 01-workspace-and-typed-kernel-contracts*
*Completed: 2026-04-16*

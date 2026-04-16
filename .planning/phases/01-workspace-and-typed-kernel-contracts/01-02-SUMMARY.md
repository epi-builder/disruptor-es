---
phase: 01-workspace-and-typed-kernel-contracts
plan: 02
subsystem: domain-kernel
tags: [rust, event-sourcing, aggregate, metadata, serde]
requires:
  - phase: 01-01
    provides: Rust 2024 workspace policy and dependency catalog
provides:
  - Typed stream, partition, tenant, revision, and metadata contracts
  - Synchronous associated-type aggregate kernel trait
  - Typed decision result and replay helper
affects: [domain, runtime, storage, projection, adapters]
tech-stack:
  added: [es-core, es-kernel]
  patterns: [opaque-id-newtypes, typed-command-metadata, synchronous-aggregate-trait]
key-files:
  created:
    - crates/es-core/Cargo.toml
    - crates/es-core/src/lib.rs
    - crates/es-kernel/Cargo.toml
    - crates/es-kernel/src/lib.rs
    - Cargo.lock
    - .gitignore
  modified:
    - Cargo.toml
key-decisions:
  - "Pinned `time` to `=0.3.44` because `time 0.3.47` requires Rust 1.88 and conflicts with the Rust 1.85 project floor."
  - "Kept `serde_json` as an `es-core` dev-dependency only for metadata round-trip tests."
patterns-established:
  - "Core string identities are opaque newtypes with constructor validation."
  - "Aggregate behavior is synchronous and typed through associated types, with no runtime or storage dependencies."
requirements-completed: [CORE-02, CORE-03, CORE-04]
duration: 4min
completed: 2026-04-16
---

# Phase 01: Typed Kernel Contracts Summary

**Typed event-sourcing core metadata plus a synchronous aggregate trait with replayable state application**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-16T14:00:06Z
- **Completed:** 2026-04-16T14:04:20Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Created `es-core` with stream IDs, partition keys, tenant IDs, stream revisions, expected revisions, command metadata, event metadata, and typed constructor errors.
- Created `es-kernel` with the associated-type `Aggregate` trait, typed `Decision<E, R>`, and synchronous `replay` helper.
- Added unit tests for metadata invariants, serde round-tripping, typed decisions, and ordered replay.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement typed core IDs, revisions, and metadata** - `054bea1` (feat)
2. **Task 2: Implement synchronous aggregate kernel trait** - `1e92d41` (feat)

## Files Created/Modified

- `Cargo.toml` - Pins `time` to an MSRV-compatible exact version.
- `Cargo.lock` - Locks the initial Rust dependency graph.
- `.gitignore` - Ignores Cargo build output.
- `crates/es-core/Cargo.toml` - Defines the core contract crate dependencies.
- `crates/es-core/src/lib.rs` - Implements core identity, revision, metadata, and tests.
- `crates/es-kernel/Cargo.toml` - Defines the kernel crate with an `es-core` path dependency.
- `crates/es-kernel/src/lib.rs` - Implements aggregate, decision, replay contracts, and tests.

## Decisions Made

- Used `=0.3.44` for `time` because Cargo selected `0.3.47`, which requires Rust 1.88 and fails under the pinned Rust 1.85 toolchain.
- Added `.gitignore` for `/target/` after the first Cargo test run generated build output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Pin `time` to Rust 1.85-compatible version**
- **Found during:** Task 1 (typed core metadata tests)
- **Issue:** Cargo selected `time 0.3.47`, which requires Rust 1.88 and blocks verification under the pinned Rust 1.85 toolchain.
- **Fix:** Changed workspace `time` dependency to `=0.3.44`, the compatible version observed by Cargo for Rust 1.85.
- **Files modified:** `Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo test -p es-core metadata_contracts` and `cargo test -p es-kernel aggregate_kernel_contracts` pass.
- **Committed in:** `054bea1`

**2. [Rule 3 - Blocking] Ignore generated Cargo target directory**
- **Found during:** Task 1 verification
- **Issue:** Cargo generated `target/`, which would otherwise remain as untracked build output.
- **Fix:** Added `.gitignore` with `/target/`.
- **Files modified:** `.gitignore`
- **Verification:** `git status --short` no longer reports `target/`.
- **Committed in:** `054bea1`

**Total deviations:** 2 auto-fixed (2 blocking).
**Impact on plan:** Both fixes preserve the planned architecture and make the Rust 1.85 verification path executable.

## Issues Encountered

- `time 0.3.47` MSRV exceeded the project toolchain. Resolved by exact pinning to `0.3.44`.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p es-core metadata_contracts`
- `cargo test -p es-kernel aggregate_kernel_contracts`
- `cargo tree -p es-core`
- `cargo tree -p es-kernel`
- Acceptance greps for core and kernel public API and forbidden runtime/storage/adapter dependencies.

## Next Phase Readiness

Boundary and example crates can now depend on typed core/kernel contracts without introducing storage, runtime, network, or broker dependencies into deterministic domain code.

## Self-Check: PASSED

---
*Phase: 01-workspace-and-typed-kernel-contracts*
*Completed: 2026-04-16*

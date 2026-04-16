---
phase: 01-workspace-and-typed-kernel-contracts
plan: 01
subsystem: infra
tags: [rust, cargo, workspace, cargo-deny, validation]
requires: []
provides:
  - Rust 1.85 toolchain pin for the workspace
  - Rust 2024 virtual Cargo workspace policy
  - cargo-deny supply-chain policy baseline
  - Rust-specific Phase 01 validation strategy
affects: [workspace, dependency-policy, validation]
tech-stack:
  added: [rust-1.85, cargo-deny-config]
  patterns: [workspace-inherited-metadata, workspace-lint-policy, nyquist-validation-map]
key-files:
  created:
    - rust-toolchain.toml
    - Cargo.toml
    - deny.toml
  modified:
    - .planning/phases/01-workspace-and-typed-kernel-contracts/01-VALIDATION.md
key-decisions:
  - "Pinned Rust 1.85 as the workspace MSRV for Rust 2024 support."
  - "Kept runtime, storage, adapter, broker, and disruptor dependencies out of the root catalog for Phase 01."
patterns-established:
  - "Workspace crates inherit Rust 2024 metadata, MSRV, dependency versions, and lint policy from the root workspace."
  - "Supply-chain checks are configured in repo before cargo-deny becomes a required local tool."
requirements-completed: [CORE-01, CORE-04]
duration: 11min
completed: 2026-04-16
---

# Phase 01: Workspace Policy Summary

**Rust 2024 workspace policy with pinned 1.85 toolchain, cargo-deny baseline, and concrete Phase 01 validation commands**

## Performance

- **Duration:** 11 min
- **Started:** 2026-04-16T13:49:08Z
- **Completed:** 2026-04-16T14:00:06Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added a Rust 1.85 toolchain pin with rustfmt and clippy components.
- Created the root virtual Cargo workspace with resolver 3, Rust 2024 metadata, dependency catalog, and inherited lint policy.
- Added a cargo-deny policy baseline denying unknown registries and git sources.
- Finalized the validation runtime estimate while preserving the Rust-specific Nyquist validation map.

## Task Commits

Each task was committed atomically:

1. **Task 1: Pin Rust 2024 workspace toolchain** - `693c71d` (build)
2. **Task 2: Add dependency policy baseline** - `0f2ce32` (build)
3. **Task 3: Finalize Nyquist validation strategy** - `217a13f` (docs)

## Files Created/Modified

- `rust-toolchain.toml` - Pins Rust 1.85 with rustfmt and clippy.
- `Cargo.toml` - Defines the virtual Rust 2024 workspace, workspace package metadata, dependency versions, and lint policy.
- `deny.toml` - Provides the cargo-deny supply-chain policy baseline.
- `.planning/phases/01-workspace-and-typed-kernel-contracts/01-VALIDATION.md` - States the Rust validation runtime estimate.

## Decisions Made

- Used Rust 1.85.1 as installed by the `1.85` toolchain channel while keeping the project policy pinned to `channel = "1.85"`.
- Did not add Tokio, SQLx, Axum, Tonic, broker, PostgreSQL, or disruptor dependencies in this plan because later phases own those crate boundaries.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope change.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Verification

- `rustup toolchain install 1.85 --profile minimal --component rustfmt --component clippy`
- `rustc +1.85 --version`
- `cargo +1.85 --version`
- Root workspace policy grep checks for resolver, edition, MSRV, and unsafe lint.
- `deny.toml` grep checks for advisories, licenses, unknown registry denial, and multiple-version warning.
- Validation strategy grep checks for Nyquist compliance, quick run, full suite, and automated verification statement.

## Next Phase Readiness

Workspace-level policy is ready for typed core, kernel, boundary, and example crates to inherit package metadata, dependencies, and lint settings.

## Self-Check: PASSED

---
*Phase: 01-workspace-and-typed-kernel-contracts*
*Completed: 2026-04-16*

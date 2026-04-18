---
phase: 06-outbox-and-process-manager-workflows
fixed_at: 2026-04-18T08:55:11Z
review_path: .planning/phases/06-outbox-and-process-manager-workflows/06-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-04-18T08:55:11Z
**Source review:** .planning/phases/06-outbox-and-process-manager-workflows/06-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### CR-01: Claimed outbox rows are never reclaimed after dispatcher failure

**Files modified:** `crates/es-store-postgres/src/outbox.rs`, `crates/es-store-postgres/tests/outbox.rs`
**Commit:** 5ff1041
**Applied fix:** `claim_pending` now reclaims expired `publishing` rows by `locked_until`, and an integration test verifies a second worker can reclaim an expired claim.

### WR-01: Publish and retry transitions do not verify row ownership or status

**Files modified:** `crates/es-outbox/src/dispatcher.rs`, `crates/es-store-postgres/src/outbox.rs`, `crates/es-store-postgres/tests/outbox.rs`
**Commit:** f1d97a4
**Applied fix:** The dispatcher carries `worker_id` through publish and retry transitions, PostgreSQL updates require `status = 'publishing'` and matching `locked_by`, and tests cover pending and stale-worker transition attempts.

### WR-02: Multi-line order rejection can leave earlier reservations held

**Files modified:** `crates/app/src/commerce_process_manager.rs`
**Commit:** cc2953f
**Applied fix:** The commerce process manager tracks successful inventory reservations and submits deterministic `ReleaseInventory` compensation commands before rejecting an order after a later line fails. Status: fixed, requires human verification because this is workflow logic.

---

_Fixed: 2026-04-18T08:55:11Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

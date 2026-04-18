---
phase: 06-outbox-and-process-manager-workflows
reviewed: 2026-04-18T08:48:16Z
depth: standard
files_reviewed: 20
files_reviewed_list:
  - crates/app/Cargo.toml
  - crates/app/src/commerce_process_manager.rs
  - crates/app/src/lib.rs
  - crates/es-outbox/Cargo.toml
  - crates/es-outbox/src/dispatcher.rs
  - crates/es-outbox/src/error.rs
  - crates/es-outbox/src/lib.rs
  - crates/es-outbox/src/models.rs
  - crates/es-outbox/src/process_manager.rs
  - crates/es-outbox/src/publisher.rs
  - crates/es-outbox/tests/contracts.rs
  - crates/es-outbox/tests/process_manager.rs
  - crates/es-store-postgres/Cargo.toml
  - crates/es-store-postgres/migrations/20260418010000_outbox.sql
  - crates/es-store-postgres/src/error.rs
  - crates/es-store-postgres/src/lib.rs
  - crates/es-store-postgres/src/models.rs
  - crates/es-store-postgres/src/outbox.rs
  - crates/es-store-postgres/src/sql.rs
  - crates/es-store-postgres/tests/outbox.rs
findings:
  critical: 1
  warning: 2
  info: 0
  total: 3
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-04-18T08:48:16Z
**Depth:** standard
**Files Reviewed:** 20
**Status:** issues_found

## Summary

Reviewed the outbox contracts, PostgreSQL outbox storage, process-manager offset handling, commerce process-manager workflow, SQL migration, manifests, and tests. The existing focused test run passed, but the implementation has one durability gap in outbox claiming and two workflow/state-transition risks that are not covered by the current tests.

Verification run:

```bash
cargo test -p es-outbox -p es-store-postgres -p app
```

Result: passed.

## Critical Issues

### CR-01: Claimed outbox rows are never reclaimed after dispatcher failure

**File:** `crates/es-store-postgres/src/outbox.rs:94`

**Issue:** `claim_pending` only selects rows where `status = 'pending'` and `available_at <= now()`. It writes `status = 'publishing'` plus `locked_until`, but no query ever selects expired `publishing` rows again. If a dispatcher crashes, is cancelled, or loses process state after claiming but before `mark_published` or `schedule_retry`, that row remains `publishing` permanently and the integration event is never delivered. The schema has `locked_until`, but the runtime does not use it for recovery.

**Fix:**

```sql
WITH claimed AS (
    SELECT outbox_id
    FROM outbox_messages
    WHERE tenant_id = $1
      AND (
          (status = 'pending' AND available_at <= now())
          OR (status = 'publishing' AND locked_until <= now())
      )
    ORDER BY source_global_position, outbox_id
    LIMIT $2
    FOR UPDATE SKIP LOCKED
)
UPDATE outbox_messages AS o
SET status = 'publishing',
    locked_by = $3,
    locked_until = now() + ($4 * INTERVAL '1 second'),
    attempts = attempts + 1,
    updated_at = now()
FROM claimed
WHERE o.outbox_id = claimed.outbox_id
  AND o.tenant_id = $1
RETURNING o.*
```

Add an integration test that claims a row with a short lock, leaves it in `publishing`, advances past `locked_until`, and verifies another worker can reclaim it.

## Warnings

### WR-01: Publish and retry transitions do not verify row ownership or status

**File:** `crates/es-store-postgres/src/outbox.rs:134`

**Issue:** `mark_published` updates by `tenant_id` and `outbox_id` only; `schedule_retry` has the same issue at `crates/es-store-postgres/src/outbox.rs:166`. The dispatcher receives a `worker_id` when claiming, but the follow-up transitions do not require the row to still be `publishing` or still owned by that worker. Once expired-lock reclaim is added, a stale worker can mark another worker's claimed row as published or retry it after a newer attempt has taken over. Even before reclaim, direct callers can silently mark pending or failed rows as published.

**Fix:** Carry `worker_id` through the store transition API and constrain updates to the claimed row state. Return an error when no row is updated.

```sql
UPDATE outbox_messages
SET status = 'published',
    published_at = now(),
    locked_by = NULL,
    locked_until = NULL,
    last_error = NULL,
    updated_at = now()
WHERE tenant_id = $1
  AND outbox_id = $2
  AND status = 'publishing'
  AND locked_by = $3
RETURNING outbox_id
```

Apply the same ownership predicate to retry/fail transitions and add tests for stale worker attempts.

### WR-02: Multi-line order rejection can leave earlier reservations held

**File:** `crates/app/src/commerce_process_manager.rs:67`

**Issue:** The process manager reserves inventory line by line. If one reservation succeeds and a later reservation fails, the loop breaks at `crates/app/src/commerce_process_manager.rs:95` and rejects the order at `crates/app/src/commerce_process_manager.rs:117`, but it never releases the inventory already reserved for earlier lines. The product domain has `ReleaseInventory`, so a multi-line failure can leak reserved stock and make later orders incorrectly fail inventory checks. Existing tests cover one-line success and one-line failure, but not partial success followed by failure.

**Fix:** Track successfully reserved lines and release them before rejecting the order, using deterministic idempotency keys for each compensation command. Add a test with two order lines where the first reserve succeeds, the second fails, and the manager submits `ReleaseInventory` for the first product before `RejectOrder`.

```rust
let mut reserved = Vec::new();

// On each successful ReserveInventory reply:
reserved.push((product_id.clone(), line.quantity));

// Before RejectOrder:
for (product_id, quantity) in reserved {
    submit_product_command(ProductCommand::ReleaseInventory {
        product_id,
        quantity,
    })
    .await?;
}
```

---

_Reviewed: 2026-04-18T08:48:16Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

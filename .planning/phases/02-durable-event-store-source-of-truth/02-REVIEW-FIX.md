---
phase: 02-durable-event-store-source-of-truth
source_review: 02-REVIEW.md
status: fixed
fixed:
  critical: 0
  warning: 2
  info: 0
created: 2026-04-16T23:22:00Z
---

# Phase 02 Code Review Fix Summary

## Fixed Findings

### WR-01: Concurrent first appends can surface database errors

Added a transaction-scoped stream advisory lock before reading the stream revision in the append transaction. This serializes per-tenant/per-stream append decisions after idempotency ownership is established and before optimistic-concurrency validation.

Regression coverage added:

- `concurrent_no_stream_first_appends_return_one_conflict`
- `concurrent_any_first_appends_serialize_revisions`

### WR-02: Snapshots can be saved for nonexistent or future stream revisions

Changed snapshot saving to run in a transaction, lock/read the owning stream row, reject nonexistent streams, and reject snapshot revisions greater than the durable stream revision with `StoreError::SnapshotRevisionConflict`.

Regression coverage added:

- `save_snapshot_rejects_nonexistent_stream`
- `save_snapshot_rejects_future_revision`

## Verification

- `cargo test -p es-store-postgres --test append_occ --test snapshots -- --nocapture`
- `cargo test --workspace --all-targets`

Both commands passed.

---
phase: 02-durable-event-store-source-of-truth
reviewed: 2026-04-16T23:13:35Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - Cargo.toml
  - crates/es-store-postgres/Cargo.toml
  - crates/es-store-postgres/migrations/20260417000000_event_store.sql
  - crates/es-store-postgres/src/error.rs
  - crates/es-store-postgres/src/event_store.rs
  - crates/es-store-postgres/src/ids.rs
  - crates/es-store-postgres/src/lib.rs
  - crates/es-store-postgres/src/models.rs
  - crates/es-store-postgres/src/rehydrate.rs
  - crates/es-store-postgres/src/sql.rs
  - crates/es-store-postgres/tests/append_occ.rs
  - crates/es-store-postgres/tests/common/mod.rs
  - crates/es-store-postgres/tests/dedupe.rs
  - crates/es-store-postgres/tests/global_reads.rs
  - crates/es-store-postgres/tests/harness_smoke.rs
  - crates/es-store-postgres/tests/snapshots.rs
findings:
  critical: 0
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-04-16T23:13:35Z
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

Reviewed the PostgreSQL event-store crate, migration, and integration tests at standard depth. The append transaction correctly scopes most reads and writes by tenant, stores idempotency results in the same transaction as events, and the current integration suite passes with `cargo test -p es-store-postgres`.

Two transaction-correctness gaps remain: concurrent first appends to the same missing stream are not serialized before stream creation, and snapshots can be saved for revisions that are not durably present in the event stream.

## Warnings

### WR-01: Concurrent First Appends Can Surface Database Errors Instead Of Correct OCC Outcomes

**File:** `crates/es-store-postgres/src/sql.rs:20`

**Issue:** `select_stream_revision_for_update` only locks an existing `streams` row. When two different idempotency keys append concurrently to the same missing `(tenant_id, stream_id)`, both transactions can observe `current_revision = None` before either row exists. Both then compute `first_revision = 1`. The later transaction can reach `upsert_stream_revision` and `insert_event` with duplicate `(tenant_id, stream_id, stream_revision)` values, causing a raw SQL unique-constraint error instead of the expected event-store behavior: `ExpectedRevision::NoStream` should return `StoreError::StreamConflict`, and `ExpectedRevision::Any` should append after the winner's revision.

This is especially important because first-write races are normal for event-sourced streams under retry, fan-in, or horizontally scaled adapters.

**Fix:**
Serialize per-stream append decisions before reading the stream revision, or use an atomic conditional stream-row claim/update that maps conflicts to `StoreError::StreamConflict`. A direct fix using a tenant+stream advisory transaction lock keeps the existing flow small:

```rust
async fn acquire_stream_lock(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    stream_id: &StreamId,
) -> StoreResult<()> {
    sqlx::query(
        r#"
        SELECT pg_advisory_xact_lock(
            hashtextextended($1 || ':' || $2, 1)
        )
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(stream_id.as_str())
    .execute(&mut **tx)
    .await?;

    Ok(())
}
```

Call it after the dedupe lock and before `select_stream_revision_for_update`. Then add integration coverage for two concurrent non-duplicate appends to a missing stream under both `ExpectedRevision::NoStream` and `ExpectedRevision::Any`.

### WR-02: Snapshots Can Be Saved For Nonexistent Or Future Stream Revisions

**File:** `crates/es-store-postgres/src/sql.rs:451`

**Issue:** `save_snapshot` inserts or updates `snapshots` without verifying that the target stream exists or that `request.stream_revision` is less than or equal to the durable stream revision. A caller can save a snapshot for a nonexistent stream or for revision 100 when only revisions 1..3 exist. `load_rehydration` then trusts the latest snapshot revision and calls `read_stream_after` with that value, which can skip all real events and return an invalid aggregate state.

Because the event store is the source of truth, snapshot rows must not be allowed to advance beyond committed events.

**Fix:**
Wrap snapshot writes in a transaction, lock/read the owning stream row, and reject missing or future revisions before inserting the snapshot. Add a typed error such as `StoreError::SnapshotRevisionConflict`.

```rust
let current_revision = sqlx::query_scalar::<_, i64>(
    r#"
    SELECT revision
    FROM streams
    WHERE tenant_id = $1 AND stream_id = $2
    FOR UPDATE
    "#,
)
.bind(request.tenant_id.as_str())
.bind(request.stream_id.as_str())
.fetch_optional(&mut *tx)
.await?;

match current_revision {
    Some(current) if stream_revision <= current => {
        // insert/update snapshot in the same transaction
    }
    Some(current) => {
        return Err(StoreError::SnapshotRevisionConflict {
            stream_id: request.stream_id.as_str().to_owned(),
            requested: stream_revision,
            current,
        });
    }
    None => {
        return Err(StoreError::StreamConflict {
            stream_id: request.stream_id.as_str().to_owned(),
            expected: "existing stream".to_owned(),
            actual: None,
        });
    }
}
```

Add tests that reject snapshots for nonexistent streams and snapshots beyond the current stream revision, plus a rehydration regression test proving an invalid future snapshot cannot hide committed events.

---

_Reviewed: 2026-04-16T23:13:35Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

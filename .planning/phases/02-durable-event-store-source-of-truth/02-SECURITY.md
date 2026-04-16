---
phase: 02-durable-event-store-source-of-truth
phase_number: 02
status: secured
asvs_level: 1
threats_total: 20
threats_closed: 20
threats_open: 0
audited: 2026-04-17
block_on: high
---

# Phase 02 Security Verification

## Summary

Phase 02 threat mitigations are closed. All 20 registered threats from the 02-01 through 02-04 plan threat models were verified by direct evidence in the PostgreSQL migration, storage DTO validation, SQLx query helpers, public API signatures, and integration tests.

No accepted risks or transferred risks are recorded for this phase.

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-02-01 | Information Disclosure | mitigate | CLOSED | Tenant-owned storage tables include `tenant_id` in primary keys or tenant read-supporting indexes: migration lines 1-6, 23, 38, 48, and 51-58. |
| T-02-02 | Tampering | mitigate | CLOSED | PostgreSQL constraints reject invalid or duplicate data: revision checks, event ID uniqueness, event type/schema checks, stream revision uniqueness, and FK are in migration lines 4, 10-24, and 31-34. |
| T-02-03 | Tampering / Repudiation | mitigate | CLOSED | `command_dedup` has `PRIMARY KEY (tenant_id, idempotency_key)` and committed-result columns in migration lines 27-38; duplicate result lookup reads by tenant/idempotency in `sql.rs` lines 98-119. |
| T-02-04 | Spoofing | mitigate | CLOSED | Migration contains no DB UUID default; grep for `uuidv7()`/`uuid_generate` returned no matches. Events bind caller-provided `event.event_id` in `sql.rs` lines 241-258. |
| T-02-05 | Denial of Service | mitigate | CLOSED | Event payloads use JSONB columns in migration lines 20 and 45; storage SQL uses static SQLx query strings and bind parameters, with no `format!`, `push_str`, or interpolated SQL matches in `sql.rs`. |
| T-02-06 | Denial of Service | mitigate | CLOSED | `MAX_JSON_PAYLOAD_BYTES` is defined as 1 MiB and enforced before persistence in `models.rs` lines 8-9 and 43-50. |
| T-02-07 | Spoofing | mitigate | CLOSED | `AppendRequest` carries `CommandMetadata` and no separate tenant string; `command_metadata.tenant_id` is the append tenant source in `models.rs` lines 63-75 and 80-103. |
| T-02-08 | Tampering | mitigate | CLOSED | Empty append requests return `StoreError::EmptyAppend` in `models.rs` lines 87-89 and are also guarded in `event_store.rs` lines 23-29. |
| T-02-09 | SQL Injection | mitigate | CLOSED | Storage queries use SQLx bind parameters for append, dedupe, events, snapshots, and reads; representative bind evidence is in `sql.rs` lines 64-73, 102-111, 238-269, 296-329, 388-415, 433-459, and 514-538. Grep found no dynamic SQL construction in `sql.rs`. |
| T-02-10 | Information Disclosure | mitigate | CLOSED | Public read methods require `TenantId` for stream, global, latest snapshot, and rehydration reads in `event_store.rs` lines 32-55 and 62-78. |
| T-02-11 | Tampering | mitigate | CLOSED | Append SQL uses SQLx bind parameters and no dynamic SQL; event insert binds every caller value in `sql.rs` lines 238-269. Grep found no `format!`, `push_str`, or interpolated SQL in `sql.rs`. |
| T-02-12 | Information Disclosure | mitigate | CLOSED | Append/dedupe predicates bind tenant IDs in `sql.rs` lines 102-111, 126-135, 150-159, and 206-217; integration test `idempotency_key_is_scoped_by_tenant` is in `dedupe.rs` lines 139-166. |
| T-02-13 | Tampering / Repudiation | mitigate | CLOSED | Duplicate command replay checks `command_dedup` before stream/event writes and returns `AppendOutcome::Duplicate` in `sql.rs` lines 13-18; committed dedupe results are inserted after events in lines 278-332. Dedupe replay/no-extra-events tests are in `dedupe.rs` lines 79-137. |
| T-02-14 | Spoofing | mitigate | CLOSED | Event insert persists `command_id`, `correlation_id`, `causation_id`, and `tenant_id` from `CommandMetadata` in `sql.rs` lines 241-268; `metadata_columns_are_persisted` verifies the columns in `append_occ.rs` lines 187-251. |
| T-02-15 | Denial of Service | mitigate | CLOSED | Empty appends are rejected before SQL in `event_store.rs` lines 23-29; OCC mismatch maps to `StoreError::StreamConflict` before insert in `sql.rs` lines 22-29 and 165-190; rollback behavior is tested in `append_occ.rs` lines 254-301. |
| T-02-16 | Information Disclosure | mitigate | CLOSED | Stream/global read predicates include `tenant_id` in `sql.rs` lines 388-415 and 433-459; tenant-scoped global read behavior is tested in `global_reads.rs` lines 169-191. |
| T-02-17 | Tampering | mitigate | CLOSED | Snapshot writes are keyed by `(tenant_id, stream_id, stream_revision)`, use SQLx binds, and upsert JSONB snapshot columns in `sql.rs` lines 514-538; the table primary key is in migration lines 41-48. |
| T-02-18 | Denial of Service | mitigate | CLOSED | Read methods accept explicit `limit: i64` in `event_store.rs` lines 33-54; negative limits return `InvalidReadLimit` and SQL uses bound `LIMIT` parameters in `sql.rs` lines 374-407, 420-452, and 464-469. |
| T-02-19 | Repudiation | mitigate | CLOSED | Global reads use durable `events.global_position` ordered ascending, not timestamps or disruptor sequences, in `sql.rs` lines 433-459; verification confirms disruptor independence in `02-VERIFICATION.md` lines 21-25. |
| T-02-20 | Spoofing | mitigate | CLOSED | Snapshot and event rows preserve tenant-scoped stream identity: DTO fields in `models.rs` lines 130-190, snapshot read predicate in `sql.rs` lines 551-562, and cross-tenant snapshot tests in `snapshots.rs` lines 75-139. |

## Unregistered Flags

None.

`02-02-SUMMARY.md` and `02-03-SUMMARY.md` include `## Threat Flags` sections and both record none. `02-01-SUMMARY.md` and `02-04-SUMMARY.md` do not record additional threat flags. No implementation evidence introduced an unregistered flag during this verification.

## Accepted Risks

None.

## Transferred Risks

None.

## Audit Trail

| Date | Auditor | ASVS Level | Closed | Open | Notes |
|------|---------|------------|--------|------|-------|
| 2026-04-17 | Codex security auditor | 1 | 20 | 0 | Verified all Phase 02 plan threat-model mitigations against listed implementation and test files. Phase verification is passed and review status is clean. |

## Supporting Verification

- `02-VERIFICATION.md` reports `status: passed` and 5/5 must-haves verified.
- `02-REVIEW.md` reports `status: clean` with 0 findings after review fixes.
- `02-REVIEW-FIX.md` reports review fix verification commands passed.

---
phase: 06-outbox-and-process-manager-workflows
verified: 2026-04-18T09:05:44Z
status: passed
score: 22/22 must-haves verified
overrides_applied: 0
---

# Phase 6: Outbox and Process Manager Workflows Verification Report

**Phase Goal:** Integration events and cross-entity workflows are driven from committed events through durable outbox rows and process managers, keeping broker publication and workflow follow-ups off the hot command path.
**Verified:** 2026-04-18T09:05:44Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Event append transactions can create outbox rows derived from committed domain events in the same durable commit. | VERIFIED | `AppendRequest::new_with_outbox` carries derived messages and validates source event IDs; `sql::append` inserts events, then outbox rows, then dedupe result inside one transaction. |
| 2 | A dispatcher can publish pending outbox rows through a publisher trait and mark successful rows as published. | VERIFIED | `dispatch_once` calls `publisher.publish(message.publish_envelope()).await`, then calls `mark_published` only on `Ok(())`; PostgreSQL integration test asserts published status and `published_at`. |
| 3 | Dispatcher retries are idempotent by source event and topic, so repeated attempts do not create duplicate external effects. | VERIFIED | Outbox table has `UNIQUE (tenant_id, source_event_id, topic)`; `OutboxMessage::idempotency_key()` is `tenant:topic:source_event_id`; `InMemoryPublisher` records only the first key. |
| 4 | A process manager reacts to order/product workflow events and issues follow-up commands through the same command gateway without distributed transactions. | VERIFIED | `CommerceOrderProcessManager` handles committed `OrderPlaced`, submits `ReserveInventory`, then `ConfirmOrder` or `RejectOrder` through `CommandGateway`; offsets advance only after replies. |
| 5 | Outbox messages have validated tenant, topic, source event, message key, payload, status, worker, batch, retry, and idempotency fields before storage or publisher use. | VERIFIED | Validated newtypes and constructors in `es-outbox::models`; row mapping reconstructs typed values before returning `OutboxMessage`. |
| 6 | Publisher calls receive deterministic idempotency keys built from tenant ID, topic, and source event ID. | VERIFIED | `OutboxMessage::publish_envelope()` uses `idempotency_key()` from tenant/topic/source event. |
| 7 | The `es-outbox` crate exposes storage-neutral contracts without SQLx, broker, adapter, or disruptor dependencies. | VERIFIED | `rg` found no `sqlx`, broker, adapter, `es-runtime`, `example-commerce`, or disruptor dependency in `crates/es-outbox`. |
| 8 | PostgreSQL schema stores pending, publishing, published, and failed outbox rows with tenant scoping and source-event/topic idempotency. | VERIFIED | Migration defines status check, tenant ID, source fields, and tenant/source/topic unique constraint. |
| 9 | Outbox repository claims due pending rows in bounded batches using row locks and exposes mark-published, retry, failure, and process-manager offset operations. | VERIFIED | `claim_pending` uses tenant filter, due status checks, ordering, `LIMIT`, and `FOR UPDATE SKIP LOCKED`; repository exposes publish/retry/failure/offset APIs. |
| 10 | Schema and repository tests prove tenant isolation and source-event/topic uniqueness before append integration exists. | VERIFIED | `outbox_is_idempotent_by_source_event_and_topic` and `outbox_repository_filters_by_tenant` are present in container-backed tests. |
| 11 | Append requests can carry outbox messages that reference appended event IDs. | VERIFIED | `AppendRequest` has `outbox_messages`, `new_with_outbox`, and rejects unknown source event IDs. |
| 12 | Outbox rows are inserted after their source events and before command dedupe and transaction commit. | VERIFIED | `sql::append` inserts events, stores returned global positions, inserts outbox messages, then writes command dedupe before `tx.commit()`. |
| 13 | Conflicts, rollback, and duplicate idempotency replay do not create duplicate outbox rows. | VERIFIED | Duplicate replay exits through command dedupe before writes; conflict test asserts zero outbox rows after rollback; duplicate replay test asserts one row. |
| 14 | A dispatcher can claim pending rows, publish through `Publisher`, and mark successful rows as published only after publish returns `Ok(())`. | VERIFIED | Dispatcher ordering is explicit in `dispatch_once`; integration test validates durable row status after success. |
| 15 | Failed publish attempts are retried with deterministic idempotency keys, and dispatcher outcomes report rows that become failed after max attempts. | VERIFIED | Dispatcher passes the same envelope key, calls `schedule_retry`, and counts `RetryScheduled` versus `Failed`; integration tests cover retry and max-attempt failure. |
| 16 | Dispatcher code remains storage-neutral and depends on an `OutboxStore` port instead of SQLx. | VERIFIED | `crates/es-outbox/src/dispatcher.rs` defines and uses `OutboxStore`; PostgreSQL implements the port in `es-store-postgres`. |
| 17 | A process manager reads committed events by global position and advances durable offsets only after follow-up command replies finish or the event is intentionally skipped. | VERIFIED | `process_batch` awaits `manager.process` before `advance_process_manager_offset`; app tests prove reply waiting. |
| 18 | Process-manager batches are loaded from `PostgresEventStore::read_global` by tenant and saved offset, not only from caller-supplied in-memory vectors. | VERIFIED | `process_committed_batch` loads offset and calls `reader.read_global`; `PostgresEventStore` implements `CommittedEventReader` by delegating to `read_global`. |
| 19 | `StoredEvent` rows are mapped into `ProcessEvent` with tenant, command, correlation, causation, payload, and metadata preserved. | VERIFIED | `impl From<StoredEvent> for ProcessEvent` copies all required fields; integration test asserts preservation. |
| 20 | The concrete commerce workflow lives in the app composition crate without creating a workspace dependency cycle. | VERIFIED | `crates/app/src/commerce_process_manager.rs` depends on `es-outbox`, `es-runtime`, and `example-commerce`; `es-outbox` has no reverse dependency. |
| 21 | `OrderPlaced` workflow submits `ReserveInventory`, then `ConfirmOrder` or `RejectOrder` through command gateways based on reserve reply. | VERIFIED | Workflow code and app tests cover reserve-confirm and reserve-reject paths. |
| 22 | Follow-up command idempotency keys are deterministic from process manager name, source event ID, target aggregate, and action. | VERIFIED | Code formats `pm:{name}:{event_id}:{action}:{target_id}` for reserve, release, confirm, and reject; tests assert reserve/confirm and compensation keys. |

**Score:** 22/22 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/es-outbox/src/error.rs` | Typed outbox errors | VERIFIED | `gsd-tools verify artifacts` passed. |
| `crates/es-outbox/src/models.rs` | Validated DTOs and dispatch value objects | VERIFIED | Substantive typed constructors, idempotency key, and publish envelope. |
| `crates/es-outbox/src/publisher.rs` | Publisher trait and idempotent test publisher | VERIFIED | Trait and in-memory idempotency store present. |
| `crates/es-store-postgres/migrations/20260418010000_outbox.sql` | Outbox and process-manager offset tables | VERIFIED | Tables, constraints, unique key, and indexes present. |
| `crates/es-store-postgres/src/outbox.rs` | PostgreSQL outbox/process-manager repository | VERIFIED | Claim, publish, retry, failure, offsets, and trait adapters present. |
| `crates/es-store-postgres/src/models.rs` | Append request outbox support | VERIFIED | `outbox_messages` and `new_with_outbox` source validation present. |
| `crates/es-store-postgres/src/sql.rs` | Append transaction outbox inserts | VERIFIED | Inserts source events before outbox rows and before dedupe result. |
| `crates/es-outbox/src/dispatcher.rs` | Storage-neutral dispatcher | VERIFIED | `OutboxStore` port and `dispatch_once` implementation present. |
| `crates/es-outbox/src/process_manager.rs` | Storage-neutral process-manager contracts | VERIFIED | `ProcessManager`, reader/offset ports, and batch helpers present. |
| `crates/app/src/commerce_process_manager.rs` | Concrete commerce process-manager workflow | VERIFIED | Gateway-driven order/product workflow and tests present. |
| `crates/app/src/lib.rs` | App workflow module export | VERIFIED | `pub mod commerce_process_manager;` present. |
| `crates/es-store-postgres/tests/outbox.rs` | PostgreSQL integration coverage | VERIFIED | Append, dispatcher, storage, and process-manager tests present. |
| `crates/es-outbox/tests/contracts.rs` | Outbox contract coverage | VERIFIED | Validates newtypes, idempotency, and publisher behavior. |
| `crates/es-outbox/tests/process_manager.rs` | Process-manager contract coverage | VERIFIED | Validates offset filtering and committed-reader use. |

`gsd-tools verify artifacts` passed for all 17 plan-declared artifacts across the five plan files.

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `models.rs` | `publisher.rs` | `PublishEnvelope` | VERIFIED | Automated regex missed the actual location, but `PublishEnvelope` is re-exported and returned by `OutboxMessage::publish_envelope`. |
| `models.rs` | publisher idempotency | tenant/topic/source event key | VERIFIED | `format!("{}:{}:{}", tenant, topic, source_event_id)` is implemented. |
| `outbox.rs` | `outbox_messages` | SQLx bind queries | VERIFIED | Claim, insert, publish, retry, and failure updates use bound values. |
| `outbox.rs` | `process_manager_offsets` | monotonic offset upsert | VERIFIED | `GREATEST(process_manager_offsets.last_global_position, EXCLUDED.last_global_position)` present. |
| `sql.rs` | `outbox_messages` | inserted source event/global position | VERIFIED | Automated regex was invalid, but source code clearly calls `insert_outbox_message` after inserted events are known. |
| `sql.rs` | `command_dedup` | outbox insert before dedupe commit | VERIFIED | Outbox insertion loop precedes `insert_dedupe_result`. |
| `dispatcher.rs` | `Publisher` | `publisher.publish` | VERIFIED | Automated regex missed formatting; line-level inspection shows `publisher.publish(message.publish_envelope()).await`. |
| `dispatcher.rs` | `OutboxStore` | mark published after publish | VERIFIED | `mark_published` is called only in the `Ok(())` branch. |
| `commerce_process_manager.rs` | `CommandGateway` | `try_submit` | VERIFIED | Product and order follow-up commands are submitted through gateways. |
| `process_manager.rs` | process-manager offsets | advance after process returns | VERIFIED | `advance_process_manager_offset` follows awaited process/skipped outcome. |
| `process_manager.rs` | `PostgresEventStore::read_global` | committed event reader | VERIFIED | `process_committed_batch` uses `CommittedEventReader::read_global`; PostgreSQL adapter delegates to event store. |
| `outbox.rs` | `ProcessManagerOffsetStore` | durable offset implementation | VERIFIED | `PostgresOutboxStore` implements the offset port. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `crates/es-store-postgres/src/sql.rs` | `request.outbox_messages` | `AppendRequest::new_with_outbox` | Yes - inserted source events produce committed global positions in the same transaction. | VERIFIED |
| `crates/es-outbox/src/dispatcher.rs` | `claimed` | `OutboxStore::claim_pending` | Yes - PostgreSQL implementation reads due durable rows with locks. | VERIFIED |
| `crates/es-store-postgres/src/outbox.rs` | claimed rows | `outbox_messages` table | Yes - tenant/status/availability query returns persisted rows. | VERIFIED |
| `crates/es-outbox/src/process_manager.rs` | `events` | `CommittedEventReader::read_global` | Yes - PostgreSQL implementation reads committed event-store rows by tenant/global position. | VERIFIED |
| `crates/app/src/commerce_process_manager.rs` | follow-up commands | decoded `ProcessEvent.payload` | Yes - payload becomes product/order commands through gateways and replies are awaited. | VERIFIED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Outbox crate contracts and dispatcher/process-manager tests | `cargo test -p es-outbox` | Orchestrator validation passed. | PASS |
| PostgreSQL outbox integration tests | `cargo test -p es-store-postgres --test outbox -- --test-threads=1 --nocapture` | Orchestrator validation passed. | PASS |
| Whole workspace integration | `cargo test --workspace` | Orchestrator validation passed. | PASS |
| Schema drift | schema drift check | `drift_detected=false` per orchestrator validation. | PASS |
| Code review re-review | review gate | `status: clean` in `06-REVIEW.md`. | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| INT-01 | 06-01, 06-02, 06-03 | Append transaction can create outbox rows derived from committed domain events. | SATISFIED | `AppendRequest::new_with_outbox`; append transaction inserts outbox rows after source events and before dedupe/commit; tests cover commit, duplicate replay, and rollback. |
| INT-02 | 06-01, 06-02, 06-04 | Outbox dispatcher publishes pending rows through a publisher trait and marks successful rows as published. | SATISFIED | `Publisher` trait, `dispatch_once`, `OutboxStore`, and PostgreSQL published-row transition are wired and tested. |
| INT-03 | 06-01, 06-02, 06-03, 06-04 | Outbox dispatch is retryable and idempotent by source event and topic. | SATISFIED | Tenant/source/topic unique constraint, deterministic publisher idempotency key, retry/fail status transitions, and duplicate-publish guard are present and tested. |
| INT-04 | 06-05 | A process-manager example reacts to order/product workflow events and issues follow-up commands through the same command gateway. | SATISFIED | `process_committed_batch` consumes committed events; `CommerceOrderProcessManager` submits product/order follow-up commands through `CommandGateway` and waits for replies before offset advancement. |

No orphaned Phase 6 requirements were found in `.planning/REQUIREMENTS.md`; INT-01 through INT-04 are all claimed by plan frontmatter and mapped to Phase 6.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/es-store-postgres/src/sql.rs` | 546 | `Some(current) if stream_revision <= current => {}` | INFO | Empty match arm is intentional snapshot conflict validation and unrelated to Phase 6 outbox behavior. No blocker. |

Stub scan found no TODO/FIXME/placeholder, hardcoded empty data, or console/log-only implementation patterns in Phase 6 files.

### Human Verification Required

None. The phase is backend contract/storage/workflow code with runnable automated coverage; no visual, real external broker, or manual UI flow is part of this phase.

### Gaps Summary

No blocking gaps found. The implementation satisfies the roadmap success criteria and all plan must-haves. Three automated key-link checks initially reported failures due to brittle regex patterns or an invalid regex, but manual line-level inspection verified the links are present and wired.

Disconfirmation pass:
- Partial requirement check: the broad INT-04 wording is satisfied by the planned committed `OrderPlaced` workflow that crosses into product and order command gateways; no separate product-event-triggered manager was specified by the phase plans.
- Misleading test check: dispatcher tests verify durable row status after `dispatch_once`, not only in-memory outcomes.
- Error path check: publish failures, max-attempt failure, stale worker ownership, expired publish lock reclamation, append conflict rollback, and malformed process-manager payload decode paths are represented in code/tests.

---

_Verified: 2026-04-18T09:05:44Z_
_Verifier: Claude (gsd-verifier)_

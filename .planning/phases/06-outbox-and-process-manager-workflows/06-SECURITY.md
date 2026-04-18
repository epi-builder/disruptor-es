---
phase: 06
slug: outbox-and-process-manager-workflows
status: verified
threats_open: 0
asvs_level: 1
created: 2026-04-18
audited: 2026-04-18
---

# Phase 06 - Security

Per-phase security verification for outbox and process-manager workflows. This audit verifies only the threats declared in the five Phase 06 `<threat_model>` blocks and the executor `## Threat Flags` sections.

## Trust Boundaries

| Boundary | Description | Primary Data Crossing |
|----------|-------------|-----------------------|
| append request -> outbox contracts | Integration message fields enter typed construction before storage. | Tenant IDs, source event IDs, topic, message key, payload, metadata |
| outbox contracts -> PostgreSQL | Validated DTOs become durable tenant-owned rows. | Outbox row fields, source global position, status, retry state |
| command dedupe -> outbox insert | Duplicate command replay must not create new external effects. | Dedupe keys, committed event IDs, outbox inserts |
| PostgreSQL outbox -> dispatcher | Durable pending rows become publish attempts. | Claimed outbox rows, worker locks, retry counters |
| dispatcher -> publisher | Durable rows become external effects through a publisher trait. | Publish envelopes and idempotency keys |
| committed events -> workflow commands | Durable event payloads become follow-up command envelopes. | ProcessEvent payload/metadata, command metadata, idempotency keys |
| process manager -> command gateway | Workflow code crosses into runtime admission and waits for replies. | Product and order command envelopes, reply channels |
| process manager -> offsets | Durable offset advancement records workflow completion. | Tenant-scoped process-manager offsets |

## Threat Register

| Plan Threat | Category | Component | Disposition | Status | Evidence |
|-------------|----------|-----------|-------------|--------|----------|
| 06-01/T-06-01 | Information Disclosure | `OutboxMessage.tenant_id` | mitigate | closed | `OutboxMessage` stores `tenant_id: TenantId` in `crates/es-outbox/src/models.rs:279`; PostgreSQL row mapping reconstructs it through `TenantId::new` in `crates/es-store-postgres/src/outbox.rs:459`. |
| 06-01/T-06-02 | Tampering | `OutboxMessage::idempotency_key` | mitigate | closed | Idempotency key is built by `OutboxMessage::idempotency_key` in `crates/es-outbox/src/models.rs:312`; contract tests assert the key and duplicate publish behavior in `crates/es-outbox/tests/contracts.rs:86` and `crates/es-outbox/tests/contracts.rs:97`. |
| 06-01/T-06-03 | Denial of Service | `RetryPolicy` and dispatcher | mitigate | closed | `RetryPolicy::new` rejects `max_attempts < 1` in `crates/es-outbox/src/models.rs:173`; dispatcher/storage failed-at-bound behavior is present in `crates/es-store-postgres/src/outbox.rs:178` and `crates/es-store-postgres/src/outbox.rs:211`. |
| 06-01/T-06-04 | Tampering | SQL construction | accept | closed | Accepted risk AR-06-01 documents that Plan 06-01 introduced no SQL and delegated SQL mitigation to later PostgreSQL plans. |
| 06-01/T-06-05 | Tampering | Process-manager replay keys | mitigate | closed | `ProcessManagerName` validates non-empty names in `crates/es-outbox/src/models.rs:73`; deterministic workflow key formats are implemented in `crates/app/src/commerce_process_manager.rs:79`, `crates/app/src/commerce_process_manager.rs:142`, and `crates/app/src/commerce_process_manager.rs:155`. |
| 06-02/T-06-01 | Information Disclosure | `outbox_messages` queries | mitigate | closed | Migration includes tenant-scoped columns/unique/indexes in `crates/es-store-postgres/migrations/20260418010000_outbox.sql:3`, `:19`, and `:24`; repository predicates filter by tenant in `crates/es-store-postgres/src/outbox.rs:94`; tenant isolation test starts at `crates/es-store-postgres/tests/outbox.rs:574`. |
| 06-02/T-06-02 | Tampering | Duplicate external effects | mitigate | closed | `UNIQUE (tenant_id, source_event_id, topic)` exists in `crates/es-store-postgres/migrations/20260418010000_outbox.sql:19`; duplicate insert test starts at `crates/es-store-postgres/tests/outbox.rs:301`. |
| 06-02/T-06-03 | Denial of Service | Poison retry loop | mitigate | closed | Retry state columns exist in `crates/es-store-postgres/migrations/20260418010000_outbox.sql:11`, `:12`, and `:16`; `schedule_retry` transitions to `failed` at max attempts in `crates/es-store-postgres/src/outbox.rs:178`; bounded retry test starts at `crates/es-store-postgres/tests/outbox.rs:436`. |
| 06-02/T-06-04 | Tampering | SQL injection | mitigate | closed | Repository SQL uses static SQLx queries and `.bind(...)`, for example `crates/es-store-postgres/src/outbox.rs:67` through `:73`, `:115` through `:118`, and `:193` through `:197`. Dynamic strings found in this file are error messages, not SQL construction. |
| 06-02/T-06-05 | Tampering | Workflow replay duplicate commands | mitigate | closed | Tenant-scoped `process_manager_offsets` table is defined in `crates/es-store-postgres/migrations/20260418010000_outbox.sql:26`; monotonic update uses `GREATEST` in `crates/es-store-postgres/src/outbox.rs:297`; monotonic offset test starts at `crates/es-store-postgres/tests/outbox.rs:639`. |
| 06-03/T-06-01 | Information Disclosure | Append outbox rows | mitigate | closed | Append-time outbox insert binds `request.command_metadata.tenant_id` and does not accept a separate outbox tenant in `crates/es-store-postgres/src/sql.rs:317`. |
| 06-03/T-06-02 | Tampering | Duplicate external effects | mitigate | closed | `AppendRequest::new_with_outbox` validates source event IDs in `crates/es-store-postgres/src/models.rs:103` and `:124`; append insert uses `ON CONFLICT (tenant_id, source_event_id, topic) DO NOTHING` in `crates/es-store-postgres/src/sql.rs:313`. |
| 06-03/T-06-03 | Denial of Service | Poison retry loop | accept | closed | Accepted risk AR-06-02 documents that append does not execute retry loops and Plan 06-04 owns bounded dispatcher retry processing. |
| 06-03/T-06-04 | Tampering | SQL injection | mitigate | closed | Append SQL uses static SQLx queries and binds values, including tenant binds in `crates/es-store-postgres/src/sql.rs:84`, `:103`, `:122`, `:146`, `:170`, `:227`, `:272`, and `:366`; no publisher call appears in `crates/es-store-postgres/src/sql.rs`. |
| 06-03/T-06-05 | Tampering | Workflow replay duplicate commands | mitigate | closed | Append transaction inserts outbox messages before dedupe result in `crates/es-store-postgres/src/sql.rs:46` through `:62`; duplicate replay and conflict rollback tests start at `crates/es-store-postgres/tests/outbox.rs:225` and `:262`. |
| 06-04/T-06-01 | Information Disclosure | Dispatcher tenant scope | mitigate | closed | `dispatch_once` accepts one `TenantId` and passes it to store calls in `crates/es-outbox/src/dispatcher.rs:42`, `:53`, `:69`, and `:75`; PostgreSQL trait adapter preserves tenant filtering in `crates/es-store-postgres/src/outbox.rs:314`. |
| 06-04/T-06-02 | Tampering | Duplicate external effects | mitigate | closed | Dispatcher publishes `message.publish_envelope()` in `crates/es-outbox/src/dispatcher.rs:66`; unit test verifies idempotency preservation in `crates/es-outbox/src/dispatcher.rs:357`; in-memory publisher dedupes by key in `crates/es-outbox/src/publisher.rs:76`. |
| 06-04/T-06-03 | Denial of Service | Poison retry loop | mitigate | closed | Dispatcher passes `RetryPolicy` to `schedule_retry` in `crates/es-outbox/src/dispatcher.rs:75`; PostgreSQL returns `RetryScheduled` or `Failed` in `crates/es-store-postgres/src/outbox.rs:211` and `:212`; max-attempt integration test starts at `crates/es-store-postgres/tests/outbox.rs:838`. |
| 06-04/T-06-04 | Tampering | SQL injection | mitigate | closed | Dispatcher is storage-neutral and contains no SQL; PostgreSQL implementation uses bound SQLx parameters as evidenced in `crates/es-store-postgres/src/outbox.rs:115` through `:118` and `:193` through `:197`. |
| 06-04/T-06-05 | Tampering | Workflow replay duplicate commands | accept | closed | Accepted risk AR-06-03 documents that dispatcher does not issue workflow commands; Plan 06-05 owns process-manager command idempotency. |
| 06-05/T-06-01 | Information Disclosure | Follow-up command metadata | mitigate | closed | Follow-up metadata copies `tenant_id` from `ProcessEvent` in `crates/app/src/commerce_process_manager.rs:193` and `:198`; process events preserve tenant mapping from stored events in `crates/es-store-postgres/src/outbox.rs:418` through `:427`. |
| 06-05/T-06-02 | Tampering | Duplicate external effects | accept | closed | Accepted risk AR-06-04 documents that Plan 06-05 issues commands, not publisher effects; publisher idempotency is verified by Plans 06-01 and 06-04. |
| 06-05/T-06-03 | Denial of Service | Poison workflow event | mitigate | closed | Decode failures return `OutboxError::PayloadDecode` in `crates/app/src/commerce_process_manager.rs:187`; command submit failures map to `CommandSubmit` in `crates/app/src/commerce_process_manager.rs:204`; `process_batch` advances offsets only after successful manager processing in `crates/es-outbox/src/process_manager.rs:118` through `:125`. |
| 06-05/T-06-04 | Tampering | SQL injection | accept | closed | Accepted risk AR-06-05 documents that Plan 06-05 has no SQL and uses `ProcessManagerOffsetStore`; PostgreSQL offset persistence was already mitigated in Plan 06-02. |
| 06-05/T-06-05 | Tampering | Workflow replay duplicate commands | mitigate | closed | Deterministic command idempotency keys are formatted in `crates/app/src/commerce_process_manager.rs:79`, `:116`, `:142`, and `:155`; offset advancement follows processing in `crates/es-outbox/src/process_manager.rs:118` through `:125`; tests verify deterministic keys and reply waiting in `crates/app/src/commerce_process_manager.rs:596` and `:665`. |

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-06-01 | 06-01/T-06-04 | Plan 06-01 is storage-neutral contract code and adds no SQL. SQL injection mitigation is explicitly transferred to later PostgreSQL plans in the same phase and verified for Plans 06-02 through 06-04. | GSD security auditor | 2026-04-18 |
| AR-06-02 | 06-03/T-06-03 | Append transaction code does not execute dispatcher retry loops. Poison retry behavior is bounded by Plan 06-04 dispatcher/storage behavior and verified there. | GSD security auditor | 2026-04-18 |
| AR-06-03 | 06-04/T-06-05 | Dispatcher publishes outbox messages and does not issue workflow commands. Deterministic process-manager command idempotency is owned by Plan 06-05 and verified there. | GSD security auditor | 2026-04-18 |
| AR-06-04 | 06-05/T-06-02 | Process-manager code issues commands, not publisher effects. External publisher idempotency remains in Plans 06-01 and 06-04 and is verified as closed. | GSD security auditor | 2026-04-18 |
| AR-06-05 | 06-05/T-06-04 | Plan 06-05 process-manager contracts and app workflow add no SQL. Durable offset persistence flows through `ProcessManagerOffsetStore`; PostgreSQL SQL injection mitigation was verified in Plan 06-02. | GSD security auditor | 2026-04-18 |

## Unregistered Flags

None.

Executor summaries with threat flag sections reported no unregistered flags:

| Summary | Result |
|---------|--------|
| 06-03-SUMMARY.md | No new network endpoints, auth paths, file access patterns, or trust-boundary schema changes beyond planned append-to-outbox persistence. |
| 06-04-SUMMARY.md | No new network endpoints, auth paths, file access patterns, or trust-boundary schema changes beyond planned dispatcher-to-publisher and PostgreSQL outbox surfaces. |
| 06-05-SUMMARY.md | No new network endpoints, auth paths, file access patterns, or unplanned trust-boundary schema changes; planned workflow boundaries used tenant/correlation propagation and deterministic idempotency keys. |

## Verification Notes

| Check | Result |
|-------|--------|
| Threat model extraction | 25 plan-scoped threats extracted from 5 Phase 06 PLAN files. |
| Mitigated threats | 20/20 closed by implementation and tests. |
| Accepted threats | 5/5 closed by bounded accepted-risk rationale above. |
| Transferred threats | 0. |
| Threat flags | No unregistered flags. |
| Local verification run | `cargo test -p es-outbox -- --nocapture` passed on 2026-04-18. |
| Local verification run | `cargo test -p app commerce_process_manager -- --nocapture` passed on 2026-04-18. |
| Existing phase verification | `06-VERIFICATION.md` records passed PostgreSQL outbox integration tests and whole-workspace tests. |
| Code review state | `06-REVIEW.md` records clean review status after code-review fixes. |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-18 | 25 | 25 | 0 | GSD security auditor |

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-18

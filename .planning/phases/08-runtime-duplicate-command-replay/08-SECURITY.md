---
phase: 08
slug: runtime-duplicate-command-replay
status: verified
threats_open: 0
asvs_level: 1
created: 2026-04-20
---

# Phase 08 - Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| Runtime -> PostgreSQL command_dedup | Runtime-provided typed reply payload becomes durable replay data. | `CommandReplayRecord { append, reply }` JSONB |
| PostgreSQL command_dedup -> Runtime | Durable JSONB is decoded into typed replay records used to answer duplicate commands. | `CommandReplayRecord` / `CommandReplyPayload` |
| Tenant A -> shared store -> Tenant B | Tenant-scoped idempotency keys coexist in one durable table. | `tenant_id`, `idempotency_key`, replay payload |
| Runtime shard cache -> command caller | Shard-local replay records are used to answer duplicate caller requests. | `DedupeRecord` and typed replies |
| Domain aggregate -> runtime idempotency layer | Idempotency decisions must intercept commands before domain state is read or mutated. | command envelope metadata and reply payload |
| HTTP client -> adapter -> runtime gateway | External JSON requests cross into typed command envelopes and runtime replies cross back into JSON responses. | JSON command requests and typed response DTOs |
| Process manager -> command gateways | Durable event replay can cause deterministic follow-up commands to be submitted again. | source events and follow-up command envelopes |
| Validation artifacts -> executor/checker | Validation metadata determines whether replay paths are considered covered. | phase validation and verification artifacts |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| 08-01/T-08-01 | Tampering | `command_dedup.response_payload` | mitigate | `insert_dedupe_result` encodes `CommandReplayRecord { append, reply }` after events/outbox data are inserted and before transaction commit. Evidence: `crates/es-store-postgres/src/sql.rs:54`, `crates/es-store-postgres/src/sql.rs:62`, `crates/es-store-postgres/src/sql.rs:68`, `crates/es-store-postgres/src/sql.rs:350`, `crates/es-store-postgres/src/sql.rs:391`. | closed |
| 08-01/T-08-02 | Tampering / Information Disclosure | `lookup_command_replay` | mitigate | Replay lookup binds both tenant and idempotency key; tenant-scope test asserts same idempotency key under another tenant returns `None`. Evidence: `crates/es-store-postgres/src/sql.rs:415`, `crates/es-store-postgres/src/sql.rs:422`, `crates/es-store-postgres/src/sql.rs:423`, `crates/es-store-postgres/tests/dedupe.rs:147`. | closed |
| 08-01/T-08-03 | Tampering / Denial of Service | replay JSON decode | mitigate | Typed replay rows decode through serde; legacy append-only rows return `None`; corrupt payloads return `StoreError::DedupeResultDecode`. Evidence: `crates/es-store-postgres/src/error.rs:91`, `crates/es-store-postgres/src/sql.rs:153`, `crates/es-store-postgres/src/sql.rs:431`, `crates/es-store-postgres/src/sql.rs:433`, `crates/es-store-postgres/src/sql.rs:435`. | closed |
| 08-01/T-08-04 | Tampering | outbox/process-manager duplicate effects | transfer | Transfer target is Plan 08-03 process-manager retry coverage; duplicate follow-up retry is verified there without process-manager-local dedupe. Evidence: `.planning/phases/08-runtime-duplicate-command-replay/08-03-PLAN.md` threat model and `crates/app/src/commerce_process_manager.rs:1038`. | closed |
| 08-02/T-08-01 | Tampering | `ShardState::process_next_handoff` | mitigate | Local and durable duplicate branches execute before rehydration and domain `decide`; tests assert cache/store hits skip append and state mutation. Evidence: `crates/es-runtime/src/shard.rs:175`, `crates/es-runtime/src/shard.rs:187`, `crates/es-runtime/src/shard.rs:218`, `crates/es-runtime/src/shard.rs:235`, `crates/es-runtime/tests/runtime_flow.rs:656`, `crates/es-runtime/tests/runtime_flow.rs:724`. | closed |
| 08-02/T-08-02 | Tampering | `DedupeKey` and runtime lookup | mitigate | `DedupeKey` includes tenant plus idempotency key; runtime lookup passes both from envelope metadata. Evidence: `crates/es-runtime/src/cache.rs:51`, `crates/es-runtime/src/cache.rs:55`, `crates/es-runtime/src/cache.rs:57`, `crates/es-runtime/src/shard.rs:170`, `crates/es-runtime/src/shard.rs:187`. | closed |
| 08-02/T-08-03 | Tampering / Denial of Service | `RuntimeEventCodec::decode_reply` | mitigate | Runtime codec owns reply validation and mismatches return `RuntimeError::Codec`. Evidence: `crates/es-runtime/src/command.rs:90`, `crates/es-runtime/src/command.rs:97`, `crates/es-runtime/tests/runtime_flow.rs:161`, `crates/es-runtime/tests/runtime_flow.rs:163`, `crates/es-runtime/tests/runtime_flow.rs:168`, `crates/app/src/stress.rs:574`. | closed |
| 08-02/T-08-04 | Tampering | duplicate append race branch | mitigate | `AppendOutcome::Duplicate` performs durable replay lookup and sends decoded replay outcome or codec error, not `decision.reply`. Evidence: `crates/es-runtime/src/shard.rs:330`, `crates/es-runtime/src/shard.rs:334`, `crates/es-runtime/src/shard.rs:339`, `crates/es-runtime/src/shard.rs:346`, `crates/es-runtime/tests/runtime_flow.rs:782`. | closed |
| 08-03/T-08-01 | Tampering | HTTP duplicate retry flow | mitigate | HTTP duplicate retry test uses real order `CommandEngine`, processes both requests, asserts one append and original committed response. Evidence: `crates/adapter-http/tests/commerce_api.rs:162`, `crates/adapter-http/tests/commerce_api.rs:175`, `crates/adapter-http/tests/commerce_api.rs:185`, `crates/adapter-http/tests/commerce_api.rs:195`, `crates/adapter-http/tests/commerce_api.rs:203`. | closed |
| 08-03/T-08-02 | Tampering | HTTP idempotency key reuse | mitigate | Adapter passes request idempotency key into the runtime envelope and no adapter-local idempotency cache was found. Evidence: `crates/adapter-http/src/commerce.rs:247`, `crates/adapter-http/src/commerce.rs:468`, `crates/adapter-http/src/commerce.rs:477`, `crates/adapter-http/tests/commerce_api.rs:215`. | closed |
| 08-03/T-08-03 | Tampering / Denial of Service | replay payload typed response | mitigate | HTTP and process-manager tests use typed replay codecs and assert committed positions/reply shape; codec mismatch behavior is covered by Plan 08-02. Evidence: `crates/adapter-http/tests/commerce_api.rs:205`, `crates/adapter-http/tests/commerce_api.rs:211`, `crates/app/src/commerce_process_manager.rs:358`, `crates/app/src/commerce_process_manager.rs:432`, `crates/app/src/commerce_process_manager.rs:1118`, `crates/app/src/commerce_process_manager.rs:1119`. | closed |
| 08-03/T-08-04 | Tampering / Repudiation | `CommerceOrderProcessManager` follow-up retry | mitigate | The process-manager replay test processes the same source event twice before offset advancement, asserts deterministic keys, one append per downstream store, and original outcomes without local dedupe. Evidence: `crates/app/src/commerce_process_manager.rs:1038`, `crates/app/src/commerce_process_manager.rs:1067`, `crates/app/src/commerce_process_manager.rs:1080`, `crates/app/src/commerce_process_manager.rs:1101`, `crates/app/src/commerce_process_manager.rs:1114`. | closed |

---

## Accepted Risks Log

No accepted risks.

---

## Unregistered Flags

No `## Threat Flags` sections were present in `08-01-SUMMARY.md`, `08-02-SUMMARY.md`, or `08-03-SUMMARY.md`; no unregistered flags were logged.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-20 | 12 | 12 | 0 | Codex gsd-security-auditor |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-20

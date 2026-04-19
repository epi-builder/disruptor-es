---
phase: 08-runtime-duplicate-command-replay
verified: 2026-04-19T14:55:57Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
---

# Phase 8: Runtime Duplicate Command Replay Verification Report

**Phase Goal:** Repeated commands with the same tenant and idempotency key are detected before aggregate decision and return the original committed result, preserving duplicate retry behavior across HTTP, runtime, storage, and process-manager replay paths.
**Verified:** 2026-04-19T14:55:57Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Shard command processing checks shard-local idempotency before rehydrating aggregate state or calling domain `decide`. | VERIFIED | `crates/es-runtime/src/shard.rs:170` builds tenant-scoped `DedupeKey`; `:175` checks `self.dedupe.get(&dedupe_key)`; rehydration starts at `:218` and `A::decide` at `:235`. |
| 2 | Shard command processing checks durable PostgreSQL idempotency before calling `A::decide` on cache misses. | VERIFIED | `crates/es-runtime/src/shard.rs:187` calls `store.lookup_command_replay(...)`; the durable-hit branch returns at `:207`, before rehydration and `A::decide`. |
| 3 | Duplicate commands return the original decoded reply and committed append without appending events or applying duplicate events to cache. | VERIFIED | Cache and store duplicate branches call `replay_command_outcome` (`crates/es-runtime/src/shard.rs:176`, `:195`) and return before append. `cargo test -p es-runtime runtime_duplicate -- --nocapture` passed 2 duplicate replay tests. |
| 4 | A durable command dedupe row stores the original typed reply payload beside the committed append summary. | VERIFIED | `crates/es-store-postgres/src/sql.rs:391` encodes `CommandReplayRecord { append, reply }` when the append request carries `command_reply_payload`; insert stores it into `command_dedup.response_payload` at `:354-364`. |
| 5 | Existing append duplicate behavior still returns the original committed append summary from `command_dedup`. | VERIFIED | `decode_committed_append_from_dedupe_payload` in `crates/es-store-postgres/src/sql.rs:153` decodes new replay wrappers first and legacy `CommittedAppend` second. Plan tests and existing duplicate behavior are present. |
| 6 | Runtime duplicate replay data remains shard-local and does not introduce global mutable business-state locks. | VERIFIED | `DedupeCache` is a shard-owned `HashMap<DedupeKey, DedupeRecord>` in `crates/es-runtime/src/cache.rs:67`; grep found no adapter/process-manager `DedupeCache`, `HashMap<.*idempotency`, or global idempotency locks. |
| 7 | Durable store dedupe remains the source of truth when the runtime cache misses or after restart. | VERIFIED | `PostgresEventStore::lookup_command_replay` delegates to SQL lookup (`crates/es-store-postgres/src/event_store.rs:99`); restart and tenant-scope integration tests passed via `cargo test -p es-store-postgres command_replay -- --nocapture`. |
| 8 | HTTP duplicate retries are covered by a test proving original committed results are replayed through the real gateway/runtime path. | VERIFIED | `duplicate_place_order_retry_returns_original_response` drives two HTTP requests through `CommandEngine<Order>::process_one`, asserts one append and matching committed response fields (`crates/adapter-http/tests/commerce_api.rs:163-214`). Targeted test passed. |
| 9 | Deterministic process-manager follow-up retries are covered by a test proving original committed results are replayed. | VERIFIED | `process_manager_replayed_followups_return_original_outcomes` processes the same event twice through real product/order engines and asserts one append per downstream store (`crates/app/src/commerce_process_manager.rs:1038-1119`). Targeted test passed. |
| 10 | The Phase 08 requirement-level validation artifact accounts for storage, runtime, HTTP, and process-manager replay commands. | VERIFIED | `.planning/phases/08-runtime-duplicate-command-replay/08-VALIDATION.md` has `nyquist_compliant: true`, `wave_0_complete: true`, and green rows for Plans 08-01, 08-02, and 08-03. |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/es-store-postgres/src/models.rs` | Reply payload and replay DTOs | VERIFIED | `CommandReplyPayload`, `CommandReplayRecord`, `AppendRequest::command_reply_payload`, and unit tests exist. |
| `crates/es-store-postgres/src/sql.rs` | Durable replay persistence and lookup | VERIFIED | Wrapper encoding, compatibility decode, tenant-scoped `SELECT response_payload` lookup, and replay decode exist. |
| `crates/es-store-postgres/src/event_store.rs` | Public replay lookup API | VERIFIED | `PostgresEventStore::lookup_command_replay` delegates to SQL lookup. |
| `crates/es-store-postgres/tests/dedupe.rs` | Durable replay tests | VERIFIED | Round-trip, restart, and tenant-scope tests exist and passed. |
| `crates/es-runtime/src/command.rs` | Reply encode/decode codec contract | VERIFIED | `RuntimeEventCodec` requires `encode_reply` and `decode_reply`. |
| `crates/es-runtime/src/cache.rs` | Shard-local replayable dedupe records | VERIFIED | `DedupeRecord` stores full `CommandReplayRecord`; `DedupeKey` includes tenant and idempotency key. |
| `crates/es-runtime/src/store.rs` | Runtime-facing durable replay lookup | VERIFIED | `RuntimeEventStore::lookup_command_replay` and PostgreSQL adapter delegate exist. |
| `crates/es-runtime/src/shard.rs` | Pre-decision local and durable duplicate replay branches | VERIFIED | Cache lookup and durable lookup precede rehydration and `A::decide`; append requests attach reply payloads. |
| `crates/es-runtime/tests/runtime_flow.rs` | Runtime warm and durable replay tests | VERIFIED | Runtime duplicate tests passed. |
| `crates/adapter-http/tests/commerce_api.rs` | HTTP duplicate retry test | VERIFIED | Uses real `CommandEngine<Order>` and `process_one`; no adapter-local idempotency cache found. |
| `crates/app/src/commerce_process_manager.rs` | Process-manager retry test | VERIFIED | Uses real product/order engines; no process-manager-local idempotency set/map found. |
| `.planning/phases/08-runtime-duplicate-command-replay/08-VALIDATION.md` | Validation sign-off | VERIFIED | Requirement-level sampling rows are green and include the planned commands. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/es-store-postgres/src/event_store.rs` | `crates/es-store-postgres/src/sql.rs` | `PostgresEventStore::lookup_command_replay` delegates to SQL | VERIFIED | Manual check: `event_store.rs:104` calls `sql::lookup_command_replay`. Tool regex failed because of an escaped pattern issue, not missing wiring. |
| `crates/es-store-postgres/src/sql.rs` | `command_dedup.response_payload` | `SELECT response_payload FROM command_dedup WHERE tenant_id = $1 AND idempotency_key = $2` | VERIFIED | Query exists in append duplicate paths and replay lookup. |
| `crates/es-store-postgres/src/sql.rs` | `crates/es-store-postgres/src/models.rs` | Response payload serializes `CommandReplayRecord` when a reply payload exists | VERIFIED | `encode_dedupe_response_payload` serializes `CommandReplayRecord`. |
| `crates/es-runtime/src/shard.rs` | `crates/es-runtime/src/cache.rs` | Dedupe lookup immediately after envelope extraction | VERIFIED | Manual check: `self.dedupe.get(&dedupe_key)` exists at `shard.rs:175`. Tool regex failed because of pattern escaping. |
| `crates/es-runtime/src/shard.rs` | `RuntimeEventStore::lookup_command_replay` | Durable lookup before rehydrate and `A::decide` | VERIFIED | Store lookup occurs at `shard.rs:187`; rehydrate and decide occur later at `:218` and `:235`. |
| `crates/es-runtime/src/shard.rs` | `AppendRequest::with_command_reply_payload` | First execution persists encoded reply with append request | VERIFIED | `with_command_reply_payload` is called at `shard.rs:290` before append. |
| `crates/adapter-http/tests/commerce_api.rs` | `CommandGateway<Order>` | Duplicate HTTP requests use same key and real runtime engine | VERIFIED | Test uses `order_engine.gateway()` and `order_engine.process_one()`. |
| `crates/app/src/commerce_process_manager.rs` | `CommandGateway<Product>` and `CommandGateway<Order>` | Same source event processed twice before offset advancement | VERIFIED | Test uses product and order engines and asserts duplicate follow-up replay. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `crates/es-store-postgres/src/sql.rs` | `response_payload` | `insert_dedupe_result` encodes append plus reply into `command_dedup.response_payload`; lookup reads by tenant/idempotency key. | Yes | FLOWING |
| `crates/es-runtime/src/shard.rs` | `CommandReplayRecord.reply` | Shard-local `DedupeCache` or `RuntimeEventStore::lookup_command_replay`; decoded through `RuntimeEventCodec::decode_reply`. | Yes | FLOWING |
| `crates/adapter-http/tests/commerce_api.rs` | HTTP response body | Router awaits real `CommandGateway<Order>` response from `CommandEngine::process_one`. | Yes | FLOWING |
| `crates/app/src/commerce_process_manager.rs` | Follow-up command outcomes | Process manager submits through real command gateways; engines process and replay duplicates. | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Runtime cache and durable duplicate replay skip rehydrate/decide/append | `cargo test -p es-runtime runtime_duplicate -- --nocapture` | 2 tests passed | PASS |
| PostgreSQL command replay round-trip, restart, and tenant-scope lookup | `cargo test -p es-store-postgres command_replay -- --nocapture` | 3 tests passed | PASS |
| HTTP duplicate retry returns original response shape through runtime | `cargo test -p adapter-http duplicate_place_order_retry_returns_original_response -- --nocapture` | 1 test passed | PASS |
| Process-manager duplicate follow-up retry replays original outcomes | `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` | 1 test passed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| STORE-03 | 08-01, 08-02, 08-03 | Command deduplication returns the prior committed result for a repeated tenant/idempotency key. | SATISFIED | Store replay lookup is tenant-scoped and runtime/HTTP/process-manager duplicate tests assert original committed results. |
| RUNTIME-03 | 08-02 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. | SATISFIED | Cache and dedupe data structures are fields of `ShardState`; no adapter/process-manager idempotency maps or global business-state locks were found. Residual tenant-scope warning noted below. |
| RUNTIME-05 | 08-01, 08-02, 08-03 | Command replies are sent only after durable event-store append commit succeeds. | SATISFIED | First execution sends `CommandOutcome` only in committed append branch; duplicate replay returns committed replay records. |
| INT-04 | 08-03 | Process-manager example reacts to order/product events and issues follow-up commands through the same command gateway. | SATISFIED | Process-manager test uses real product/order `CommandEngine` gateways and duplicate replay assertions. |
| API-01 | 08-03 | Thin HTTP adapter decodes requests, attaches metadata, sends through bounded ingress, and awaits replies. | SATISFIED | HTTP duplicate test drives `commerce_routes` through `CommandGateway<Order>` and runtime engine. |
| API-03 | 08-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads. | SATISFIED | HTTP duplicate test asserts stream revision, first/last revision, global positions, event IDs, and typed placed reply, plus full deterministic body equality. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/es-runtime/src/shard.rs` | 56 | `ShardHandoffToken::placeholder` | Info | Functional disruptor initialization scaffold; not an unimplemented placeholder. |
| `crates/es-runtime/src/cache.rs` | 8 | `AggregateCache` keyed by `StreamId` only | Warning | Code review CR-01 identifies possible cross-tenant aggregate cache bleed for non-duplicate commands sharing a stream id. Phase 08 dedupe replay is tenant-scoped and pre-cache, so this is residual project risk, not a Phase 08 duplicate replay blocker. |
| `crates/app/src/commerce_process_manager.rs` | 78 | Process-manager reserve/release idempotency keys use source event plus product id only | Warning | Code review WR-01 identifies possible key collision for duplicate product lines in one order. Single-line retry replay is verified; duplicate product-line key uniqueness remains a follow-up risk. |

### Human Verification Required

None. Phase 08 behaviors are covered by automated storage, runtime, adapter, and app tests.

### Gaps Summary

No Phase 08 goal-blocking gaps found. The duplicate replay contract is implemented at the storage layer, wired through the runtime before aggregate rehydration/decision, and verified at HTTP and process-manager boundaries.

Residual warnings from code review remain worth addressing in follow-up work:

- Aggregate cache is not tenant-scoped for non-duplicate commands with the same stream id.
- Process-manager follow-up idempotency keys can collide when an order contains multiple lines for the same product.

These do not invalidate the Phase 08 duplicate command replay goal as verified here, because the implemented dedupe key is tenant-scoped and duplicate replay is checked before aggregate cache use.

---

_Verified: 2026-04-19T14:55:57Z_
_Verifier: Claude (gsd-verifier)_

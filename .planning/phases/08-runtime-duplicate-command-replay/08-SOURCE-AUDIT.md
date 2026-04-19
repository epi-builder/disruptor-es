# Phase 08 Source Audit

SOURCE | ID | Feature/Requirement | Plan | Status | Notes
--- | --- | --- | --- | --- | ---
GOAL | - | Repeated commands with the same tenant and idempotency key are detected before aggregate decision and return the original committed result across HTTP, runtime, storage, and process-manager replay paths. | 08-01, 08-02, 08-03 | COVERED | Store persists original outcome, runtime checks before decision, adapter/app paths verify behavior.
REQ | STORE-03 | Command deduplication returns the prior committed result for a repeated tenant/idempotency key. | 08-01, 08-02 | COVERED | Durable `StoredCommandOutcome` plus runtime replay from cache/store.
REQ | RUNTIME-03 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. | 08-02 | COVERED | `DedupeCache` remains shard-owned; production global lock grep is required.
REQ | RUNTIME-05 | Command replies are sent only after durable event-store append commit succeeds. | 08-02 | COVERED | First commits populate dedupe after append; duplicates replay only prior committed outcomes.
REQ | INT-04 | Process-manager example issues follow-up commands through the same command gateway. | 08-03 | COVERED | Duplicate process-manager replay test observes gateway-submitted deterministic keys.
REQ | API-01 | Thin HTTP adapter decodes, attaches metadata, sends through bounded ingress, and awaits replies. | 08-03 | COVERED | HTTP duplicate regression proves duplicate behavior still flows through `CommandGateway`.
REQ | API-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads. | 08-03 | COVERED | Duplicate response test checks current correlation plus original append/reply fields.
RESEARCH | R-01 | Add pre-decision duplicate lookup: shard-local cache first, durable store second. | 08-02 | COVERED | `ShardState::process_next_handoff` must check before `rehydrate_state` and `A::decide`.
RESEARCH | R-02 | Persist original typed reply payload with committed append metadata. | 08-01, 08-02 | COVERED | Store owns JSON payload, runtime codec encodes/decodes `A::Reply`.
RESEARCH | R-03 | Add durable lookup method to `RuntimeEventStore` and `PostgresEventStore`. | 08-01, 08-02 | COVERED | Plans require `load_command_outcome`.
RESEARCH | R-04 | Do not implement HTTP-only idempotency or adapter-local retry maps. | 08-03 | COVERED | Adapter grep rejects maps/direct storage dependency.
RESEARCH | R-05 | Process-manager crash/retry must reuse deterministic keys with runtime/store replay. | 08-03 | COVERED | App regression invokes the same `ProcessEvent` twice.
SECURITY | T-08-01 | Cross-tenant idempotency collision. | 08-01, 08-02, 08-03 | COVERED | All threat models require `(tenant_id, idempotency_key)`.
SECURITY | T-08-02 | Replay result substitution. | 08-01, 08-02, 08-03 | COVERED | Stored original response is decoded; duplicate decision recomputation is forbidden.
SECURITY | T-08-03 | Adapter-local retry state bypass. | 08-03 | COVERED | Adapter remains a thin gateway client.
SECURITY | T-08-04 | Process-manager crash/retry repeats side effects. | 08-02, 08-03 | COVERED | Runtime replay plus process-manager duplicate regression.
CONTEXT | - | No `08-CONTEXT.md` exists. | - | NOT APPLICABLE | No locked D-XX decisions or deferred ideas for this phase.

Result: all source items are covered; no unplanned items found.

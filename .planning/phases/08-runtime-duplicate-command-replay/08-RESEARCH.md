# Phase 8: Runtime Duplicate Command Replay - Research

**Researched:** 2026-04-19 [VERIFIED: environment date]
**Domain:** Rust event-sourced command runtime idempotency across shard cache, PostgreSQL command dedupe, HTTP adapter replies, and process-manager replay [VERIFIED: .planning/ROADMAP.md; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
**Confidence:** HIGH for local architecture and current gap; MEDIUM for durable typed reply schema shape because it requires an implementation decision [VERIFIED: code audit; ASSUMED]

## User Constraints

No `08-CONTEXT.md` exists for this phase, so there are no phase-specific locked decisions, discretion notes, or deferred ideas to copy verbatim. [VERIFIED: `gsd-tools init phase-op 8`; VERIFIED: no `*-CONTEXT.md` file found]

Project-level constraints still apply: prefer `pnpm` for Node tooling and `uv` for Python tooling; keep this Rust-first; treat the event store as source of truth; never treat disruptor rings as durable state; route the same aggregate or partition key to the same shard owner; keep hot business state processor-local where practical; publish externally through outbox rows committed with domain events; separate adapter, command engine, projection, and outbox concerns. [VERIFIED: AGENTS.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STORE-03 | Command deduplication returns the prior committed result for a repeated tenant/idempotency key. [VERIFIED: .planning/REQUIREMENTS.md] | Requires a durable pre-decision lookup path plus persisted duplicate response data that can replay typed replies without running `decide` again. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-store-postgres/src/sql.rs] |
| RUNTIME-03 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. [VERIFIED: .planning/REQUIREMENTS.md] | Keep dedupe in `ShardState`; change `DedupeRecord` to carry enough reply data for fast replay and check it before rehydration. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: crates/es-runtime/src/shard.rs] |
| RUNTIME-05 | Command replies are sent only after durable event-store append commit succeeds. [VERIFIED: .planning/REQUIREMENTS.md] | Duplicate replies must come from a previously committed durable result, not a freshly computed domain decision or uncommitted runtime sequence. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| INT-04 | Process-manager follow-up commands go through the same command gateway. [VERIFIED: .planning/REQUIREMENTS.md] | Deterministic process-manager idempotency keys already exist; runtime replay must make crash/retry after command success but before offset advancement idempotent. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs] |
| API-01 | Thin HTTP adapter decodes, attaches metadata, sends through bounded ingress, and awaits replies. [VERIFIED: .planning/REQUIREMENTS.md] | HTTP handlers already submit through `CommandGateway`; runtime behavior must ensure duplicate HTTP retries receive replayed outcomes. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| API-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads. [VERIFIED: .planning/REQUIREMENTS.md] | Duplicate replay must preserve durable append fields and typed reply DTO behavior; current adapter maps `CommandOutcome<A::Reply>` to DTOs. [VERIFIED: crates/adapter-http/src/commerce.rs] |

</phase_requirements>

## Summary

The blocker is specifically in `ShardState::process_next_handoff`: it rehydrates aggregate state and calls `A::decide` before checking `DedupeCache::get`, so a retried command can be rejected by already-mutated aggregate state before PostgreSQL command dedupe is reached. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

The current PostgreSQL append path already enforces tenant-scoped command dedupe inside the append transaction using a transaction-scoped advisory lock, `command_dedup`, and `AppendOutcome::Duplicate`. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: migrations/20260417000000_event_store.sql] However, runtime cannot use that durable dedupe before `decide` because `RuntimeEventStore` exposes only `append` and `load_rehydration`, and `append` needs newly encoded events from `decide`. [VERIFIED: crates/es-runtime/src/store.rs; VERIFIED: crates/es-runtime/src/shard.rs]

Primary recommendation: add a pre-decision runtime duplicate-result lookup that checks shard-local cache first and durable store second, and persist/cache the original `CommandOutcome` reply data, not only `CommittedAppend`. [VERIFIED: inference from crates/es-runtime/src/command.rs and crates/es-store-postgres/src/sql.rs]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Shard-local duplicate replay | API / Backend runtime | Database / Storage | The shard owns hot-path cache and can avoid rehydration/decision for warm duplicates. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: crates/es-runtime/src/shard.rs] |
| Durable duplicate replay after restart | Database / Storage | API / Backend runtime | PostgreSQL is the source of truth for prior committed results after runtime cache loss. [VERIFIED: AGENTS.md; VERIFIED: migrations/20260417000000_event_store.sql] |
| Typed reply reconstruction | API / Backend runtime | Database / Storage | Runtime owns `A::Reply`; storage can persist JSON payloads but cannot know aggregate-specific Rust types without a codec boundary. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/models.rs] |
| HTTP duplicate behavior | Browser / Client entry through HTTP adapter | API / Backend runtime | HTTP handlers are thin and await runtime replies; they should not implement dedupe logic. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: docs/hot-path-rules.md] |
| Process-manager crash/retry behavior | API / Backend workflow layer | API / Backend runtime and Database / Storage | Process managers submit deterministic follow-up idempotency keys through gateways and advance offsets after replies. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs] |
| Duplicate metrics/traces | API / Backend runtime | Adapter observability | Runtime already emits command latency and dedupe-related outcomes; bounded metric labels must remain. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/app/src/observability.rs] |

## Project Constraints

- Do not move idempotency into HTTP handlers; adapters must submit through `CommandGateway` and must not mutate aggregate, projector, or outbox state directly. [VERIFIED: docs/hot-path-rules.md; VERIFIED: crates/adapter-http/src/commerce.rs]
- Do not use disruptor sequence numbers as durable replay positions. [VERIFIED: AGENTS.md; VERIFIED: docs/hot-path-rules.md]
- Do not use global `Arc<Mutex<HashMap<...>>>` business-state maps for aggregate or dedupe state. [VERIFIED: AGENTS.md; VERIFIED: docs/hot-path-rules.md]
- Preserve deterministic synchronous aggregate `decide/apply`; idempotency replay must bypass duplicate decisions instead of changing domain logic. [VERIFIED: crates/es-kernel/src/lib.rs; VERIFIED: crates/es-runtime/src/shard.rs]
- Keep outbox publication and process-manager offsets durable; process-manager replay must rely on deterministic command idempotency keys plus runtime/store duplicate replay. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs]

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust workspace | edition 2024, rust-version 1.85 | Phase implementation language and crate boundaries | Existing workspace policy and installed toolchain match this baseline. [VERIFIED: Cargo.toml; VERIFIED: `rustc --version`] |
| `es-runtime` crate | local 0.1.0 | Shard-local cache, command envelope, gateway, engine, and runtime store port | This is where the broken pre-decision path lives. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/src/store.rs] |
| `es-store-postgres` crate | local 0.1.0 | Durable append, command dedupe, rehydration, and SQL ownership | This crate already owns `command_dedup` and append transaction behavior. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-store-postgres/src/models.rs] |
| `sqlx` | 0.8.6 | PostgreSQL access and migrations | Existing store implementation uses explicit SQL through SQLx. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/src/sql.rs] |
| `tokio` | 1.52.0 | Async runtime and one-shot command replies | `CommandReply` and runtime tests use Tokio channels and async tests. [VERIFIED: Cargo.toml; VERIFIED: crates/es-runtime/src/command.rs] |
| `serde` / `serde_json` | 1.0.228 / 1.0.149 | Persisting generic duplicate response payloads and typed reply payloads | Existing store models already serialize `CommittedAppend` to JSONB response payload. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/src/sql.rs] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `axum` | 0.8.9 | HTTP command adapter tests | Use for duplicate HTTP retry tests at adapter boundary. [VERIFIED: Cargo.toml; VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| `testcontainers` / `testcontainers-modules` | 0.25.0 / 0.13.0 | PostgreSQL integration tests | Use for durable dedupe lookup and schema migration tests. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/tests/dedupe.rs] |
| `metrics` | 0.24.3 | Duplicate outcome counters/histograms | Use existing bounded labels such as aggregate/outcome/shard; do not label with tenant or idempotency key. [VERIFIED: Cargo.toml; VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/app/src/observability.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Runtime/store pre-decision lookup | HTTP-only idempotency middleware | Wrong tier; process-manager replay and non-HTTP gateways would still break. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: docs/template-guide.md] |
| Persisted typed reply payload | Re-run `A::decide` on duplicate | Fails the audited flow because already-mutated state can produce a fresh domain error or different reply. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: crates/es-runtime/src/shard.rs] |
| Extend existing store/runtime contracts | Adopt a generic CQRS framework | Out of scope; project uses owned event-store/runtime abstractions. [VERIFIED: AGENTS.md; VERIFIED: .planning/STATE.md] |
| Cache-only duplicate replay | No durable lookup | Fails after restart and process-manager crash/retry because shard memory is not durable. [VERIFIED: AGENTS.md; VERIFIED: crates/es-runtime/src/cache.rs] |

**Installation:** No new package is recommended for this phase. [VERIFIED: Cargo.toml; VERIFIED: code audit]

**Version verification:** Existing relevant versions are in the workspace manifest; `rustc 1.85.1`, `cargo 1.85.1`, Docker 29.3.1, and SQLx/Testcontainers dependencies are available in the current environment. [VERIFIED: command output; VERIFIED: Cargo.toml]

## Architecture Patterns

### System Architecture Diagram

```text
HTTP adapter / process manager / other gateway client
        |
        v
CommandGateway bounded ingress
        |
        v
PartitionRouter by tenant + aggregate partition key
        |
        v
ShardHandle -> disruptor handoff token -> ShardState::process_next_handoff
        |
        v
Build DedupeKey(tenant_id, idempotency_key)
        |
        +--> shard-local DedupeCache hit?
        |         |
        |         v
        |   decode/replay cached original CommandOutcome -> reply
        |
        +--> durable command_dedup hit?
        |         |
        |         v
        |   decode persisted original CommandOutcome -> warm cache -> reply
        |
        +--> miss
                  |
                  v
          rehydrate aggregate state -> A::decide -> encode events
                  |
                  v
          PostgreSQL append transaction:
          advisory idempotency lock -> dedupe recheck -> stream lock -> insert events/outbox -> persist duplicate response payload
                  |
                  v
          update aggregate cache + dedupe cache -> reply after commit
```

This flow keeps the cache as an optimization and PostgreSQL as the source of truth. [VERIFIED: AGENTS.md; VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: inference from target design]

### Recommended Project Structure

```text
crates/es-runtime/src/
├── cache.rs          # generic duplicate records with original reply + committed append
├── command.rs        # command outcome and reply codec boundary
├── shard.rs          # pre-decision duplicate lookup and replay
├── store.rs          # runtime-facing durable dedupe lookup method
└── tests/            # runtime duplicate replay regressions

crates/es-store-postgres/src/
├── models.rs         # durable duplicate result DTOs
├── sql.rs            # command_dedup lookup/insert response payload changes
└── tests/dedupe.rs   # PostgreSQL duplicate result contract tests

crates/adapter-http/tests/
└── commerce_api.rs   # duplicate HTTP retry response contract

crates/app/src/
└── commerce_process_manager.rs or tests  # crash/retry deterministic follow-up coverage
```

The files above are the minimum likely touch points because current contracts split runtime, store, adapter, and process-manager behavior across these crates. [VERIFIED: `rg --files`; VERIFIED: code audit]

### Pattern 1: Duplicate Replay Before State Loading

**What:** Build the tenant-scoped dedupe key from the envelope and check runtime cache plus durable dedupe before calling `load_rehydration` or `A::decide`. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/src/cache.rs]

**When to use:** Every command handoff in `ShardState::process_next_handoff`, including HTTP and process-manager submissions. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/app/src/commerce_process_manager.rs]

**Example:**

```rust
// Source: crates/es-runtime/src/shard.rs and recommended Phase 8 shape.
let dedupe_key = DedupeKey {
    tenant_id: envelope.metadata.tenant_id.clone(),
    idempotency_key: envelope.idempotency_key.clone(),
};

if let Some(record) = self.dedupe.get(&dedupe_key) {
    let outcome = record.outcome.clone();
    let _ = envelope.reply.send(Ok(outcome));
    return Ok(true);
}

if let Some(record) = store.load_dedupe(&dedupe_key).await? {
    let outcome = codec.decode_duplicate(record)?;
    self.dedupe.record(dedupe_key, outcome.clone().into());
    let _ = envelope.reply.send(Ok(outcome));
    return Ok(true);
}
```

The exact type names should follow the implementation, but the ordering must be fixed: duplicate check first, rehydration/decision only on miss. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: inference from code audit]

### Pattern 2: Persist The Original Typed Reply Payload

**What:** Store a duplicate response payload that contains both durable append metadata and a serialized representation of the original aggregate reply. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: inference from code audit]

**When to use:** Required when duplicate replay must return the original success payload after runtime restart or durable cache miss. [VERIFIED: .planning/ROADMAP.md; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

**Example:**

```rust
// Source: crates/es-runtime/src/command.rs and crates/es-store-postgres/src/models.rs.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredCommandOutcome {
    pub append: CommittedAppend,
    pub reply_payload: serde_json::Value,
}
```

This is an implementation sketch; the planner should require codec methods or trait bounds so generic runtime can encode/decode `A::Reply` without making storage depend on aggregate types. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/models.rs; VERIFIED: inference from code audit]

### Pattern 3: Commit-Gated Cache Population

**What:** Record dedupe cache entries only after the append has committed or after durable duplicate lookup returns a stored committed result. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-store-postgres/src/sql.rs]

**When to use:** On `AppendOutcome::Committed`, `AppendOutcome::Duplicate`, and durable pre-decision lookup hit. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: inference from target design]

**Example:**

```rust
// Source: crates/es-runtime/src/shard.rs.
match store.append(append_request).await? {
    AppendOutcome::Committed(committed) => {
        let outcome = CommandOutcome::new(decision.reply, committed);
        self.dedupe.record(dedupe_key, DedupeRecord::from_outcome(outcome.clone()));
        let _ = envelope.reply.send(Ok(outcome));
    }
    AppendOutcome::Duplicate(stored) => {
        let outcome = codec.decode_duplicate(stored)?;
        self.dedupe.record(dedupe_key, DedupeRecord::from_outcome(outcome.clone()));
        let _ = envelope.reply.send(Ok(outcome));
    }
}
```

### Anti-Patterns to Avoid

- **Calling `A::decide` before idempotency lookup:** This is the audited bug and can turn a duplicate success into a fresh domain error. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: crates/es-runtime/src/shard.rs]
- **Returning a fresh reply with an old append:** Current runtime duplicate append handling pairs `AppendOutcome::Duplicate(committed)` with `decision.reply`, which can be inconsistent with the original command result. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: inference from code audit]
- **Storage-only fix:** PostgreSQL dedupe is not reachable before `decide` through the current `RuntimeEventStore` contract. [VERIFIED: crates/es-runtime/src/store.rs; VERIFIED: crates/es-runtime/src/shard.rs]
- **HTTP-only fix:** Process-manager retry and future gRPC/WebSocket gateways submit through the runtime and would remain affected. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: docs/template-guide.md]
- **Unbounded metric labels:** Do not label metrics with tenant IDs, command IDs, stream IDs, event IDs, or idempotency keys. [VERIFIED: crates/app/src/observability.rs]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Durable idempotency concurrency | A custom in-memory lock or process-wide mutex | PostgreSQL transaction-scoped advisory lock plus primary key on `(tenant_id, idempotency_key)` | Existing append path already serializes same-key appends and survives process restart. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: migrations/20260417000000_event_store.sql; CITED: https://www.postgresql.org/docs/current/functions-admin.html] |
| Duplicate response persistence | Ad hoc string blobs | `serde_json::Value` payload with typed encode/decode at runtime boundary | Existing schema uses JSONB response payload and Rust models derive serde. [VERIFIED: migrations/20260417000000_event_store.sql; VERIFIED: crates/es-store-postgres/src/models.rs] |
| Retry semantics in adapters | Per-endpoint dedupe maps | Runtime/store idempotency port | Adapters are thin and process managers also submit commands. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: crates/app/src/commerce_process_manager.rs] |
| Cross-process command replay | Disruptor sequence or shard memory | Durable `command_dedup` lookup | Rings and shard caches are not durable. [VERIFIED: AGENTS.md; VERIFIED: crates/es-runtime/src/cache.rs] |

**Key insight:** A cache lookup alone closes only warm duplicate retries; the phase needs durable lookup and typed reply persistence to close restart and process-manager replay paths. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: inference from code audit]

## Common Pitfalls

### Pitfall 1: Cache Hit Replays Append But Not Reply

**What goes wrong:** Duplicate replies can contain old global positions but a newly computed or missing typed reply. [VERIFIED: crates/es-runtime/src/shard.rs]
**Why it happens:** Current `DedupeRecord` stores only `CommittedAppend`, and current duplicate append branch uses `decision.reply`. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: crates/es-runtime/src/shard.rs]
**How to avoid:** Store/cache an outcome shape that includes both `CommittedAppend` and serialized or typed `A::Reply`. [VERIFIED: inference from code audit]
**Warning signs:** Tests assert only global positions or dedupe length, not the original reply payload after state mutation. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs]

### Pitfall 2: Durable Lookup Cannot Be Implemented With Current Store Port

**What goes wrong:** The planner asks runtime to call PostgreSQL dedupe before `decide`, but no method exists to do it. [VERIFIED: crates/es-runtime/src/store.rs]
**Why it happens:** `RuntimeEventStore` currently exposes `append` and `load_rehydration` only. [VERIFIED: crates/es-runtime/src/store.rs]
**How to avoid:** Add a `load_dedupe` or `lookup_command_result` method to `RuntimeEventStore` and implement it in `PostgresRuntimeEventStore`. [VERIFIED: inference from code audit]
**Warning signs:** The planned fix only edits `ShardState::process_next_handoff` and does not touch `store.rs` or `es-store-postgres`. [VERIFIED: code audit]

### Pitfall 3: Dedupe Key Scope Drift

**What goes wrong:** Duplicate commands could collide across tenants or fail to hit within the same tenant. [VERIFIED: migrations/20260417000000_event_store.sql]
**Why it happens:** The cache and database must use exactly `(tenant_id, idempotency_key)`. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: migrations/20260417000000_event_store.sql]
**How to avoid:** Reuse `DedupeKey` from envelope metadata and idempotency key for cache and store lookups. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-runtime/src/cache.rs]
**Warning signs:** Tests use same idempotency key but different tenants and expect a duplicate. [VERIFIED: crates/es-store-postgres/tests/dedupe.rs]

### Pitfall 4: Process-Manager Offset Advances Too Early

**What goes wrong:** A crash after follow-up commands but before offset advancement causes replay; without duplicate replay, follow-up commands may fail domain validation. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
**Why it happens:** `process_batch` advances offset after `manager.process` returns; follow-up commands use deterministic idempotency keys. [VERIFIED: crates/es-outbox/src/process_manager.rs; VERIFIED: crates/app/src/commerce_process_manager.rs]
**How to avoid:** Add a regression test that runs the same `ProcessEvent` twice with the same idempotency keys and already-mutated target aggregate state. [VERIFIED: inference from code audit]
**Warning signs:** Process-manager tests assert command submission order but do not drive commands through real runtime duplicate replay. [VERIFIED: crates/app/src/commerce_process_manager.rs]

## Code Examples

### Current Broken Ordering

```rust
// Source: crates/es-runtime/src/shard.rs
let current_state = if let Some(cached) = self.cache.get(&envelope.stream_id) {
    cached.clone()
} else {
    rehydrate_state(store, codec, &envelope).await?
};

let decision = A::decide(&current_state, envelope.command, &envelope.metadata)?;
```

`DedupeCache::get` is available but absent from this pre-decision section. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-runtime/src/cache.rs]

### Current Durable Dedupe Read

```rust
// Source: crates/es-store-postgres/src/sql.rs
SELECT response_payload
FROM command_dedup
WHERE tenant_id = $1 AND idempotency_key = $2
```

This lookup is currently private to append SQL and returns `CommittedAppend` from JSON. [VERIFIED: crates/es-store-postgres/src/sql.rs]

### Recommended Runtime Store Port

```rust
// Source: recommended Phase 8 contract based on crates/es-runtime/src/store.rs.
pub trait RuntimeEventStore: Clone + Send + Sync + 'static {
    fn append(&self, request: AppendRequest) -> BoxFuture<'_, StoreResult<AppendOutcome>>;
    fn load_rehydration(&self, tenant_id: &TenantId, stream_id: &StreamId)
        -> BoxFuture<'_, StoreResult<RehydrationBatch>>;
    fn load_dedupe(&self, key: DedupeKey) -> BoxFuture<'_, StoreResult<Option<StoredCommandOutcome>>>;
}
```

The exact DTO should be storage-owned or core-owned; the runtime port needs a pre-decision method regardless of naming. [VERIFIED: crates/es-runtime/src/store.rs; VERIFIED: inference from code audit]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Best-effort retry guarded only by domain validation | Idempotent request key returns the same saved result for retries | Established API pattern; Stripe documents saving and returning the same result for a key, including failures. [CITED: https://docs.stripe.com/api/idempotent_requests] | Runtime must replay prior results instead of recalculating domain behavior. [VERIFIED: inference from cited docs and audit] |
| App-local duplicate maps | Durable idempotency table with transaction protection | Already implemented in Phase 2 storage. [VERIFIED: .planning/STATE.md; VERIFIED: crates/es-store-postgres/src/sql.rs] | Phase 8 should expose the existing durable behavior before `decide`. [VERIFIED: inference from code audit] |
| Adapters own retry semantics | Gateway/runtime/store own retry semantics | Current project docs require thin gateways and runtime submission. [VERIFIED: docs/template-guide.md; VERIFIED: docs/hot-path-rules.md] | HTTP, process-manager, and future gateways share behavior. [VERIFIED: inference from project docs] |

**Deprecated/outdated:**
- Cache-only idempotency is insufficient for this project because the event store is the source of truth and process restarts must not lose duplicate replay behavior. [VERIFIED: AGENTS.md; VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- Returning only `CommittedAppend` for duplicate replay is insufficient for API-03 typed reply payloads unless the reply is reconstructed by a verified codec or stored payload. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: inference from code audit]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Durable duplicate replay should preserve typed success payloads by persisting or codec-reconstructing `A::Reply`, not only `CommittedAppend`. [ASSUMED] | Summary, Patterns, Pitfalls | If the project accepts append-only duplicate replies without typed reply fidelity, the phase could be smaller, but API-03 wording and current adapter shape suggest reply fidelity matters. |
| A2 | It is acceptable to evolve the `command_dedup.response_payload` JSON shape or add a compatible wrapper payload in Phase 8. [ASSUMED] | Architecture Patterns | If backward compatibility with existing seeded DB rows matters, the plan needs a migration/backfill compatibility task. |

## Open Questions

1. **Should duplicate replay preserve the original correlation ID or use the retry request correlation ID?** [ASSUMED]
   - What we know: HTTP responses currently use the retry request's metadata correlation ID via `with_correlation(correlation_id)`. [VERIFIED: crates/adapter-http/src/commerce.rs]
   - What's unclear: The phase says original committed result, while API-03 separately requires correlation ID. [VERIFIED: .planning/ROADMAP.md; VERIFIED: .planning/REQUIREMENTS.md]
   - Recommendation: Preserve original append/reply fields, but keep retry correlation in adapter response unless the user explicitly wants stored original correlation. [VERIFIED: inference from adapter code]

2. **Should domain errors be stored for idempotency?** [ASSUMED]
   - What we know: Current storage dedupe records only successful append outcomes because `AppendRequest` rejects empty events and command dedupe is inserted after events. [VERIFIED: crates/es-store-postgres/src/models.rs; VERIFIED: crates/es-store-postgres/src/sql.rs]
   - What's unclear: The phase phrase "success/error reply shape" could mean replaying committed success only, or also replaying previously returned domain errors. [VERIFIED: user prompt]
   - Recommendation: Scope Phase 8 to committed success duplicates unless a prior durable error record exists; do not persist domain validation failures without an explicit decision because they are not event-store commits. [VERIFIED: inference from source-of-truth constraints]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain | Compile/runtime tests | yes | rustc 1.85.1 | None needed. [VERIFIED: command output] |
| Cargo | Rust tests | yes | cargo 1.85.1 | None needed. [VERIFIED: command output] |
| Docker | PostgreSQL Testcontainers tests | yes | Docker 29.3.1 | If unavailable in CI, run runtime and adapter unit tests and mark PostgreSQL integration tests blocked. [VERIFIED: command output] |
| cargo-nextest | Optional faster test runner | no | - | Use `cargo test`, already used by repository plans. [VERIFIED: command output; VERIFIED: .planning/phases/07-adapters-observability-stress-and-template-guidance/07-RESEARCH.md] |

**Missing dependencies with no fallback:** None identified for the phase. [VERIFIED: environment audit]

**Missing dependencies with fallback:** `cargo-nextest` is not installed; use `cargo test`. [VERIFIED: command output]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Cargo test with Tokio async tests; SQLx/Testcontainers for PostgreSQL integration. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/tests/dedupe.rs] |
| Config file | Root `Cargo.toml` workspace and crate manifests. [VERIFIED: Cargo.toml] |
| Quick run command | `cargo test -p es-runtime duplicate -- --nocapture` [VERIFIED: existing test package compiles] |
| Full suite command | `cargo test --workspace` [VERIFIED: project convention in prior phase research and docs] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| STORE-03 | Durable duplicate lookup returns original committed duplicate response without appending. [VERIFIED: .planning/REQUIREMENTS.md] | PostgreSQL integration | `cargo test -p es-store-postgres --test dedupe duplicate_ -- --test-threads=1 --nocapture` | Exists, needs Phase 8 assertions. [VERIFIED: crates/es-store-postgres/tests/dedupe.rs] |
| RUNTIME-03 | Shard-local dedupe is checked before rehydration/decide and without global business-state locks. [VERIFIED: .planning/REQUIREMENTS.md] | Runtime unit/integration | `cargo test -p es-runtime duplicate -- --nocapture` | Exists, needs new failing regression. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| RUNTIME-05 | Duplicate replies are emitted only from prior committed result and not before commit. [VERIFIED: .planning/REQUIREMENTS.md] | Runtime async test | `cargo test -p es-runtime reply_is_sent_after_append_commit duplicate -- --nocapture` | Existing commit-gate test plus new duplicate test needed. [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] |
| INT-04 | Process-manager replay after follow-up success but before offset advancement returns duplicate success. [VERIFIED: .planning/REQUIREMENTS.md] | App/process-manager integration | `cargo test -p app process_manager_duplicate -- --nocapture` | Not yet; Wave 0 gap. [VERIFIED: crates/app/src/commerce_process_manager.rs] |
| API-01 | HTTP duplicate retry flows through bounded gateway and awaits runtime duplicate reply. [VERIFIED: .planning/REQUIREMENTS.md] | Adapter async test | `cargo test -p adapter-http duplicate -- --nocapture` | Test file exists; duplicate test needed. [VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| API-03 | HTTP duplicate response preserves durable positions and typed reply DTO. [VERIFIED: .planning/REQUIREMENTS.md] | Adapter/possibly app integration | `cargo test -p adapter-http duplicate -- --nocapture` | Test file exists; duplicate response assertion needed. [VERIFIED: crates/adapter-http/tests/commerce_api.rs] |

### Sampling Rate

- **Per task commit:** Run the smallest crate command that covers the edited boundary, usually `cargo test -p es-runtime duplicate -- --nocapture` or `cargo test -p es-store-postgres --test dedupe duplicate_ -- --test-threads=1 --nocapture`. [VERIFIED: test infrastructure]
- **Per wave merge:** Run `cargo test -p es-runtime -p es-store-postgres -p adapter-http -p app --no-run` plus relevant duplicate tests. [VERIFIED: command output]
- **Phase gate:** Run `cargo test --workspace` before `/gsd-verify-work`. [VERIFIED: project convention]

### Wave 0 Gaps

- [ ] `crates/es-runtime/tests/runtime_flow.rs` - add a duplicate regression that fails today by proving a second same-key command against mutated cached state must not call `decide`, must not call `load_rehydration`, and must not append. [VERIFIED: current test file]
- [ ] `crates/es-store-postgres/tests/dedupe.rs` - add durable duplicate response payload shape tests for append metadata plus typed reply payload or wrapper JSON. [VERIFIED: current test file]
- [ ] `crates/adapter-http/tests/commerce_api.rs` - add duplicate HTTP retry response contract preserving original durable append fields and typed reply DTO. [VERIFIED: current test file]
- [ ] `crates/app` process-manager test - add crash/retry simulation where follow-up commands already committed and offset has not advanced. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase does not introduce authentication. [VERIFIED: phase scope] |
| V3 Session Management | no | Phase does not introduce sessions. [VERIFIED: phase scope] |
| V4 Access Control | yes | Tenant-scoped dedupe keys and storage predicates must use `TenantId`. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: migrations/20260417000000_event_store.sql] |
| V5 Input Validation | yes | Existing `CommandEnvelope` and `AppendRequest` reject empty idempotency keys; preserve that behavior. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/models.rs] |
| V6 Cryptography | no | Phase does not introduce cryptographic primitives. [VERIFIED: phase scope] |

### Known Threat Patterns for Rust Runtime Idempotency

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant replay collision | Information Disclosure / Tampering | Scope every lookup and primary key by `(tenant_id, idempotency_key)`. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: migrations/20260417000000_event_store.sql] |
| Replay result substitution | Tampering | Persist and decode the stored original response payload; do not recompute reply from mutated state. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: inference from code audit] |
| Idempotency key leakage in metrics | Information Disclosure | Use bounded metric labels only; keep idempotency key out of metric labels. [VERIFIED: crates/app/src/observability.rs] |
| Duplicate concurrent append race | Tampering | Keep PostgreSQL advisory lock and primary key conflict handling. [VERIFIED: crates/es-store-postgres/src/sql.rs; CITED: https://www.postgresql.org/docs/current/functions-admin.html] |

## Sources

### Primary (HIGH confidence)

- `AGENTS.md` - project constraints, event-store source of truth, runtime ownership, package-manager rules. [VERIFIED: AGENTS.md]
- `.planning/ROADMAP.md` - Phase 8 goal, success criteria, requirement mapping. [VERIFIED: .planning/ROADMAP.md]
- `.planning/REQUIREMENTS.md` - STORE-03, RUNTIME-03, RUNTIME-05, INT-04, API-01, API-03 definitions. [VERIFIED: .planning/REQUIREMENTS.md]
- `.planning/v1.0-MILESTONE-AUDIT.md` - blocker evidence and affected flows. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- `crates/es-runtime/src/shard.rs` - current runtime processing order and duplicate append branch. [VERIFIED: crates/es-runtime/src/shard.rs]
- `crates/es-runtime/src/cache.rs` - current `DedupeCache` and `DedupeRecord` shape. [VERIFIED: crates/es-runtime/src/cache.rs]
- `crates/es-runtime/src/store.rs` - current runtime store port lacks pre-decision dedupe lookup. [VERIFIED: crates/es-runtime/src/store.rs]
- `crates/es-store-postgres/src/sql.rs` and `migrations/20260417000000_event_store.sql` - durable dedupe implementation. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: migrations/20260417000000_event_store.sql]
- `crates/adapter-http/src/commerce.rs` and tests - HTTP response mapping and gateway submission. [VERIFIED: crates/adapter-http/src/commerce.rs; VERIFIED: crates/adapter-http/tests/commerce_api.rs]
- `crates/app/src/commerce_process_manager.rs` and `crates/es-outbox/src/process_manager.rs` - deterministic follow-up idempotency keys and offset advancement order. [VERIFIED: crates/app/src/commerce_process_manager.rs; VERIFIED: crates/es-outbox/src/process_manager.rs]
- PostgreSQL current documentation - advisory lock functions and insert conflict behavior. [CITED: https://www.postgresql.org/docs/current/functions-admin.html; CITED: https://www.postgresql.org/docs/current/sql-insert.html]

### Secondary (MEDIUM confidence)

- Stripe idempotent requests documentation - common API idempotency behavior of returning the same saved result for a key. [CITED: https://docs.stripe.com/api/idempotent_requests]

### Tertiary (LOW confidence)

- None used. [VERIFIED: research process]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new packages needed and existing versions/manifests were verified locally. [VERIFIED: Cargo.toml; VERIFIED: command output]
- Architecture: HIGH for pre-decision ordering and tier boundaries because the audited code path is explicit. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md; VERIFIED: crates/es-runtime/src/shard.rs]
- Durable typed reply shape: MEDIUM - the need is clear from current code, but exact schema and backward compatibility require a planning decision. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/sql.rs; ASSUMED]
- Pitfalls: HIGH - each pitfall maps to a current code path or missing test assertion. [VERIFIED: code audit]

**Research date:** 2026-04-19 [VERIFIED: environment date]
**Valid until:** 2026-05-19 for codebase-local findings; revalidate if Phase 8 is delayed past major storage/runtime refactors. [ASSUMED]

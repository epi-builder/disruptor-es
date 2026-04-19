---
phase: 07-adapters-observability-stress-and-template-guidance
phase_number: 07
status: secured
asvs_level: 1
block_on: high
threats_total: 23
threats_closed: 23
threats_open: 0
verified_at: 2026-04-19
auditor: gsd-security-auditor
---

# Phase 07 Security Verification

## Scope

Verified declared Phase 07 threat mitigations from plans 07-01 through 07-07 against implemented code, tests, benchmark harnesses, and documentation. This audit did not scan for new vulnerabilities outside the threat register.

Implementation files were read only. This document is the only file written by the security audit.

## Trust Boundaries

| Boundary | Source Plans | Verification Focus |
|---|---|---|
| HTTP client -> adapter DTOs | 07-01 | Tenant, idempotency, metadata, and domain IDs are constructed through typed constructors before bounded runtime submission. |
| Adapter -> CommandGateway | 07-01 | Adapter owns no hot business state and submits through nonblocking `CommandGateway::try_submit`. |
| Runtime/storage -> telemetry backend | 07-02, 07-07 | High-cardinality identity fields are trace fields, while metrics use bounded labels and durable lag sources. |
| Test harness -> PostgreSQL | 07-03, 07-07 | PostgreSQL tests exercise public repositories, OCC, dedupe, tenant scope, projection lag, and outbox paths. |
| Bench harness -> database/results users | 07-04 | Bench scenarios are layer-separated and storage benchmarks require explicit database configuration. |
| CLI/stress load -> runtime/report users | 07-05, 07-07 | Stress uses bounded ingress, records rejects, and reports lag separately from command success. |
| Documentation -> future implementers | 07-06 | Docs include grep-verifiable forbidden patterns, gateway rules, and stress interpretation rules. |

## Threat Register

| Threat ID | Category | Component | Disposition | Status | Evidence |
|---|---|---|---|---|---|
| T-07-01 | Spoofing | `PlaceOrderRequest.tenant_id` and metadata DTOs | mitigate | CLOSED | `CommandRequestMetadata` carries `tenant_id` and builds `CommandMetadata` with `TenantId::new` in `crates/adapter-http/src/commerce.rs:48` and `crates/adapter-http/src/commerce.rs:509`; tests assert the routed envelope metadata tenant in `crates/adapter-http/tests/commerce_api.rs:60`. |
| T-07-02 | Tampering | idempotency key and command DTOs | mitigate | CLOSED | Domain IDs are built through `OrderId::new`, `UserId::new`, `ProductId::new`, `Sku::new`, and `Quantity::new` before submission in `crates/adapter-http/src/commerce.rs:236`, `crates/adapter-http/src/commerce.rs:321`, and `crates/adapter-http/src/commerce.rs:408`; empty idempotency keys are rejected by `CommandEnvelope::new` in `crates/es-runtime/src/command.rs:34`. |
| T-07-03 | Denial of Service | Tower and `CommandGateway::try_submit` ingress | mitigate | CLOSED | `submit_command` calls nonblocking `gateway.try_submit(envelope)` in `crates/adapter-http/src/commerce.rs:492`; `CommandGateway` uses bounded `mpsc::channel(ingress_capacity)` and `try_send` in `crates/es-runtime/src/gateway.rs:41` and `crates/es-runtime/src/gateway.rs:64`; overload maps to HTTP 429 in `crates/adapter-http/src/error.rs:89`. |
| T-07-04 | Elevation of Privilege | adapter direct state mutation | mitigate | CLOSED | Boundary tests forbid storage/projection/outbox dependencies and hot-state mutation markers in `crates/adapter-http/tests/dependency_boundaries.rs:7` and `crates/adapter-http/tests/dependency_boundaries.rs:19`; adapter state contains only gateways in `crates/adapter-http/src/commerce.rs:17`. |
| T-07-05 | Information Disclosure | metric labels | mitigate | CLOSED | `FORBIDDEN_METRIC_LABELS` lists tenant, command, correlation, causation, stream, event, and idempotency labels in `crates/app/src/observability.rs:55`; the bounded-label unit test verifies the catalog in `crates/app/src/observability.rs:167`. |
| T-07-06 | Denial of Service | metrics series cardinality | mitigate | CLOSED | Allowed labels are limited to `aggregate`, `outcome`, `reason`, `shard`, `projector`, and `topic` in `crates/app/src/observability.rs:66`; runtime metrics use bounded labels such as `aggregate/outcome/reason` in `crates/es-runtime/src/gateway.rs:62`. |
| T-07-07 | Repudiation | command tracing | mitigate | CLOSED | Adapter, gateway, engine, shard, and event-store spans include command/correlation/causation IDs as trace fields in `crates/adapter-http/src/commerce.rs:480`, `crates/es-runtime/src/gateway.rs:47`, `crates/es-runtime/src/engine.rs:107`, `crates/es-runtime/src/shard.rs:159`, and `crates/es-store-postgres/src/event_store.rs:32`. |
| T-07-08 | Tampering | append/OCC/dedupe tests | mitigate | CLOSED | Wrong expected revision is asserted as a `StoreError::StreamConflict` in `crates/es-store-postgres/tests/phase7_integration.rs:310`; duplicate idempotency returns the original committed result in `crates/es-store-postgres/tests/phase7_integration.rs:437`. |
| T-07-09 | Information Disclosure | tenant-scoped reads | mitigate | CLOSED | Test helpers require explicit `TenantId` for append, snapshot, projection, and outbox flows in `crates/es-store-postgres/tests/phase7_integration.rs:26`, `crates/es-store-postgres/tests/phase7_integration.rs:56`, `crates/es-store-postgres/tests/phase7_integration.rs:74`, and `crates/es-store-postgres/tests/phase7_integration.rs:513`; store APIs bind tenant-scoped reads in `crates/es-store-postgres/src/event_store.rs:89`. |
| T-07-10 | Tampering | SQL query paths | mitigate | CLOSED | Phase 7 integration coverage uses `PostgresEventStore`, `PostgresProjectionStore`, `PostgresOutboxStore`, and `dispatch_once` public APIs in `crates/es-store-postgres/tests/phase7_integration.rs:517`; tenant max-position SQL is static and bound with `.bind(tenant.as_str())` in `crates/es-store-postgres/tests/phase7_integration.rs:187`. |
| T-07-11 | Information Disclosure | benchmark output | mitigate | CLOSED | Storage bench prints only the generic `storage_only requires DATABASE_URL` message and does not print the URL in `benches/storage_only.rs:20`; bench comments and Criterion names do not print tenant, command, or event IDs in `benches/ring_only.rs:1`, `benches/domain_only.rs:1`, `benches/adapter_only.rs:1`, `benches/projector_outbox.rs:1`. |
| T-07-12 | Tampering | benchmark scenario interpretation | mitigate | CLOSED | Scenario comments explicitly separate ring-only, domain-only, adapter-only, storage-only, and projector/outbox behavior in `benches/ring_only.rs:1`, `benches/domain_only.rs:1`, `benches/adapter_only.rs:1`, `benches/storage_only.rs:1`, and `benches/projector_outbox.rs:1`; stress docs separate ring-only from integrated stress in `docs/stress-results.md:5`. |
| T-07-13 | Denial of Service | storage benchmark database | accept | CLOSED | Accepted risk documented below. The bench requires explicit `DATABASE_URL`, emits no fallback discovery, and exits early if absent in `benches/storage_only.rs:20` and `benches/storage_only.rs:30`. |
| T-07-14 | Denial of Service | stress runner ingress | mitigate | CLOSED | Smoke configs use bounded `ingress_capacity` and `ring_size` in `crates/app/src/stress.rs:72`; stress submission uses `try_submit` and increments `commands_rejected` on overload in `crates/app/src/stress.rs:265`. |
| T-07-15 | Repudiation | stress report | mitigate | CLOSED | `StressReport` includes `commands_submitted`, `commands_succeeded`, `commands_rejected`, and `reject_rate` in `crates/app/src/stress.rs:126`; the report populates those fields in `crates/app/src/stress.rs:330`. |
| T-07-16 | Tampering | command success interpretation | mitigate | CLOSED | Command replies are counted first in `crates/app/src/stress.rs:284`; projection and outbox lag are sampled afterward in `crates/app/src/stress.rs:308`; docs state lag is not command success in `docs/stress-results.md:51`. |
| T-07-17 | Tampering | architecture guidance | mitigate | CLOSED | Exact forbidden patterns and required checks are documented in `docs/hot-path-rules.md:37` and `docs/hot-path-rules.md:45`. |
| T-07-18 | Information Disclosure | gateway guidance | mitigate | CLOSED | Template guidance directs WebSocket/gRPC gateways to `CommandGateway` plus read-model query APIs and forbids shared hot aggregate state in `docs/template-guide.md:43` and `docs/template-guide.md:51`; HTTP guidance returns typed response DTOs in `docs/template-guide.md:37`. |
| T-07-19 | Repudiation | stress interpretation | mitigate | CLOSED | Docs distinguish ring-only handoff cost from single-service integrated stress in `docs/stress-results.md:5` and `docs/stress-results.md:24`. |
| T-07-20 | Tampering | `es_projection_lag` gauge | mitigate | CLOSED | Projection lag uses tenant-scoped durable `SELECT COALESCE(max(global_position), 0) FROM events WHERE tenant_id = $1` and subtracts current/applied projector offsets in `crates/es-store-postgres/src/projection.rs:88`, `crates/es-store-postgres/src/projection.rs:96`, `crates/es-store-postgres/src/projection.rs:124`, and `crates/es-store-postgres/src/projection.rs:189`; tenant-isolation backlog test covers tenant A plus tenant B noise in `crates/es-store-postgres/tests/phase7_integration.rs:337`. |
| T-07-21 | Repudiation | `StressReport.append_latency_p95_micros` | mitigate | CLOSED | `MeasuredRuntimeEventStore` records duration around `inner.append(request).await` in `crates/app/src/stress.rs:170` and `crates/app/src/stress.rs:189`; the report builds append latency from those durations in `crates/app/src/stress.rs:300` and writes `append_latency_p95_micros` in `crates/app/src/stress.rs:342`. |
| T-07-22 | Information Disclosure | stress/projection SQL | mitigate | CLOSED | Projection/stress max-position SQL binds tenant IDs in `crates/es-store-postgres/src/projection.rs:193` and `crates/app/src/stress.rs:452`; `StressReport` fields are numeric scenario metrics only in `crates/app/src/stress.rs:126` and do not include raw tenant IDs, command IDs, event IDs, or database URLs. |
| T-07-23 | Denial of Service | stress queue sampling | accept | CLOSED | Accepted risk documented below. The implementation samples bounded in-memory queue lengths read-only through `CommandEngine::shard_depths()` in `crates/es-runtime/src/engine.rs:89` and uses that sample in `crates/app/src/stress.rs:252`. |

## Accepted Risks Log

| Threat ID | Risk | Rationale | Evidence | Owner | Review Trigger |
|---|---|---|---|---|---|
| T-07-13 | Storage benchmark can place load on the database named by `DATABASE_URL`. | Accepted because the benchmark requires an explicit operator-provided `DATABASE_URL`, performs no production target discovery, and prints no URL. This keeps the risk local to intentional benchmark execution. | `benches/storage_only.rs:20`, `benches/storage_only.rs:30` | Template maintainers | Revisit if benchmarks gain auto-discovery, config-file database loading, or CI production credentials. |
| T-07-23 | Stress queue sampling may observe runtime queue lengths during load. | Accepted because `shard_depths()` is read-only, samples existing bounded in-memory queues, returns owned `Vec<usize>`, and does not add blocking synchronization to the hot path. | `crates/es-runtime/src/engine.rs:89`, `crates/app/src/stress.rs:252` | Template maintainers | Revisit if sampling becomes cross-thread, lock-based, exported at high frequency, or exposed as an external unauthenticated endpoint. |

## Unregistered Flags

None. All `## Threat Flags` sections in Phase 07 summaries report `None`, and no SUMMARY threat flag required separate unregistered logging.

## Audit Trail

| Date | Action | Result |
|---|---|---|
| 2026-04-19 | Loaded required Phase 07 plans, summaries, verification, review, implementation files, docs, benchmarks, and `AGENTS.md`. | Completed before threat classification. |
| 2026-04-19 | Checked project-local skill directories `.claude/skills/` and `.agents/skills/`. | No project-local skills found. |
| 2026-04-19 | Verified each threat in the provided register by declared disposition. | 23/23 closed, 0 open. |
| 2026-04-19 | Incorporated SUMMARY.md threat flags. | No unregistered flags. |
| 2026-04-19 | Wrote Phase 07 security report. | `threats_open: 0`. |

## Sign-Off

Security status: secured.

ASVS Level: 1.

Threats closed: 23/23.

Threats open: 0.

Implementation files modified: none.

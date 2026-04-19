---
phase: 07-adapters-observability-stress-and-template-guidance
verified: 2026-04-19T02:18:29Z
status: passed
score: 11/11 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 9/11
  gaps_closed:
    - "Structured traces and metrics expose trustworthy projection lag and load signals under Phase 7 observability."
    - "A single-service integrated stress test reports real throughput, latency, queue depth, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization."
  gaps_remaining: []
  regressions: []
---

# Phase 7: Adapters, Observability, Stress, and Template Guidance Verification Report

**Phase Goal:** The template is usable from thin HTTP boundaries, observable under load, verified against real storage paths, benchmarked by layer, and documented with the rules that keep the architecture correct.
**Verified:** 2026-04-19T02:18:29Z
**Status:** passed
**Re-verification:** Yes - after gap-closure Plan 07-07

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | HTTP command endpoints decode requests, attach metadata, submit through bounded ingress, and return stream revision, global position, correlation ID, and typed payloads. | VERIFIED | Quick regression check: `crates/adapter-http/src/commerce.rs` still contains `CommandEnvelope::<A>::new`, `CommandGateway::try_submit`, one-shot reply handling, trace fields, and `CommandSuccess` durable append fields. |
| 2 | Adapter code does not mutate aggregate, projector, event-store, or outbox state directly. | VERIFIED | Quick regression check: `crates/adapter-http/tests/dependency_boundaries.rs` still forbids storage/projection/outbox dependencies and direct mutation markers; adapter source only submits through gateways. |
| 3 | Structured traces and metrics expose command identity, shard/global positions, queue depth, ring wait, decision latency, append latency, conflicts, dedupe hits, projection lag, outbox lag, and command latency. | VERIFIED | `crates/app/src/observability.rs` catalogs required metrics; runtime/storage/outbox paths emit bounded labels. The prior projection-lag gap is closed: `crates/es-store-postgres/src/projection.rs:88`, `:96`, `:124`, and `:189` compute `es_projection_lag` from tenant-scoped durable max event-store position and current/applied projector offset. |
| 4 | Integration tests verify append, conflicts, dedupe, snapshots, projector checkpoints, and outbox dispatch against real/containerized PostgreSQL. | VERIFIED | `crates/es-store-postgres/tests/phase7_integration.rs` uses `common::start_postgres`, `PostgresEventStore`, `PostgresProjectionStore`, `PostgresOutboxStore`, and `dispatch_once`; the targeted projection backlog regression passed. |
| 5 | Benchmark artifacts separate ring-only, domain-only, adapter-only, storage-only, single-service/full-E2E, projector/outbox, hot-key, burst, and degraded-dependency behavior. | VERIFIED | Quick regression check: root `benches/*.rs` remain split by layer and stress scenarios still include single-service, full E2E in process, hot-key, burst, and degraded dependency variants in `crates/app/src/stress.rs`. |
| 6 | Single-service integrated stress exercises production-shaped composition and reports real throughput, latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization. | VERIFIED | Prior stress-report gap is closed: `MeasuredRuntimeEventStore` records append duration around `inner.append(request)`, `CommandEngine::shard_depths()` samples real shard queues, and `sample_projection_lag` returns max tenant backlog after bounded catch-up instead of constant zero. Targeted app tests passed. |
| 7 | Documentation states hot-path rules, service-boundary guidance, and stress interpretation separate from ring-only microbenchmarks. | VERIFIED | Quick regression check: `docs/hot-path-rules.md`, `docs/template-guide.md`, and `docs/stress-results.md` still contain event-store source-of-truth, `CommandGateway`, outbox, gateway, and ring-only guidance. |
| 8 | Metric labels are bounded and exclude raw tenant IDs, command IDs, stream IDs, event IDs, and idempotency keys. | VERIFIED | `FORBIDDEN_METRIC_LABELS` remains present in `crates/app/src/observability.rs`; identity fields appear as trace/span fields while metric labels use bounded names such as `aggregate`, `outcome`, `reason`, `shard`, `projector`, and `topic`. |
| 9 | PostgreSQL integration tests use existing Testcontainers and SQLx migrations rather than mocks or SQLite. | VERIFIED | `phase7_integration.rs` uses the existing PostgreSQL harness and public PostgreSQL store APIs; no mock or SQLite path is used for Phase 7 storage verification. |
| 10 | Ring-only benchmark output cannot be mistaken for service throughput. | VERIFIED | Quick regression check: `benches/ring_only.rs` remains isolated from domain/adapter/storage/projection/outbox service paths, and docs state ring-only benchmarks measure disruptor handoff cost rather than service throughput. |
| 11 | Projection and outbox lag are reported outside command success gates. | VERIFIED | `crates/app/src/stress.rs:284-317` counts command replies first, then samples projection and outbox lag after command success accounting. |

**Score:** 11/11 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/adapter-http/src/commerce.rs` | Axum commerce command DTOs and handlers | VERIFIED | Routes, metadata construction, gateway submission, trace fields, and durable success DTOs remain present. |
| `crates/adapter-http/src/error.rs` | Typed API error mapping | VERIFIED | JSON `IntoResponse` contract remains present for runtime/domain/store error mapping. |
| `crates/adapter-http/tests/dependency_boundaries.rs` | Adapter boundary regression tests | VERIFIED | Forbidden dependency and source marker checks remain present. |
| `crates/app/src/observability.rs` | Observability bootstrap and metric catalog | VERIFIED | Required metric names and forbidden-label tests remain present. |
| `crates/es-store-postgres/src/projection.rs` | Tenant durable max-position projection lag computation | VERIFIED | `tenant_latest_global_position` queries `SELECT COALESCE(max(global_position), 0) FROM events WHERE tenant_id = $1`; idle and applied paths set lag from durable latest minus offset. |
| `crates/es-store-postgres/tests/phase7_integration.rs` | Backlog-sized projection lag integration coverage | VERIFIED | `phase7_projection_lag_uses_tenant_durable_backlog_not_batch_size` appends 250 tenant A events plus tenant B noise and asserts observed gauge equals durable backlog. |
| `crates/es-runtime/src/engine.rs` | Read-only shard depth sampling | VERIFIED | `CommandEngine::shard_depths()` returns `pending_len() + pending_handoffs()` per shard without exposing mutable shard internals. |
| `crates/app/src/stress.rs` | Measured append latency, shard depth, and projection lag in `StressReport` | VERIFIED | Append latency comes from `MeasuredRuntimeEventStore`, shard depth from `engine.shard_depths()`, and projection lag from tenant max global position minus post-catch-up offset. |
| `crates/app/src/main.rs` | Thin stress bootstrap | VERIFIED | `stress-smoke` remains a thin call into app stress library code. |
| `benches/ring_only.rs` | Ring-only benchmark | VERIFIED | Ring-only benchmark remains isolated to disruptor path behavior. |
| `benches/domain_only.rs` | Domain-only benchmark | VERIFIED | Domain benchmark remains in-memory decide/apply only. |
| `benches/adapter_only.rs` | Adapter-only benchmark | VERIFIED | Adapter benchmark uses DTO/envelope/gateway admission only. |
| `benches/storage_only.rs` | Storage-only benchmark | VERIFIED | Storage benchmark uses PostgreSQL event-store paths and requires `DATABASE_URL` for measurement. |
| `benches/projector_outbox.rs` | Projector/outbox benchmark | VERIFIED | Projector catch-up and outbox dispatch benchmarks use PostgreSQL/Testcontainers paths. |
| `docs/hot-path-rules.md` | Hot-path rules | VERIFIED | Source-of-truth, single-owner, gateway, outbox, and forbidden-pattern rules remain present. |
| `docs/template-guide.md` | Template extension guide | VERIFIED | HTTP/WebSocket/gRPC gateway guidance remains tied to `CommandGateway` and query APIs. |
| `docs/stress-results.md` | Stress interpretation guide | VERIFIED | Required stress fields and ring-only separation guidance remain present. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `crates/adapter-http/src/commerce.rs` | `CommandGateway::try_submit` | `CommandEnvelope::<A>::new` then gateway submission | VERIFIED | Quick regression grep found envelope creation and gateway submission. |
| `crates/adapter-http/src/error.rs` | `axum::response::IntoResponse` | Runtime error to JSON status mapping | VERIFIED | Previously passed link remains unchanged by 07-07 scope. |
| `crates/es-runtime/src/shard.rs` | Metric catalog | Runtime metric emission | VERIFIED | Required runtime metric names remain emitted and cataloged. |
| `crates/es-store-postgres/src/projection.rs` | `events.global_position` | `tenant_latest_global_position` query before lag gauge update | VERIFIED | Manual check confirms tenant-scoped `max(global_position)` SQL and lag gauge updates at idle/applied paths. |
| `crates/es-store-postgres/tests/phase7_integration.rs` | PostgreSQL harness | `common::start_postgres` | VERIFIED | Tests use the real containerized PostgreSQL harness. |
| `crates/app/src/stress.rs` | `crates/es-runtime/src/engine.rs` | `run_single_service_stress` samples `CommandEngine::shard_depths()` | VERIFIED | `engine.shard_depths()` is sampled before/after processing and assigned to `StressReport.shard_depth_max`. |
| `crates/app/src/stress.rs` | `RuntimeEventStore::append` | `MeasuredRuntimeEventStore` records append duration around `inner.append` | VERIFIED | The wrapper implements `RuntimeEventStore` and records successful append durations into the append latency histogram. |
| `docs/template-guide.md` | HTTP adapter pattern | `CommandGateway` guidance | VERIFIED | Docs retain thin gateway guidance. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `crates/es-store-postgres/src/projection.rs` | `es_projection_lag` | Tenant-scoped durable `events.max(global_position)` minus current/applied projector offset | Yes | VERIFIED |
| `crates/es-store-postgres/tests/phase7_integration.rs` | Observed projection lag gauge | Local `metrics::Recorder` captures `es_projection_lag` for `phase7-commerce-read-models` | Yes | VERIFIED |
| `crates/es-runtime/src/engine.rs` | `shard_depths` | Per-shard accepted queue length plus processable handoffs | Yes | VERIFIED |
| `crates/app/src/stress.rs` | `append_latency_p95_micros` | Durations measured inside `MeasuredRuntimeEventStore::append` around real `inner.append` | Yes | VERIFIED |
| `crates/app/src/stress.rs` | `shard_depth_max` | Max sampled from `engine.shard_depths()` during submit/process/reply collection | Yes | VERIFIED |
| `crates/app/src/stress.rs` | `projection_lag` | Tenant durable max global position minus projector offset after bounded catch-up | Yes | VERIFIED |
| `crates/app/src/stress.rs` | `outbox_lag` | Pending durable outbox rows after dispatch sampling | Yes | VERIFIED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Projection lag uses durable tenant backlog, not current batch size or idle zeroing | `cargo test -p es-store-postgres phase7_projection_lag_uses_tenant_durable_backlog_not_batch_size -- --nocapture` | 1 passed; test completed successfully | PASS |
| Stress projection lag is not always zero with controlled durable backlog | `cargo test -p app stress_projection_lag_reports_controlled_backlog -- --nocapture` | 1 passed; test completed successfully | PASS |
| Single-service stress smoke still reports required fields after measured-signal changes | `cargo test -p app single_service_stress_smoke -- --nocapture` | 1 passed; test completed successfully | PASS |
| Gap-closure artifacts exist and are substantive | `gsd-tools verify artifacts 07-07-PLAN.md` | 4/4 artifacts passed | PASS |
| Gap-closure key links | `gsd-tools verify key-links 07-07-PLAN.md` plus manual grep | Tool found one link and missed two due regex/pattern parsing; manual grep verified all three links | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| API-01 | 07-01 | Thin HTTP adapter decodes requests, attaches metadata, sends through bounded ingress, and awaits replies. | SATISFIED | Commerce handlers build metadata/envelopes and call `try_submit`. |
| API-02 | 07-01 | Adapter does not mutate aggregate, projector, or outbox state directly. | SATISFIED | Boundary tests and manifest checks forbid direct dependencies and mutation markers. |
| API-03 | 07-01 | API responses include stream revision, global position, correlation ID, and typed payloads. | SATISFIED | `CommandSuccess` carries durable append metadata and typed replies. |
| API-04 | 07-06 | Docs explain WebSocket/gRPC gateways without shared hot state. | SATISFIED | `docs/template-guide.md` says WebSocket and gRPC gateways are thin `CommandGateway` plus query clients. |
| OBS-01 | 07-02 | Runtime emits structured traces with command, correlation, causation, tenant, stream, shard, and global position fields. | SATISFIED | Adapter, gateway, engine, shard, and event-store append spans carry required fields where available. |
| OBS-02 | 07-02, 07-05, 07-07 | Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, conflicts, dedupe hits, projection lag, outbox lag, p95/p99 command latency. | SATISFIED | Metric catalog and instrumentation exist; 07-07 fixed projection lag and stress signal data sources. |
| TEST-02 | 07-03, 07-07 | Integration tests verify append, conflicts, dedupe, snapshots, projector checkpoints, outbox dispatch, and projection lag against PostgreSQL. | SATISFIED | `phase7_integration.rs` includes containerized PostgreSQL coverage and the backlog-sized projection lag regression. |
| TEST-03 | 07-04, 07-05 | Benchmark harnesses separately measure ring, domain, adapter, storage, single-service/full-E2E, projector/outbox, hot-key, burst, degraded dependency. | SATISFIED | Five bench files plus `StressScenario` variants cover the required split. |
| TEST-04 | 07-05, 07-07 | Single-service stress reports throughput, latency, queue depths, append latency, projection/outbox lag, reject rate, CPU/core utilization. | SATISFIED | `StressReport` fields are populated from production-shaped composition, measured append calls, runtime shard depth, and durable lag sampling. |
| DOC-01 | 07-06 | Docs state hot-path rules, forbidden patterns, service-boundary guidance, and new service guidance. | SATISFIED | Three docs contain the required architecture and extension guidance. |

No Phase 7 requirement IDs from `.planning/REQUIREMENTS.md` were orphaned from plan frontmatter.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| None | - | - | - | No blocker anti-patterns remain in the 07-07 verification scope. The only `Ok(0)` matches in `crates/app/src/stress.rs` are legitimate no-events early returns for projection/outbox lag sampling. |

### Human Verification Required

None. This phase is code, tests, benchmarks, and documentation; the decisive gaps are covered by automated regression tests and code/data-flow inspection.

### Gaps Summary

No blocking gaps remain. Plan 07-07 closes both prior verifier gaps:

- `es_projection_lag` now uses tenant-scoped durable event-store maximum global position in both idle and applied catch-up paths, with PostgreSQL regression coverage proving backlog-sized lag is reported.
- Single-service stress append latency, shard depth, and projection lag now come from measured runtime/store/projection sources instead of command round-trip timing, ingress-derived proxies, or constant zero.

There are no later milestone phases to defer unresolved Phase 7 items to; the roadmap marks Phase 7 as the final completed phase.

---

_Verified: 2026-04-19T02:18:29Z_
_Verifier: Claude (gsd-verifier)_

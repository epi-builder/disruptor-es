---
phase: 07-adapters-observability-stress-and-template-guidance
verified: 2026-04-18T15:24:23Z
status: gaps_found
score: 9/11 must-haves verified
overrides_applied: 0
gaps:
  - truth: "Structured traces and metrics expose trustworthy projection lag and load signals under Phase 7 observability."
    status: partial
    reason: "Metric names and trace fields exist, but projection lag is computed from only the current batch and reports zero on idle without checking tenant durable max global position, so backlog can be underreported."
    artifacts:
      - path: "crates/es-store-postgres/src/projection.rs"
        issue: "es_projection_lag is set to last_global_position - current_offset for the fetched batch, and idle catch-up unconditionally sets zero."
    missing:
      - "Compute projection lag against the tenant's durable max event-store global position after catch-up and in idle paths."
      - "Add a backlog-sized integration test proving es_projection_lag reports remaining lag, not just current batch size."
  - truth: "A single-service integrated stress test reports real throughput, latency, queue depth, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization."
    status: failed
    reason: "The stress runner composes the production-shaped path, but several report fields are synthetic or wrong: append latency records full command round-trip latency, shard depth is derived from ingress depth, and projection lag always returns zero."
    artifacts:
      - path: "crates/app/src/stress.rs"
        issue: "append_latency.record(elapsed) uses command round-trip elapsed time; shard_depth_max is ingress_depth_max.min(ring_size); sample_projection_lag computes local lag variables but returns Ok(0)."
    missing:
      - "Populate append_latency_p95_micros from event-store append timing or rename/remove the field."
      - "Populate shard_depth_max from real shard/runtime queue depth rather than an ingress-derived proxy."
      - "Return measured projection lag from committed global position minus projector offsets after catch-up."
      - "Add a controlled backlog stress test that fails if projection_lag is always zero."
---

# Phase 7: Adapters, Observability, Stress, and Template Guidance Verification Report

**Phase Goal:** The template is usable from thin HTTP boundaries, observable under load, verified against real storage paths, benchmarked by layer, and documented with the rules that keep the architecture correct.
**Verified:** 2026-04-18T15:24:23Z
**Status:** gaps_found
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | HTTP command endpoints decode requests, attach metadata, submit through bounded ingress, and return stream revision, global position, correlation ID, and typed payloads. | VERIFIED | `crates/adapter-http/src/commerce.rs` defines route handlers, DTOs, `CommandEnvelope::<A>::new`, `CommandGateway::try_submit`, one-shot replies, `Uuid::now_v7`, `OffsetDateTime::now_utc`, and `CommandSuccess` durable metadata fields. |
| 2 | Adapter code does not mutate aggregate, projector, event-store, or outbox state directly. | VERIFIED | `crates/adapter-http/tests/dependency_boundaries.rs` forbids storage/projection/outbox dependencies and direct mutation markers; `adapter-http/Cargo.toml` has no forbidden storage/projection/outbox crate dependencies. |
| 3 | Structured traces and metrics expose command identity, shard/global positions, queue depth, ring wait, decision latency, append latency, conflicts, dedupe hits, projection lag, outbox lag, and command latency. | FAILED | Metric and span names exist, but `es_projection_lag` is not a trustworthy backlog metric: `projection.rs:94-109` reports zero when no batch is read and otherwise reports current batch distance, not tenant backlog. |
| 4 | Integration tests verify append, conflicts, dedupe, snapshots, projector checkpoints, and outbox dispatch against real/containerized PostgreSQL. | VERIFIED | `crates/es-store-postgres/tests/phase7_integration.rs` uses `common::start_postgres`, `PostgresEventStore`, `PostgresProjectionStore`, `PostgresOutboxStore`, `dispatch_once`, and four explicit Phase 7 integration tests. |
| 5 | Benchmark artifacts separate ring-only, domain-only, adapter-only, storage-only, single-service/full-E2E, projector/outbox, hot-key, burst, and degraded-dependency behavior. | VERIFIED | Root `benches/*.rs` files separate ring/domain/adapter/storage/projector-outbox scenarios; `StressScenario` covers single-service integrated, full E2E in process, hot-key, burst, and degraded dependency. |
| 6 | Single-service integrated stress exercises production-shaped composition and reports real throughput, p50/p95/p99 latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization. | FAILED | `crates/app/src/stress.rs` composes Postgres/runtime/projection/outbox, but `stress.rs:220-265` records append latency from full command elapsed time, derives shard depth from ingress depth, and `sample_projection_lag` returns zero. |
| 7 | Documentation states hot-path rules, service-boundary guidance, and stress interpretation separate from ring-only microbenchmarks. | VERIFIED | `docs/hot-path-rules.md`, `docs/template-guide.md`, and `docs/stress-results.md` include source-of-truth, single-owner, `CommandGateway`, outbox, WebSocket/gRPC, and ring-only-vs-integrated stress guidance. |
| 8 | Metric labels are bounded and exclude raw tenant IDs, command IDs, stream IDs, event IDs, and idempotency keys. | VERIFIED | `crates/app/src/observability.rs` defines `FORBIDDEN_METRIC_LABELS` and `ALLOWED_METRIC_LABELS`; grep found no high-cardinality identity fields used as metric labels in Phase 7 instrumentation paths. |
| 9 | PostgreSQL integration tests use existing Testcontainers and SQLx migrations rather than mocks or SQLite. | VERIFIED | `phase7_integration.rs` imports `common::start_postgres` and exercises public PostgreSQL store APIs. |
| 10 | Ring-only benchmark output cannot be mistaken for service throughput. | VERIFIED | `benches/ring_only.rs` comments state it measures `DisruptorPath` only and avoids domain/adapter/storage/projection/outbox imports; docs repeat the same warning. |
| 11 | Projection and outbox lag are reported outside command success gates. | VERIFIED | Stress command success is counted from command replies before `sample_projection_lag` and `sample_outbox_lag`; docs explicitly say projection/outbox lag is not command success. |

**Score:** 9/11 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/adapter-http/src/commerce.rs` | Axum commerce command DTOs and handlers | VERIFIED | Routes, DTOs, metadata, gateway submission, spans, and success DTOs present. |
| `crates/adapter-http/src/error.rs` | Typed API error mapping | VERIFIED | `ApiError` implements JSON `IntoResponse` with 429, 409, 400, 503, and 500 mappings. |
| `crates/adapter-http/tests/dependency_boundaries.rs` | Adapter boundary regression tests | VERIFIED | Source and manifest forbidden-pattern tests present. |
| `crates/app/src/observability.rs` | Observability bootstrap and metric catalog | VERIFIED | Config, exporter setup, metric names, forbidden labels, and unit tests present. |
| `crates/app/src/stress.rs` | Single-service stress runner | HOLLOW | Production-shaped composition exists, but several reported fields are synthetic or always zero. |
| `crates/app/src/main.rs` | Thin stress bootstrap | VERIFIED | `stress-smoke` calls library stress runner and prints JSON fields; no runtime/storage internals in main. |
| `benches/ring_only.rs` | Ring-only benchmark | VERIFIED | Uses `DisruptorPath` only. |
| `benches/domain_only.rs` | Domain-only benchmark | VERIFIED | Uses in-memory commerce aggregate decide/apply. |
| `benches/adapter_only.rs` | Adapter-only benchmark | VERIFIED | Uses DTO decode, `CommandEnvelope`, and `CommandGateway::try_submit`; no engine/storage. |
| `benches/storage_only.rs` | Storage-only benchmark | VERIFIED | Requires `DATABASE_URL`; no in-memory fallback. |
| `benches/projector_outbox.rs` | Projector/outbox benchmark | VERIFIED | Uses PostgreSQL 18 Testcontainers, projection catch-up, and outbox dispatch. |
| `crates/es-store-postgres/tests/phase7_integration.rs` | Real PostgreSQL integration tests | VERIFIED | Covers append/OCC, dedupe, snapshots, projection offsets, and outbox dispatch. |
| `docs/hot-path-rules.md` | Hot-path rules | VERIFIED | Required headings and source-of-truth/outbox/forbidden-pattern guidance present. |
| `docs/template-guide.md` | Template extension guide | VERIFIED | New domain, HTTP, WebSocket, gRPC, projection, outbox, and verification guidance present. |
| `docs/stress-results.md` | Stress interpretation guide | VERIFIED | Required report fields and ring-only separation guidance present. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `crates/adapter-http/src/commerce.rs` | `CommandGateway::try_submit` | `CommandEnvelope::<A>::new` then gateway submission | VERIFIED | `gsd-tools verify key-links` passed. |
| `crates/adapter-http/src/error.rs` | `axum::response::IntoResponse` | Runtime error to JSON status mapping | VERIFIED | `impl IntoResponse for ApiError` present. |
| `crates/es-runtime/src/shard.rs` | Metric catalog | `es_command_latency_seconds` and related metric names | VERIFIED | Runtime emits command, queue, ring, and decision metrics. |
| `crates/es-store-postgres/src/projection.rs` | Metrics facade | `es_projection_lag` | PARTIAL | Gauge exists, but its value underreports backlog. |
| `crates/es-store-postgres/tests/phase7_integration.rs` | PostgreSQL harness | `common::start_postgres` | VERIFIED | Testcontainers harness reused. |
| `benches/ring_only.rs` | Disruptor path | `DisruptorPath` only | VERIFIED | Ring-only bench imports runtime path only. |
| `crates/app/src/stress.rs` | Runtime engine | `CommandEngine` and `CommandGateway` | VERIFIED | Stress runner composes and drives the runtime. |
| `docs/template-guide.md` | HTTP adapter pattern | `CommandGateway` guidance | VERIFIED | Docs describe the same thin gateway boundary. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `crates/adapter-http/src/commerce.rs` | `CommandSuccess` durable fields | `CommandOutcome.append` from runtime reply | Yes | VERIFIED |
| `crates/es-store-postgres/tests/phase7_integration.rs` | Global positions, offsets, outbox publication | PostgreSQL append/projection/outbox APIs | Yes | VERIFIED |
| `crates/app/src/stress.rs` | `append_latency_p95_micros` | Full command elapsed time reused as append latency | No | HOLLOW |
| `crates/app/src/stress.rs` | `shard_depth_max` | `ingress_depth_max.min(config.ring_size)` | No | HOLLOW |
| `crates/app/src/stress.rs` | `projection_lag` | `sample_projection_lag` computes locals but returns zero | No | HOLLOW |
| `crates/es-store-postgres/src/projection.rs` | `es_projection_lag` | Current fetched batch, not tenant max global position | Partial | HOLLOW |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Workspace formatting and tests | Orchestrator recently ran `cargo fmt --check` and `cargo test --workspace -- --nocapture` | Passed per user-provided verification context | PASS |
| Required docs wording | Orchestrator recently ran `rg "event store is the source of truth|CommandGateway|single-owner|outbox|ring-only" docs` | Passed per user-provided verification context | PASS |
| Artifact and key-link checks | `gsd-tools verify artifacts` and `gsd-tools verify key-links` for all six plans | All declared artifacts exist; all key links found | PASS |
| Stress report signal quality | Code inspection of `crates/app/src/stress.rs:220-265` and `sample_projection_lag` | Report fields are present but not all measured from named sources | FAIL |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| API-01 | 07-01 | Thin HTTP adapter decodes requests, attaches metadata, sends through bounded ingress, awaits replies. | SATISFIED | Commerce handlers build metadata/envelopes and call `try_submit`. |
| API-02 | 07-01 | Adapter does not mutate aggregate, projector, or outbox state directly. | SATISFIED | Boundary tests and manifest checks forbid direct dependencies and mutation markers. |
| API-03 | 07-01 | API responses include stream revision, global position, correlation ID, typed payloads. | SATISFIED | `CommandSuccess` includes `stream_revision`, `global_positions`, `correlation_id`, `reply`; `ApiError` is typed JSON. |
| API-04 | 07-06 | Docs explain WebSocket/gRPC gateways without shared hot state. | SATISFIED | `docs/template-guide.md` has WebSocket and gRPC sections with exact thin `CommandGateway` guidance. |
| OBS-01 | 07-02 | Runtime emits structured traces with command, correlation, causation, tenant, stream, shard, global position fields. | SATISFIED | Adapter, gateway, engine, shard, and event-store append spans carry the required fields. |
| OBS-02 | 07-02, 07-05 | Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, conflicts, dedupe hits, projection lag, outbox lag, p95/p99 command latency. | BLOCKED | Metric names exist, but projection lag underreports backlog and stress append/shard/projection fields are not measured from their named sources. |
| TEST-02 | 07-03 | Integration tests verify append, conflicts, dedupe, snapshots, projector checkpoints, outbox dispatch against PostgreSQL. | SATISFIED | `phase7_integration.rs` includes four focused Testcontainers tests. |
| TEST-03 | 07-04, 07-05 | Benchmark harnesses separately measure ring, domain, adapter, storage, single-service/full-E2E, projector/outbox, hot-key, burst, degraded dependency. | SATISFIED | Five bench files plus `StressScenario` variants cover the required scenario split. |
| TEST-04 | 07-05 | Single-service stress reports throughput, latency, queue depths, append latency, projection/outbox lag, reject rate, CPU/core utilization. | BLOCKED | Report shape exists, but append latency, shard depth, and projection lag are synthetic/wrong. |
| DOC-01 | 07-06 | Docs state hot-path rules, forbidden patterns, service-boundary guidance, new service guidance. | SATISFIED | Three docs contain the required architecture and extension guidance. |

No Phase 7 requirement IDs from `.planning/REQUIREMENTS.md` were orphaned from plan frontmatter.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/app/src/stress.rs` | 222 | `append_latency.record(elapsed)` | Blocker | `append_latency_p95_micros` reports full command round-trip, not append latency. |
| `crates/app/src/stress.rs` | 264 | `shard_depth_max: ingress_depth_max.min(config.ring_size)` | Blocker | Queue-depth report is synthetic, not observed shard depth. |
| `crates/app/src/stress.rs` | 304 | `Ok(0)` from `sample_projection_lag` | Blocker | Projection lag is always zero regardless of backlog. |
| `crates/es-store-postgres/src/projection.rs` | 95 | Idle `es_projection_lag` set to zero | Warning | Idle path can falsely report caught-up status without checking tenant max global position. |
| `crates/es-store-postgres/src/projection.rs` | 108 | Batch-local projection lag | Warning | Large backlogs can be underreported as only the current batch distance. |
| `crates/es-runtime/src/shard.rs` | 405 | `try_publish(...)?` after ingress acceptance | Warning | Advisory review found accepted shard-ring overload can drop the reply instead of returning explicit overload; not counted as a Phase 7 blocker because HTTP bounded-ingress behavior is present, but it should be fixed. |

### Human Verification Required

None. This phase is code, tests, benchmarks, and documentation; the blocking issues are programmatically identifiable in the data flow.

### Gaps Summary

Phase 7 delivered the thin HTTP API, boundary tests, PostgreSQL integration tests, layer-separated benchmark files, stress runner shape, CLI bootstrap, and documentation. The phase goal is still not fully achieved because the observability/stress layer can report misleading health under load. In particular, projection lag can be zero or underreported while backlog remains, and stress fields for append latency and shard depth are not measured from the named sources.

The advisory code review listed five warnings. WR-04 and WR-05 are treated as goal-blocking because they directly affect OBS-02 and TEST-04. WR-01 through WR-03 are recorded as residual warnings but not as Phase 7 blockers because the required Phase 7 API, integration-test, and documentation contracts are otherwise present.

---

_Verified: 2026-04-18T15:24:23Z_
_Verifier: Claude (gsd-verifier)_

# Phase 7: Adapters, Observability, Stress, and Template Guidance - Research

**Researched:** 2026-04-18 [VERIFIED: environment current_date]
**Domain:** Rust Axum/Tower adapters, tracing/OpenTelemetry/metrics observability, PostgreSQL integration tests, benchmark/stress harnesses, and event-sourcing template documentation [VERIFIED: .planning/ROADMAP.md; VERIFIED: Cargo.toml]
**Confidence:** HIGH for adapter/test/storage boundaries and core observability stack; MEDIUM for exact stress target thresholds because no performance SLOs are locked yet. [VERIFIED: .planning/REQUIREMENTS.md; VERIFIED: .planning/PROJECT.md]

## User Constraints

No Phase 7 CONTEXT.md exists yet, so there are no additional locked discuss-phase decisions to copy verbatim. [VERIFIED: `node /Users/epikem/.codex/get-shit-done/bin/gsd-tools.cjs init phase-op 7`; VERIFIED: `find .planning/phases -name '*CONTEXT.md'`]

### Prompt Constraints

- HTTP command endpoints must decode requests, attach metadata, send through bounded ingress, and return stream revision, global position, correlation ID, and typed success/error payloads. [VERIFIED: user prompt; VERIFIED: .planning/REQUIREMENTS.md]
- Adapter code must not mutate aggregate state, projector state, or outbox state directly. [VERIFIED: user prompt; VERIFIED: .planning/REQUIREMENTS.md]
- Structured traces and metrics must expose command identity, shard/global positions when available, ingress and shard depth, ring wait, decision latency, append latency, conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency. [VERIFIED: user prompt; VERIFIED: .planning/REQUIREMENTS.md]
- Integration tests must verify append, conflicts, dedupe, snapshots, projector checkpoints, and outbox dispatch against real or containerized PostgreSQL. [VERIFIED: user prompt; VERIFIED: crates/es-store-postgres/tests/common/mod.rs]
- Benchmark artifacts must separate ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded-dependency behavior. [VERIFIED: user prompt; VERIFIED: .planning/PROJECT.md]
- A single-service integrated stress test must run production-shaped in-process composition and report throughput, p50/p95/p99 latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization. [VERIFIED: user prompt; VERIFIED: .planning/ROADMAP.md]
- Documentation must state hot-path rules, service-boundary guidance, and how to interpret single-service stress results separately from ring-only microbenchmarks. [VERIFIED: user prompt; VERIFIED: .planning/REQUIREMENTS.md]

### Project Constraints

- Prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: AGENTS.md]
- Rust is the primary implementation language for this template. [VERIFIED: AGENTS.md; VERIFIED: Cargo.toml]
- The event store is the source of truth; disruptor rings must not be treated as durable state. [VERIFIED: AGENTS.md; VERIFIED: .planning/PROJECT.md]
- The same aggregate or ordered partition key must map to the same shard owner. [VERIFIED: AGENTS.md; VERIFIED: crates/es-runtime/src/router.rs]
- Hot business state should be single-owner and processor-local where practical, avoiding shared mutable state in adapter handlers. [VERIFIED: AGENTS.md; VERIFIED: crates/es-runtime/src/shard.rs]
- External publication must flow through durable outbox rows committed with domain events. [VERIFIED: AGENTS.md; VERIFIED: crates/es-store-postgres/src/sql.rs]
- Adapter, command engine, projection, and outbox concerns should stay separable. [VERIFIED: AGENTS.md; VERIFIED: Cargo.toml]
- Performance tests must separate ring-only, domain-only, adapter-only, full E2E, soak, chaos, and single-service integrated scenarios. [VERIFIED: AGENTS.md; VERIFIED: .planning/PROJECT.md]

## Project Constraints (from CLAUDE.md)

No `./CLAUDE.md` file exists in this workspace; project instructions are supplied by `AGENTS.md`. [VERIFIED: `find . -maxdepth 3 -name CLAUDE.md`]

Actionable directives from `AGENTS.md`: prefer `pnpm` for Node, prefer `uv` for Python, use GSD workflow before file-changing work, preserve event-store source-of-truth semantics, keep disruptor in-process and non-durable, route ordered keys consistently to shard owners, keep hot business state single-owner, publish externally only through committed outbox rows, keep service concerns separable, and split performance tests by layer. [VERIFIED: AGENTS.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| API-01 | Thin HTTP adapter exposes command endpoints that decode requests, attach metadata, send through bounded ingress, and await command replies. [VERIFIED: .planning/REQUIREMENTS.md] | Use `axum` 0.8.9 extractors/State plus existing `CommandGateway::try_submit` and oneshot replies. [VERIFIED: cargo info axum; VERIFIED: crates/es-runtime/src/gateway.rs; CITED: https://docs.rs/axum/0.8.9/axum/extract/index.html] |
| API-02 | Adapter code does not mutate aggregate state, projector state, or outbox state directly. [VERIFIED: .planning/REQUIREMENTS.md] | Keep adapter dependencies pointed at runtime gateway and DTOs, not projection/outbox repositories or aggregate caches. [VERIFIED: crates/adapter-http/src/lib.rs; VERIFIED: crates/es-runtime/src/gateway.rs] |
| API-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads. [VERIFIED: .planning/REQUIREMENTS.md] | `CommandOutcome` already carries `CommittedAppend` with stream revisions and global positions after durable append. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/models.rs] |
| API-04 | Documentation explains how WebSocket or gRPC gateways connect without sharing hot business state. [VERIFIED: .planning/REQUIREMENTS.md] | Document all gateways as thin ingress clients of `CommandGateway` and query/projector APIs. [VERIFIED: .planning/PROJECT.md; VERIFIED: crates/adapter-grpc/src/lib.rs] |
| OBS-01 | Runtime emits structured traces with command ID, correlation ID, causation ID, tenant ID, stream ID, shard ID, and global position when available. [VERIFIED: .planning/REQUIREMENTS.md] | Use `tracing` spans at adapter, gateway, shard, append, projection, and outbox boundaries. [VERIFIED: cargo info tracing; CITED: https://docs.rs/tracing/0.1.44] |
| OBS-02 | Metrics expose queue depths, ring wait, decision/append latency, conflicts, dedupe hits, lag, and p95/p99 command latency. [VERIFIED: .planning/REQUIREMENTS.md] | Use `metrics` facade histograms/gauges/counters for runtime metrics plus `hdrhistogram` for stress summaries. [VERIFIED: cargo info metrics; VERIFIED: cargo info hdrhistogram; CITED: https://docs.rs/metrics] |
| TEST-02 | Integration tests verify storage, projection, and outbox behavior against real/containerized PostgreSQL. [VERIFIED: .planning/REQUIREMENTS.md] | Reuse existing Testcontainers PostgreSQL 18 harness and expand cross-layer integration coverage. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs; CITED: https://github.com/testcontainers/testcontainers-rs] |
| TEST-03 | Benchmark harnesses separately measure each required layer/scenario. [VERIFIED: .planning/REQUIREMENTS.md] | Use Criterion-compatible microbenches for deterministic layer benches and custom Tokio stress binary/tests for integrated load. [VERIFIED: cargo info criterion; CITED: https://criterion-rs.github.io/book] |
| TEST-04 | Single-service integrated stress test runs production-shaped composition and reports required metrics. [VERIFIED: .planning/REQUIREMENTS.md] | Compose adapter DTO path, bounded gateway, partition router, shard runtime, event store, projector, outbox dispatcher, and query path in one process. [VERIFIED: crates/app/src/lib.rs; VERIFIED: crates/es-runtime/src/engine.rs; VERIFIED: crates/es-store-postgres/src/event_store.rs] |
| DOC-01 | Documentation states hot-path rules, forbidden patterns, service-boundary guidance, and how to create a new domain service. [VERIFIED: .planning/REQUIREMENTS.md] | Write template guidance around established crate boundaries and source-of-truth rules. [VERIFIED: .planning/PROJECT.md; VERIFIED: Cargo.toml] |

</phase_requirements>

## Summary

Phase 7 should finish the template by adding a thin HTTP adapter and a service-composition/stress layer without moving business state into adapters. [VERIFIED: .planning/ROADMAP.md; VERIFIED: crates/adapter-http/src/lib.rs] The adapter should convert HTTP JSON into typed commerce commands, construct `CommandMetadata`, submit `CommandEnvelope`s through the existing bounded `CommandGateway::try_submit`, await oneshot replies, and map `RuntimeError` variants into typed JSON errors. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-runtime/src/gateway.rs; CITED: https://docs.rs/axum/0.8.9/axum/extract/index.html]

Observability should be implemented as project-owned instrumentation points over existing boundaries, not as a tracing-only afterthought. [VERIFIED: crates/es-runtime/src/engine.rs; VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-outbox/src/dispatcher.rs] Use `tracing` spans for causality and event identity, `metrics` facade instruments for counters/gauges/histograms, optional Prometheus exporter for local scraping, and OpenTelemetry OTLP/tracing integration at the app boundary for vendor-neutral export. [VERIFIED: cargo info tracing; VERIFIED: cargo info metrics; VERIFIED: cargo info metrics-exporter-prometheus; VERIFIED: cargo info opentelemetry-otlp; CITED: https://github.com/open-telemetry/opentelemetry-rust]

Testing and benchmarks should reuse existing real PostgreSQL paths and produce separate artifacts by layer. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs; VERIFIED: .planning/PROJECT.md] Latest `criterion` 0.8.2 requires Rust 1.86, while this workspace pins Rust 1.85, so Phase 7 should use `criterion` 0.7.0 unless the planner includes an explicit Rust version upgrade. [VERIFIED: Cargo.toml; VERIFIED: cargo info criterion@0.8.2; VERIFIED: cargo info criterion]

**Primary recommendation:** Implement `adapter-http` with `axum`/`tower`, add an `es-observability` or app-owned observability module using `tracing` + `metrics` + optional OTLP/Prometheus export, expand PostgreSQL integration tests, create compatible Criterion microbenches plus a custom Tokio stress runner, and document gateway/hot-path rules. [VERIFIED: cargo info axum; VERIFIED: cargo info tower; VERIFIED: cargo info tracing; VERIFIED: cargo info metrics; VERIFIED: crates/es-runtime/src/gateway.rs]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| HTTP command request decoding | API / Backend adapter | Browser / Client | Axum handlers own JSON/header extraction and response mapping; clients do not see aggregate internals. [VERIFIED: crates/adapter-http/src/lib.rs; CITED: https://docs.rs/axum/0.8.9/axum/extract/index.html] |
| Metadata and idempotency construction | API / Backend adapter | API / Backend runtime | Adapter reads request headers or generates IDs; runtime envelope validates stream/partition/expected revision. [VERIFIED: crates/es-runtime/src/command.rs] |
| Bounded ingress and overload | API / Backend runtime | API / Backend adapter | `CommandGateway` owns bounded mpsc admission and maps full queues to `RuntimeError::Overloaded`; adapter maps that to HTTP status. [VERIFIED: crates/es-runtime/src/gateway.rs; VERIFIED: crates/es-runtime/src/error.rs] |
| Aggregate mutation | API / Backend shard runtime | Database / Storage | Only shard runtime calls aggregate `decide/apply`; durable commit is PostgreSQL append. [VERIFIED: crates/es-runtime/src/shard.rs; VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Projection state | API / Backend projector | Database / Storage | Projector writes read models and offsets atomically; adapter can query but must not mutate projector state directly. [VERIFIED: crates/es-store-postgres/src/projection.rs] |
| Outbox state | API / Backend outbox worker | Database / Storage | Dispatcher claims and updates outbox rows through storage-neutral `OutboxStore`; adapters must not publish or mark outbox rows. [VERIFIED: crates/es-outbox/src/dispatcher.rs; VERIFIED: crates/es-store-postgres/src/outbox.rs] |
| Structured tracing | API / Backend app/runtime | External telemetry backend | Runtime emits spans/fields; OpenTelemetry/collector export is configured at app edge. [VERIFIED: cargo info tracing; VERIFIED: cargo info tracing-opentelemetry; VERIFIED: cargo info opentelemetry-otlp] |
| Metrics and stress summaries | API / Backend app/runtime | External metrics backend | Runtime records queue/latency/lag metrics; stress runner aggregates p50/p95/p99 and CPU/core utilization. [VERIFIED: cargo info metrics; VERIFIED: cargo info hdrhistogram; VERIFIED: cargo info sysinfo] |
| Integration tests | Database / Storage | API / Backend app | PostgreSQL semantics are required for append/OCC/dedupe/projection/outbox; tests should use the existing container harness. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| Template guidance | Documentation | API / Backend architecture | Docs encode boundaries and forbidden patterns so future domain services do not violate event-source and single-owner assumptions. [VERIFIED: .planning/PROJECT.md] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | 0.8.9, published 2026-04-14 | HTTP routing, extractors, JSON request/response boundary | Axum uses Tower middleware and supports typed extractors/rejections, matching a thin adapter over runtime gateways. [VERIFIED: crates.io API; VERIFIED: cargo info axum; CITED: https://docs.rs/axum/0.8.9] |
| `tower` | 0.5.3, published 2026-01-12 | Timeout, load shedding, concurrency/admission middleware | Tower `ServiceBuilder` composes timeout/load-shed/concurrency layers at the ingress edge. [VERIFIED: crates.io API; VERIFIED: cargo info tower; CITED: https://docs.rs/tower/0.5.3] |
| `tower-http` | 0.6.8, published 2025-12-08 | HTTP tracing, compression/CORS/request utilities | Tower HTTP provides HTTP-specific middleware without leaking into domain/runtime crates. [VERIFIED: crates.io API; VERIFIED: cargo info tower-http; CITED: https://docs.rs/tower-http/0.6.8] |
| `tracing` | 0.1.44, published 2025-12-18 | Structured spans/events | Existing workspace already depends on `tracing`; span fields fit command/correlation/tenant/shard/global-position tracing. [VERIFIED: Cargo.toml; VERIFIED: crates.io API; CITED: https://docs.rs/tracing/0.1.44] |
| `tracing-subscriber` | 0.3.23, published 2026-03-13 | App-level trace subscriber/filter/format layers | Needed in the app crate to initialize logs/spans without making lower crates global-init aware. [VERIFIED: crates.io API; VERIFIED: cargo info tracing-subscriber] |
| `metrics` | 0.24.3, published 2025-11-28 | Lightweight metrics facade for counters/gauges/histograms | Lets lower crates emit metrics without committing to Prometheus or OTLP SDK types in their APIs. [VERIFIED: crates.io API; VERIFIED: cargo info metrics; CITED: https://docs.rs/metrics] |
| `hdrhistogram` | 7.5.4, published 2023-11-18 | Stress-run p50/p95/p99/max latency summaries | Required because p95/p99 command latency is an explicit Phase 7 output and averages are insufficient. [VERIFIED: crates.io API; VERIFIED: cargo info hdrhistogram; VERIFIED: .planning/REQUIREMENTS.md] |
| `testcontainers` | Keep 0.25.0 for Rust 1.85 compatibility; latest 0.27.3 published 2026-04-15 | Containerized PostgreSQL integration tests | Existing harness already uses 0.25.0 and starts PostgreSQL 18 successfully. [VERIFIED: Cargo.toml; VERIFIED: crates.io API; VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| `testcontainers-modules` | Keep 0.13.0 for current workspace; latest 0.15.0 published 2026-02-21 | PostgreSQL module for Testcontainers | Existing harness uses `testcontainers_modules::postgres::Postgres` with image tag 18. [VERIFIED: Cargo.toml; VERIFIED: crates.io API; VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| `criterion` | Use 0.7.0 under Rust 1.85; latest 0.8.2 requires Rust 1.86 | Layer microbenchmark harness | Criterion provides statistical microbenchmarks and throughput reporting, but latest release exceeds current MSRV. [VERIFIED: cargo info criterion; VERIFIED: cargo info criterion@0.8.2; CITED: https://criterion-rs.github.io/book] |
| `sysinfo` | Use 0.36.1 under Rust 1.85; latest 0.38.4 requires Rust 1.88 | CPU/core utilization for stress reports | Latest sysinfo exceeds current MSRV; 0.36.1 is compatible with Rust 1.75 and can report system/process CPU data. [VERIFIED: cargo info sysinfo; VERIFIED: cargo info sysinfo@0.38.4] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tracing-opentelemetry` | 0.32.1, published 2026-01-12 | Bridge tracing spans to OpenTelemetry traces | Use in app initialization when OTLP trace export is enabled. [VERIFIED: crates.io API; VERIFIED: cargo info tracing-opentelemetry] |
| `opentelemetry` | 0.31.0, published 2025-09-25 | Vendor-neutral telemetry API | Use at app/export boundary for OTLP traces/metrics, not as a domain crate dependency. [VERIFIED: crates.io API; VERIFIED: cargo info opentelemetry; CITED: https://github.com/open-telemetry/opentelemetry-rust] |
| `opentelemetry_sdk` | 0.31.0, published 2025-09-25 | Meter/tracer providers and exporters | Use only in app observability initialization. [VERIFIED: crates.io API; VERIFIED: cargo info opentelemetry_sdk] |
| `opentelemetry-otlp` | 0.31.1, published 2026-03-19 | OTLP exporter to collector/backends | Use for optional production-shaped trace/metric export. [VERIFIED: crates.io API; VERIFIED: cargo info opentelemetry-otlp; CITED: https://github.com/open-telemetry/opentelemetry-rust] |
| `metrics-exporter-prometheus` | 0.18.1, published 2025-12-07 | Local Prometheus scrape endpoint | Use for local stress runs and template observability examples. [VERIFIED: crates.io API; VERIFIED: cargo info metrics-exporter-prometheus; CITED: https://docs.rs/metrics] |
| `tokio` | Workspace 1.52.0 resolves compatibly; crates.io max 1.52.1 published 2026-04-16 | Async HTTP/runtime/tests/stress orchestration | Already used by runtime and Testcontainers async tests. [VERIFIED: Cargo.toml; VERIFIED: crates.io API; VERIFIED: crates/es-runtime/src/gateway.rs] |
| `serde` / `serde_json` | `serde` 1.0.228, `serde_json` 1.0.149 | JSON DTOs and event payload fixtures | Use at adapter/storage boundaries only; do not replace typed domain commands/events in the hot path. [VERIFIED: Cargo.toml; VERIFIED: crates/example-commerce/src/order.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `axum` | Actix Web | Do not switch; existing stack is Tokio/Tower-aligned and Axum 0.8.9 is current. [VERIFIED: Cargo.toml; VERIFIED: cargo info axum] |
| `metrics` facade | Direct OpenTelemetry instruments everywhere | Use `metrics` in lower crates to avoid SDK coupling; initialize OpenTelemetry at app boundary. [VERIFIED: cargo info metrics; VERIFIED: cargo info opentelemetry] |
| Criterion 0.8.2 | Upgrade Rust to 1.86+ | Only use if the phase explicitly upgrades `rust-version`; otherwise use Criterion 0.7.0. [VERIFIED: Cargo.toml; VERIFIED: cargo info criterion@0.8.2] |
| `sysinfo` latest 0.38.4 | Upgrade Rust to 1.88+ | Only use if the phase explicitly upgrades toolchain; otherwise use 0.36.1. [VERIFIED: cargo info sysinfo@0.38.4; VERIFIED: cargo info sysinfo] |
| Mock DB tests | Existing PostgreSQL Testcontainers harness | Do not replace TEST-02 coverage; PostgreSQL transaction/locking/upsert behavior is required. [VERIFIED: .planning/REQUIREMENTS.md; VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |

**Installation:**

```bash
cargo add axum@0.8.9 tower@0.5.3 tower-http@0.6.8 -p adapter-http
cargo add tracing-subscriber@0.3.23 metrics@0.24.3 hdrhistogram@7.5.4 sysinfo@0.36.1 -p app
cargo add tracing-opentelemetry@0.32.1 opentelemetry@0.31.0 opentelemetry_sdk@0.31.0 opentelemetry-otlp@0.31.1 metrics-exporter-prometheus@0.18.1 -p app --optional
cargo add criterion@0.7.0 --dev
```

The exact workspace edits should use workspace dependencies rather than per-crate version duplication because the current root `Cargo.toml` centralizes dependency policy. [VERIFIED: Cargo.toml]

## Architecture Patterns

### System Architecture Diagram

```text
HTTP client
  |
  v
Axum route + extractors
  | decode JSON, read/generate IDs, validate idempotency key
  v
Adapter command mapper
  | builds CommandMetadata + typed commerce command
  v
CommandEnvelope::new
  | derives stream_id, partition_key, expected_revision
  v
CommandGateway::try_submit
  | bounded ingress
  |-- full/closed --> typed HTTP overload/unavailable error
  v
PartitionRouter
  | tenant + partition key
  v
Shard runtime + disruptor handoff
  | rehydrate/cache -> decide -> encode -> append
  v
PostgreSQL event store transaction
  | events + dedupe + optional outbox rows commit
  |-- conflict/dedupe/store error --> typed runtime error
  v
CommandOutcome
  | reply + stream revision + global positions
  v
HTTP typed success response

Committed events by global_position
  |                         |
  v                         v
Projector catch-up      Outbox dispatcher / process manager
  |                         |
  v                         v
Read models + offsets    Published/retry rows + process-manager offsets

Instrumentation wraps every boundary above:
adapter span -> gateway metrics -> shard/ring metrics -> append metrics -> projection/outbox lag -> stress summaries.
```

### Recommended Project Structure

```text
crates/
├── adapter-http/
│   ├── src/lib.rs              # router factory, shared app state, response/error models
│   ├── src/commerce.rs         # commerce command endpoints and request DTO mapping
│   ├── src/error.rs            # HTTP error mapping from RuntimeError/domain errors
│   └── tests/commerce_api.rs   # adapter-level tests with fake gateway/service
├── app/
│   ├── src/lib.rs              # production-shaped composition exports
│   ├── src/observability.rs    # tracing subscriber, metrics descriptions/exporters
│   ├── src/stress.rs           # single-service integrated stress runner core
│   └── src/main.rs             # CLI/bootstrap shell for server or stress mode
├── benches/
│   ├── ring_only.rs
│   ├── domain_only.rs
│   ├── adapter_only.rs
│   ├── storage_only.rs
│   └── projector_outbox.rs
└── docs/
    ├── template-guide.md       # how to create a new domain service
    ├── hot-path-rules.md       # forbidden patterns and boundaries
    └── stress-results.md       # how to interpret benchmark/stress artifacts
```

This structure keeps HTTP DTOs in `adapter-http`, telemetry bootstrap in `app`, and benchmarks at workspace root. [VERIFIED: current crate layout from `rg --files`; VERIFIED: Cargo.toml]

### Pattern 1: Thin Axum Command Endpoint

**What:** Handler extracts state, headers, and JSON, builds metadata/envelope, submits through bounded gateway, awaits the reply, and returns a typed response. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-runtime/src/gateway.rs; CITED: https://docs.rs/axum/0.8.9/axum/extract/index.html]

**When to use:** Use for every HTTP command endpoint in Phase 7. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: existing CommandGateway/CommandEnvelope APIs + Axum 0.8 Json/State extractors.
use axum::{extract::State, Json};
use es_core::CommandMetadata;
use es_runtime::{CommandEnvelope, CommandGateway};
use example_commerce::{Order, OrderCommand};
use uuid::Uuid;

pub async fn place_order(
    State(gateway): State<CommandGateway<Order>>,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<CommandSuccess<OrderReplyDto>>, ApiError> {
    let metadata = CommandMetadata {
        command_id: request.command_id.unwrap_or_else(Uuid::now_v7),
        correlation_id: request.correlation_id.unwrap_or_else(Uuid::now_v7),
        causation_id: request.causation_id,
        tenant_id: request.tenant_id,
        requested_at: time::OffsetDateTime::now_utc(),
    };

    let (reply, receiver) = tokio::sync::oneshot::channel();
    let envelope = CommandEnvelope::<Order>::new(
        OrderCommand::PlaceOrder(request.into_domain()?),
        metadata.clone(),
        request.idempotency_key,
        reply,
    )?;

    gateway.try_submit(envelope)?;
    let outcome = receiver.await.map_err(|_| ApiError::ReplyDropped)??;

    Ok(Json(CommandSuccess::from_outcome(
        metadata.correlation_id,
        outcome,
    )))
}
```

### Pattern 2: Tower Middleware Outside the Hot Domain Path

**What:** Apply request timeout/load shedding/concurrency limits at the HTTP service layer before commands enter the runtime gateway. [CITED: https://docs.rs/tower/0.5.3/tower/struct.ServiceBuilder.html; CITED: https://docs.rs/axum/0.8.9/axum/error_handling/index.html]

**When to use:** Use in adapter router construction to fail overloaded HTTP traffic early while still preserving `CommandGateway` bounded ingress as the runtime backpressure boundary. [VERIFIED: crates/es-runtime/src/gateway.rs]

**Example:**

```rust
// Source: Tower ServiceBuilder docs and Axum error handling docs.
use axum::{error_handling::HandleErrorLayer, http::StatusCode, Router};
use std::time::Duration;
use tower::ServiceBuilder;

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/orders", axum::routing::post(place_order))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|_err| async {
                    (StatusCode::REQUEST_TIMEOUT, "request timed out")
                }))
                .timeout(Duration::from_secs(5))
                .load_shed()
                .concurrency_limit(256),
        )
}
```

### Pattern 3: Metrics as Boundary Instruments

**What:** Record counters for outcomes, gauges for depths/lags, and histograms for durations at adapter, gateway, shard, store, projector, and outbox boundaries. [VERIFIED: .planning/REQUIREMENTS.md; CITED: https://docs.rs/metrics]

**When to use:** Use for OBS-02 and stress reporting; avoid high-cardinality labels such as raw command ID on metrics. [CITED: https://github.com/open-telemetry/opentelemetry-rust; ASSUMED]

**Example:**

```rust
// Source: metrics-rs histogram/counter/gauge APIs.
use metrics::{counter, gauge, histogram};
use std::time::Instant;

pub async fn observe_append<F, T, E>(stream_type: &'static str, work: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let start = Instant::now();
    let result = work.await;
    histogram!("es_append_latency_seconds", "stream_type" => stream_type)
        .record(start.elapsed());

    match &result {
        Ok(_) => counter!("es_append_total", "outcome" => "ok").increment(1),
        Err(_) => counter!("es_append_total", "outcome" => "error").increment(1),
    }

    result
}

pub fn record_queue_depth(name: &'static str, depth: usize) {
    gauge!("es_queue_depth", "queue" => name).set(depth as f64);
}
```

### Pattern 4: Real PostgreSQL Integration Test Harness

**What:** Reuse the existing async Testcontainers PostgreSQL 18 harness and add cross-layer tests that exercise append/OCC/dedupe/snapshot/projection/outbox through real repositories. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs]

**When to use:** Use for TEST-02; do not substitute SQLite or pure mocks for acceptance coverage. [VERIFIED: .planning/REQUIREMENTS.md]

**Example:**

```rust
// Source: existing crates/es-store-postgres/tests/common/mod.rs.
#[tokio::test]
async fn command_path_persists_projectable_event_and_outbox_row() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;
    let store = es_store_postgres::PostgresEventStore::new(harness.pool.clone());
    let outbox = es_store_postgres::PostgresOutboxStore::new(harness.pool.clone());

    let outcome = store.append(valid_append_request_with_outbox()).await?;
    let committed = match outcome {
        es_store_postgres::AppendOutcome::Committed(committed) => committed,
        es_store_postgres::AppendOutcome::Duplicate(committed) => committed,
    };

    assert_eq!(vec![1], committed.global_positions);
    assert_eq!(1, outbox.pending_count(&tenant_id()).await?);
    Ok(())
}
```

### Pattern 5: Separate Microbenchmarks from Integrated Stress

**What:** Use Criterion for deterministic layer microbenchmarks and a custom Tokio stress runner for full service composition, tail latency, queue depth, lag, reject rate, and CPU/core utilization. [VERIFIED: .planning/REQUIREMENTS.md; VERIFIED: cargo info criterion; VERIFIED: cargo info hdrhistogram; VERIFIED: cargo info sysinfo]

**When to use:** Use Criterion for ring/domain/adapter/storage helpers; use stress runner for production-shaped one-process load. [VERIFIED: .planning/PROJECT.md]

**Example:**

```rust
// Source: Criterion BenchmarkGroup + Throughput docs.
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn domain_only(c: &mut Criterion) {
    let commands = fixture_commands();
    let mut group = c.benchmark_group("domain_only");
    group.throughput(Throughput::Elements(commands.len() as u64));
    group.bench_function("order_place_decide_apply", |b| {
        b.iter(|| run_domain_sequence(&commands))
    });
    group.finish();
}

criterion_group!(benches, domain_only);
criterion_main!(benches);
```

### Anti-Patterns to Avoid

- **Adapter owns aggregate state:** HTTP/gRPC/WebSocket handlers must not hold aggregate caches or mutate business state. [VERIFIED: AGENTS.md; VERIFIED: crates/es-runtime/src/shard.rs]
- **Direct outbox/broker publish from adapter:** External effects must flow through committed outbox rows. [VERIFIED: AGENTS.md; VERIFIED: crates/es-store-postgres/src/sql.rs]
- **Projection wait as command success:** API command success must return after durable append, not after projector completion. [VERIFIED: .planning/PROJECT.md; VERIFIED: crates/es-runtime/src/command.rs]
- **Using disruptor sequence as global position:** Client-visible positions, projection checkpoints, and outbox ordering must use durable PostgreSQL global positions. [VERIFIED: .planning/PROJECT.md; VERIFIED: crates/es-store-postgres/src/models.rs]
- **One benchmark number for the whole system:** Keep ring-only, domain-only, adapter-only, storage-only, integrated, and degraded-dependency results separate. [VERIFIED: .planning/PROJECT.md]
- **High-cardinality metrics labels:** Put command IDs/correlation IDs in traces/logs, not metric labels. [ASSUMED]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP routing/extraction | Custom Hyper router and JSON body parser | `axum` 0.8.9 | Axum already provides Router, State, Json, extractor rejections, and Tower integration. [VERIFIED: cargo info axum; CITED: https://docs.rs/axum/0.8.9] |
| HTTP timeouts/load shedding | Per-handler sleep/select wrappers | `tower::ServiceBuilder` layers | Tower layers compose outside handler logic and keep overload policy centralized. [CITED: https://docs.rs/tower/0.5.3/tower/struct.ServiceBuilder.html] |
| Structured spans | Custom log context maps | `tracing` + `tracing-subscriber` | Existing workspace uses `tracing`; spans naturally carry command/correlation/shard fields. [VERIFIED: Cargo.toml; VERIFIED: cargo info tracing] |
| Metrics facade/export | Global mutable counters with ad hoc scraping | `metrics` + `metrics-exporter-prometheus`, optional OpenTelemetry | Metrics crates provide standard counters/gauges/histograms and exporter integration. [VERIFIED: cargo info metrics; VERIFIED: cargo info metrics-exporter-prometheus; CITED: https://docs.rs/metrics] |
| Tail latency summaries | Sorting Vecs of durations in stress code | `hdrhistogram` | HdrHistogram is built for latency distributions and avoids average-only reporting. [VERIFIED: cargo info hdrhistogram] |
| PostgreSQL test lifecycle | Shell scripts that start/stop shared DBs | Existing Testcontainers harness | Existing tests already start isolated PostgreSQL 18 containers and run migrations. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| Statistical microbenchmarks | Custom timing loops for domain/ring/storage helpers | `criterion` 0.7.0 under Rust 1.85 | Criterion provides benchmark groups, throughput, and statistical reporting; 0.7.0 is compatible with current MSRV. [VERIFIED: cargo info criterion; CITED: https://criterion-rs.github.io/book] |
| CPU/core utilization | Parsing `top` output | `sysinfo` 0.36.1 | A library API is more portable for stress artifacts than platform-specific command parsing. [VERIFIED: cargo info sysinfo] |

**Key insight:** Phase 7 should standardize boundaries and measurement, not add clever infrastructure. The high-risk work is proving adapters stay thin and stress numbers are interpretable by layer. [VERIFIED: .planning/PROJECT.md; VERIFIED: .planning/REQUIREMENTS.md]

## Common Pitfalls

### Pitfall 1: Adapter State Leakage

**What goes wrong:** HTTP handlers keep aggregate caches, projection repositories, or outbox dispatch state directly. [VERIFIED: .planning/PROJECT.md]

**Why it happens:** It is tempting to make the adapter "convenient" by giving handlers access to all app components. [ASSUMED]

**How to avoid:** Define adapter state as gateway/query clients only; add dependency-boundary tests or `rg` checks proving `adapter-http` does not import aggregate cache, projector mutation, or outbox dispatcher APIs. [VERIFIED: crates/es-runtime/src/gateway.rs; VERIFIED: crates/es-projection/src/query.rs; VERIFIED: crates/es-outbox/src/dispatcher.rs]

**Warning signs:** `adapter-http` imports `AggregateCache`, `PostgresOutboxStore::mark_*`, projection catch-up methods, or `Arc<Mutex<HashMap<...>>>`. [VERIFIED: crates/es-runtime/src/cache.rs; VERIFIED: crates/es-store-postgres/src/outbox.rs]

### Pitfall 2: Dedupe and Overload Status Mapped as Generic 500s

**What goes wrong:** Clients cannot distinguish conflict, retryable overload, unavailable runtime, duplicate success, and validation errors. [VERIFIED: .planning/REQUIREMENTS.md]

**Why it happens:** Axum handlers often start with `anyhow` or string errors. [ASSUMED]

**How to avoid:** Implement `ApiError` with `IntoResponse`; map `RuntimeError::Overloaded` to 429 or 503, stream conflicts to 409, validation to 400, and successful dedupe to normal success with the prior committed positions. [VERIFIED: crates/es-runtime/src/error.rs; VERIFIED: crates/es-store-postgres/src/models.rs; CITED: https://docs.rs/axum/0.8.9/axum/response/trait.IntoResponse.html]

**Warning signs:** Handler returns `Result<_, anyhow::Error>` or all errors become HTTP 500. [VERIFIED: crates/adapter-http/src/lib.rs]

### Pitfall 3: Metrics Cardinality Explosion

**What goes wrong:** Metrics labels include command IDs, stream IDs, idempotency keys, or correlation IDs and overwhelm the backend. [ASSUMED]

**Why it happens:** The trace field list overlaps with metric dimensions in OBS-01/OBS-02. [VERIFIED: .planning/REQUIREMENTS.md]

**How to avoid:** Put unique identifiers in `tracing` spans/events; keep metric labels bounded to command type, aggregate type, shard ID, outcome, projector name, and outbox topic. [CITED: https://github.com/open-telemetry/opentelemetry-rust; ASSUMED]

**Warning signs:** Metric calls label raw UUIDs or arbitrary stream IDs. [ASSUMED]

### Pitfall 4: Latest Crates Break Rust 1.85

**What goes wrong:** Adding latest `criterion` or `sysinfo` forces a Rust version upgrade that violates current workspace policy. [VERIFIED: Cargo.toml; VERIFIED: cargo info criterion@0.8.2; VERIFIED: cargo info sysinfo@0.38.4]

**Why it happens:** `cargo add` defaults to the newest compatible under the local toolchain sometimes, while research tables often list latest ecosystem versions. [VERIFIED: cargo info criterion]

**How to avoid:** Pin `criterion = "0.7.0"` and `sysinfo = "0.36.1"` unless the plan intentionally updates `rust-version`. [VERIFIED: cargo info criterion; VERIFIED: cargo info sysinfo]

**Warning signs:** `cargo check` asks for Rust 1.86+ or 1.88+. [VERIFIED: cargo info criterion@0.8.2; VERIFIED: cargo info sysinfo@0.38.4]

### Pitfall 5: Integrated Stress Hides Layer Bottlenecks

**What goes wrong:** A single throughput number is misread as ring performance or database performance. [VERIFIED: .planning/PROJECT.md]

**Why it happens:** Full service runs combine adapter JSON, queueing, routing, disruptor handoff, domain logic, append I/O, projection, outbox, and query behavior. [VERIFIED: crates/es-runtime/src/engine.rs; VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-store-postgres/src/projection.rs; VERIFIED: crates/es-outbox/src/dispatcher.rs]

**How to avoid:** Emit separate artifacts for each benchmark class and include a stress-results doc explaining what each number includes. [VERIFIED: .planning/REQUIREMENTS.md]

**Warning signs:** Documentation compares ring-only microbenchmarks directly to HTTP E2E p99. [VERIFIED: .planning/PROJECT.md]

### Pitfall 6: Testcontainers Docker Availability Assumed

**What goes wrong:** Integration tests fail in environments where Docker is unavailable even though unit tests pass. [VERIFIED: crates/es-store-postgres/tests/common/mod.rs]

**Why it happens:** TEST-02 requires real/containerized PostgreSQL, and the current harness uses Docker via Testcontainers. [VERIFIED: .planning/REQUIREMENTS.md; VERIFIED: crates/es-store-postgres/tests/common/mod.rs]

**How to avoid:** Keep Testcontainers as default and document `DATABASE_URL` or manual PostgreSQL fallback only if implemented. [ASSUMED]

**Warning signs:** CI/local runs skip all DB tests without a clear marker. [ASSUMED]

## Code Examples

### Typed API Response from `CommandOutcome`

```rust
// Source: crates/es-runtime/src/command.rs and crates/es-store-postgres/src/models.rs.
#[derive(serde::Serialize)]
pub struct CommandSuccess<T> {
    pub correlation_id: uuid::Uuid,
    pub stream_revision: u64,
    pub global_position: i64,
    pub result: T,
}

impl<T, R> From<(uuid::Uuid, es_runtime::CommandOutcome<R>)> for CommandSuccess<T>
where
    T: From<R>,
{
    fn from((correlation_id, outcome): (uuid::Uuid, es_runtime::CommandOutcome<R>)) -> Self {
        let append = outcome.append;
        Self {
            correlation_id,
            stream_revision: append.last_revision.value(),
            global_position: append.global_positions.last().copied().unwrap_or_default(),
            result: T::from(outcome.reply),
        }
    }
}
```

### Tracing Span Fields for Command Path

```rust
// Source: tracing 0.1.44 span fields and existing CommandEnvelope fields.
let span = tracing::info_span!(
    "command.handle",
    tenant_id = %envelope.metadata.tenant_id.as_str(),
    command_id = %envelope.metadata.command_id,
    correlation_id = %envelope.metadata.correlation_id,
    causation_id = ?envelope.metadata.causation_id,
    stream_id = %envelope.stream_id.as_str(),
    partition_key = %envelope.partition_key.as_str(),
    shard_id = routed.shard_id.value(),
);

async move {
    // route -> shard -> append -> reply
}
.instrument(span)
.await;
```

### Stress Runner Latency Summary

```rust
// Source: hdrhistogram 7.5.4 and Phase 7 TEST-04 reporting requirements.
let mut histogram = hdrhistogram::Histogram::<u64>::new_with_bounds(1, 60_000_000, 3)
    .expect("latency histogram");

for duration in completed_command_latencies {
    histogram
        .record(duration.as_micros() as u64)
        .expect("record latency");
}

let summary = StressLatencySummary {
    p50_micros: histogram.value_at_quantile(0.50),
    p95_micros: histogram.value_at_quantile(0.95),
    p99_micros: histogram.value_at_quantile(0.99),
    max_micros: histogram.max(),
};
```

### Testcontainers PostgreSQL Reuse Pattern

```rust
// Source: crates/es-store-postgres/tests/common/mod.rs.
use sqlx::{PgPool, postgres::PgPoolOptions};
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

pub async fn start_postgres() -> anyhow::Result<(ContainerAsync<Postgres>, PgPool)> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");
    let pool = PgPoolOptions::new().max_connections(5).connect(&database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok((container, pool))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Latest Criterion by default | Criterion 0.7.0 under Rust 1.85, or explicit Rust upgrade for 0.8.2 | Criterion 0.8.2 published 2026-02-04 with Rust 1.86 MSRV. [VERIFIED: crates.io API; VERIFIED: cargo info criterion@0.8.2] | Planner must not blindly add latest Criterion unless toolchain policy changes. |
| Latest sysinfo by default | sysinfo 0.36.1 under Rust 1.85, or explicit Rust upgrade for 0.38.4 | sysinfo 0.38.4 published 2026-03-09 with Rust 1.88 MSRV. [VERIFIED: crates.io API; VERIFIED: cargo info sysinfo@0.38.4] | Stress CPU reporting should use compatible version or avoid dependency. |
| Logs-only observability | Structured tracing plus metrics histograms/gauges/counters and optional OTLP/Prometheus export | Current crates are active in 2025-2026. [VERIFIED: crates.io API for tracing, metrics, opentelemetry-otlp] | Phase 7 should emit machine-consumable telemetry, not just log lines. |
| SQLite/mock DB acceptance tests | Containerized PostgreSQL 18 tests | Existing project decision in Phase 2. [VERIFIED: .planning/STATE.md; VERIFIED: crates/es-store-postgres/tests/common/mod.rs] | TEST-02 must exercise PostgreSQL semantics. |

**Deprecated/outdated:**

- Treating disruptor ring results as command durability is forbidden in this project. [VERIFIED: AGENTS.md; VERIFIED: .planning/PROJECT.md]
- Publishing directly to brokers from adapters or command handlers is out of scope and violates the outbox rule. [VERIFIED: .planning/REQUIREMENTS.md; VERIFIED: crates/es-store-postgres/src/sql.rs]
- Single E2E benchmarks as proof of hot-path performance are insufficient for this template. [VERIFIED: .planning/PROJECT.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Metric labels should avoid raw command IDs/correlation IDs/stream IDs due cardinality risk. | Architecture Patterns; Common Pitfalls | If the project wants per-command metrics despite cardinality cost, metric schema must be redesigned and backend capacity confirmed. |
| A2 | Testcontainers should remain default and manual `DATABASE_URL` fallback is optional unless implemented. | Common Pitfalls | If CI cannot run Docker, planner must add a fallback or CI service container task. |
| A3 | It is tempting for adapters to access all app components for convenience. | Common Pitfalls | If planner ignores this human-factor risk, dependency-boundary checks may be weaker. |

## Open Questions

1. **Should Phase 7 upgrade Rust above 1.85?**
   - What we know: Workspace pins `rust-version = "1.85"` and `rustc 1.85.1` is installed locally. [VERIFIED: Cargo.toml; VERIFIED: `rustc --version`]
   - What's unclear: Whether the project wants to unlock Criterion 0.8.2 or sysinfo 0.38.4 in this phase. [VERIFIED: cargo info criterion@0.8.2; VERIFIED: cargo info sysinfo@0.38.4]
   - Recommendation: Do not upgrade Rust in Phase 7; use Criterion 0.7.0 and sysinfo 0.36.1 to keep scope focused. [VERIFIED: cargo info criterion; VERIFIED: cargo info sysinfo]

2. **Should observability export use Prometheus, OTLP, or both?**
   - What we know: `metrics-exporter-prometheus` and OpenTelemetry OTLP crates are current and compatible with Rust 1.85. [VERIFIED: cargo info metrics-exporter-prometheus; VERIFIED: cargo info opentelemetry-otlp]
   - What's unclear: The target deployment backend is not specified. [VERIFIED: .planning/PROJECT.md]
   - Recommendation: Provide Prometheus exporter for local stress/demo and optional OTLP initialization at app boundary. [VERIFIED: cargo info metrics-exporter-prometheus; VERIFIED: cargo info opentelemetry-otlp]

3. **How should the app server/stress runner be invoked?**
   - What we know: `crates/app/src/main.rs` is currently a shell. [VERIFIED: crates/app/src/main.rs]
   - What's unclear: No CLI shape is locked for `serve` vs `stress`. [VERIFIED: .planning/ROADMAP.md]
   - Recommendation: Add minimal app APIs first; add CLI commands only if needed for stress artifact generation. [ASSUMED]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust/Cargo | Build, tests, benches | yes | `rustc 1.85.1`, `cargo 1.85.1` | None needed. [VERIFIED: command output] |
| Docker | Testcontainers PostgreSQL tests | yes | Docker 29.3.1 client; `docker info` reachable | Manual PostgreSQL could be added if Docker unavailable, but not currently implemented. [VERIFIED: command output; VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| PostgreSQL client `psql` | Manual DB debugging only | no | not found | Not required by existing Testcontainers harness. [VERIFIED: command output; VERIFIED: crates/es-store-postgres/tests/common/mod.rs] |
| `cargo-nextest` | Optional faster test runner | no | not found | Use `cargo test`. [VERIFIED: command output] |
| `cargo-llvm-cov` | Optional coverage | no | not found | Use normal tests unless coverage gate is added. [VERIFIED: command output] |
| `cargo-deny` | Optional dependency audit | no | not found | Existing `deny.toml` exists but CLI is absent. [VERIFIED: command output; VERIFIED: deny.toml] |

**Missing dependencies with no fallback:**

- None blocking Phase 7 research or default implementation; Docker is available for required PostgreSQL tests. [VERIFIED: command output]

**Missing dependencies with fallback:**

- `cargo-nextest`, `cargo-llvm-cov`, and `cargo-deny` are absent; use `cargo test` and skip optional coverage/audit automation unless the plan installs these tools. [VERIFIED: command output]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness with Tokio async tests, Testcontainers PostgreSQL, and Criterion 0.7.0 for benches. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/tests/common/mod.rs; VERIFIED: cargo info criterion] |
| Config file | Root `Cargo.toml`; no separate nextest config exists. [VERIFIED: `find . -name nextest.toml`; VERIFIED: Cargo.toml] |
| Quick run command | `cargo test --workspace --no-run` for compile coverage, then targeted package tests. [VERIFIED: command output] |
| Full suite command | `cargo test --workspace -- --nocapture` plus `cargo bench --bench <name>` for benchmark artifacts. [VERIFIED: Cargo.toml; CITED: https://criterion-rs.github.io/book] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| API-01 | HTTP endpoint decodes request, attaches metadata, submits through bounded gateway, awaits reply | integration/unit with fake gateway | `cargo test -p adapter-http commerce_api -- --nocapture` | no - Wave 0 |
| API-02 | Adapter does not import or mutate aggregate cache, projector mutation, or outbox dispatcher state | dependency-boundary test / static grep | `cargo test -p adapter-http dependency_boundaries -- --nocapture` | no - Wave 0 |
| API-03 | Success/error response includes stream revision, global position, correlation ID, typed payload | integration/unit | `cargo test -p adapter-http response_contract -- --nocapture` | no - Wave 0 |
| API-04 | Docs explain WebSocket/gRPC gateways without shared hot state | doc validation / grep | `test -f docs/template-guide.md && rg "WebSocket|gRPC|CommandGateway" docs` | no - Wave 0 |
| OBS-01 | Structured traces include command/correlation/tenant/stream/shard/global fields | unit/integration with tracing subscriber capture | `cargo test -p app observability_traces -- --nocapture` | no - Wave 0 |
| OBS-02 | Metrics expose required queue/latency/conflict/dedupe/lag values | unit/integration with recorder capture | `cargo test -p app observability_metrics -- --nocapture` | no - Wave 0 |
| TEST-02 | Append/OCC/dedupe/snapshot/projector/outbox verified against PostgreSQL | integration | `cargo test -p es-store-postgres -- --nocapture` | yes |
| TEST-03 | Layer benchmark harnesses exist and run separately | bench compile/run | `cargo bench --bench ring_only -- --warm-up-time 1 --measurement-time 3` | no - Wave 0 |
| TEST-04 | Single-service integrated stress reports required metrics | integration/stress | `cargo test -p app single_service_stress_smoke -- --nocapture` | no - Wave 0 |
| DOC-01 | Hot-path rules and template guidance documented | doc validation / grep | `rg "event store is the source of truth|outbox|single-owner|ring-only" docs` | no - Wave 0 |

### Sampling Rate

- **Per task commit:** Run the relevant package tests plus `cargo test --workspace --no-run`. [VERIFIED: command output]
- **Per wave merge:** Run `cargo test --workspace -- --nocapture`; run quick smoke benches if benchmark code changed. [VERIFIED: Cargo.toml]
- **Phase gate:** Full workspace tests green, PostgreSQL integration tests pass with Docker, benchmark/stress artifacts generated or smoke-validated, and docs grep checks pass. [VERIFIED: .planning/REQUIREMENTS.md]

### Wave 0 Gaps

- [ ] `crates/adapter-http/src/commerce.rs` - command DTOs, handlers, metadata/envelope mapping for API-01/API-03. [VERIFIED: crates/adapter-http/src/lib.rs]
- [ ] `crates/adapter-http/src/error.rs` - typed `ApiError`/`IntoResponse` mapping for API-03. [VERIFIED: crates/es-runtime/src/error.rs]
- [ ] `crates/adapter-http/tests/commerce_api.rs` - fake-gateway adapter tests for API-01/API-03. [VERIFIED: crates/adapter-http/src/lib.rs]
- [ ] `crates/adapter-http/tests/dependency_boundaries.rs` - verifies API-02 imports/boundaries. [VERIFIED: .planning/REQUIREMENTS.md]
- [ ] `crates/app/src/observability.rs` - subscriber/exporter setup and metric descriptions for OBS-01/OBS-02. [VERIFIED: crates/app/src/lib.rs]
- [ ] `crates/app/src/stress.rs` - single-service integrated stress core for TEST-04. [VERIFIED: crates/app/src/main.rs]
- [ ] `benches/ring_only.rs`, `benches/domain_only.rs`, `benches/adapter_only.rs`, `benches/storage_only.rs`, `benches/projector_outbox.rs` - TEST-03 artifacts. [VERIFIED: .planning/REQUIREMENTS.md]
- [ ] `docs/hot-path-rules.md`, `docs/template-guide.md`, `docs/stress-results.md` - DOC-01 and API-04 guidance. [VERIFIED: .planning/REQUIREMENTS.md]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no for Phase 7 core template | No auth system is in v1 requirements; keep tenant ID explicit and do not invent authentication. [VERIFIED: .planning/REQUIREMENTS.md] |
| V3 Session Management | no | No browser session requirement exists in Phase 7. [VERIFIED: .planning/REQUIREMENTS.md] |
| V4 Access Control | yes | Scope every command/query by `TenantId`; never let adapter-provided tenant be optional. [VERIFIED: crates/es-core/src/lib.rs; VERIFIED: crates/es-runtime/src/command.rs] |
| V5 Input Validation | yes | Axum DTO validation plus existing domain/core constructors for tenant/stream/quantity IDs. [VERIFIED: crates/example-commerce/src/ids.rs; VERIFIED: crates/es-core/src/lib.rs] |
| V6 Cryptography | yes for ID generation only | Use `uuid` v7 helpers already present; do not hand-roll ID generation or correlation tokens. [VERIFIED: Cargo.toml; VERIFIED: crates/es-store-postgres/src/ids.rs] |

### Known Threat Patterns for Rust/Axum/PostgreSQL Adapter Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cross-tenant command/query access | Information Disclosure / Elevation of Privilege | Require tenant ID in metadata and query predicates; use tenant-scoped storage APIs. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/event_store.rs] |
| Replay/duplicate command submission | Tampering | Require idempotency key and rely on PostgreSQL command dedupe result. [VERIFIED: crates/es-runtime/src/command.rs; VERIFIED: crates/es-store-postgres/src/sql.rs] |
| Overload-induced resource exhaustion | Denial of Service | Tower timeout/load-shed/concurrency limits plus `CommandGateway` bounded ingress. [VERIFIED: crates/es-runtime/src/gateway.rs; CITED: https://docs.rs/tower/0.5.3/tower/struct.ServiceBuilder.html] |
| SQL injection in adapter-added query paths | Tampering | Use existing SQLx parameter binding patterns; do not build SQL strings from DTO fields. [VERIFIED: crates/es-store-postgres/src/sql.rs; VERIFIED: crates/es-store-postgres/src/projection.rs] |
| Sensitive IDs in metrics labels | Information Disclosure / DoS | Put unique IDs in traces/log events and keep metric labels bounded. [ASSUMED] |

## Sources

### Primary (HIGH confidence)

- `AGENTS.md` - project constraints, GSD workflow, hot-path and testing rules. [VERIFIED: AGENTS.md]
- `.planning/REQUIREMENTS.md` - API-01 through DOC-01 requirement definitions. [VERIFIED: .planning/REQUIREMENTS.md]
- `.planning/ROADMAP.md` - Phase 7 goal, success criteria, dependencies. [VERIFIED: .planning/ROADMAP.md]
- `.planning/PROJECT.md` - source-of-truth, single-owner, outbox, stress/documentation constraints. [VERIFIED: .planning/PROJECT.md]
- `Cargo.toml` - workspace Rust 2024/Rust 1.85 and current dependency policy. [VERIFIED: Cargo.toml]
- `crates/es-runtime/src/command.rs`, `gateway.rs`, `engine.rs`, `shard.rs` - bounded ingress, command outcome, runtime ownership. [VERIFIED: local code]
- `crates/es-store-postgres/src/sql.rs`, `event_store.rs`, `projection.rs`, `outbox.rs`, `models.rs` - durable append, global reads, projection/outbox repositories. [VERIFIED: local code]
- `crates/es-store-postgres/tests/common/mod.rs` - Testcontainers PostgreSQL 18 harness. [VERIFIED: local code]
- Cargo registry metadata via `cargo info` and crates.io API for versions, MSRV, and publish/update dates. [VERIFIED: cargo info; VERIFIED: crates.io API]
- Context7 `/websites/rs_axum_0_8_9_axum` - Axum extractors/error handling. [CITED: https://docs.rs/axum/0.8.9]
- Context7 `/websites/rs_tower` - Tower ServiceBuilder middleware. [CITED: https://docs.rs/tower/0.5.3]
- Context7 `/open-telemetry/opentelemetry-rust` - OpenTelemetry metrics and OTLP initialization patterns. [CITED: https://github.com/open-telemetry/opentelemetry-rust]
- Context7 `/metrics-rs/metrics` - metrics facade, histograms, Prometheus exporter. [CITED: https://docs.rs/metrics]
- Context7 `/testcontainers/testcontainers-rs` - PostgreSQL container testing patterns. [CITED: https://github.com/testcontainers/testcontainers-rs]
- Context7 `/bheisler/criterion.rs` - Criterion BenchmarkGroup/Throughput patterns. [CITED: https://criterion-rs.github.io/book]

### Secondary (MEDIUM confidence)

- None needed; research relied on project files, cargo registry metadata, and official/Context7 documentation. [VERIFIED: research log]

### Tertiary (LOW confidence)

- Assumptions about metrics cardinality and human-factor adapter leakage risk are marked in the Assumptions Log. [ASSUMED]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH - current versions and MSRV verified with `cargo info` and crates.io API; compatibility constraints are explicit. [VERIFIED: cargo info; VERIFIED: crates.io API]
- Architecture: HIGH - based on existing crate boundaries and runtime/storage/outbox implementations. [VERIFIED: Cargo.toml; VERIFIED: crates/es-runtime/src; VERIFIED: crates/es-store-postgres/src; VERIFIED: crates/es-outbox/src]
- Pitfalls: MEDIUM - boundary and compatibility pitfalls are verified; metric-cardinality and convenience-leak risks are industry assumptions marked as assumed. [VERIFIED: .planning/PROJECT.md; ASSUMED]
- Validation: HIGH - existing test harness and local `cargo test --workspace --no-run` succeeded. [VERIFIED: command output]

**Research date:** 2026-04-18 [VERIFIED: environment current_date]
**Valid until:** 2026-05-18 for architecture and project-local constraints; recheck crate versions before implementation because Axum/Tokio/Testcontainers were updated in April 2026. [VERIFIED: crates.io API]

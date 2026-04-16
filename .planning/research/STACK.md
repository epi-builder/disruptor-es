# Technology Stack

**Project:** Disruptor Event Sourcing Template
**Research dimension:** Rust stack for a disruptor-based event-sourcing + CQRS command service template
**Researched:** 2026-04-16
**Overall confidence:** HIGH for mainstream Rust/web/storage/observability choices; MEDIUM for the disruptor crate choice because the ecosystem is narrow and must be validated with project-specific benchmarks.

## Recommendation

Build a Rust 2024 workspace around a typed domain kernel, local shard runtimes, PostgreSQL 18 as the durable event store/outbox, `sqlx` for storage, `tokio` for adapter and background I/O, `axum` first for HTTP, `tonic` only when an internal gRPC boundary is needed, and `tracing` + OpenTelemetry for observability.

Use the crate named `disruptor` 4.0.0 for the Disruptor implementation. Its repository is `nicholassm/disruptor-rs`, which is likely what "disruptor-rs" refers to in architecture discussion. Do not default to the crate literally named `disruptor-rs` 0.1.1 for the core template unless a spike proves it has the exact semantics and maintenance profile needed.

The hot path should stay typed, allocation-conscious, and single-owner:

```text
adapter -> bounded command ingress -> partition router -> shard runtime
  -> disruptor ring -> typed decide/apply -> PostgreSQL append transaction
  -> reply after commit
```

Everything slower or externally dependent belongs outside that path:

```text
event store tail/outbox -> projectors -> read models
event store/outbox -> dispatcher -> optional broker adapter
metrics/tracing -> non-blocking export
```

## Recommended Stack

### Core Runtime and Hot Path

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| Rust | Edition 2024; set `rust-version = "1.85"` or higher | Language/toolchain baseline | Rust 2024 is stable from Rust 1.85.0. Use it for a greenfield template; the ecosystem crates checked here require roughly Rust 1.64-1.82+, so 1.85 is a clean floor. |
| `disruptor` | 4.0.0 | In-process low-latency ring sequencing | Best current fit for the requested Disruptor pattern. Use inside shard runtimes only; not as a durable queue, broker, or cross-process bus. |
| `tokio` | 1.52.0 | Async runtime for adapters, storage I/O, projectors, outbox loops | Standard Rust async runtime for `axum`, `tonic`, `sqlx`, and background tasks. Keep domain `decide/apply` sync and deterministic; use Tokio at boundaries. |
| `crossbeam` / `crossbeam-channel` | Check latest during implementation | Low-level bounded queues where async is not needed | Prefer bounded channels between sync runtime components. Use `tokio::sync` channels only across async boundaries. |
| `parking_lot` | Check latest during implementation | Narrow non-hot-path locks | Acceptable for metrics registries or test harness state. Do not use it to hide global aggregate state behind locks. |
| `thiserror` | 2.0.18 | Domain and infrastructure error enums | Typed, cheap error modeling. Use for public domain/storage errors. |
| `anyhow` | Check latest during implementation | CLI/bootstrap/test harness errors | Good at outer application edges. Do not use as domain or event-store API surface. |

### Domain, Serialization, and IDs

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| `serde` | 1.0.228 | Serialize commands/events/snapshots at adapter/storage boundaries | Ubiquitous and compatible with `sqlx` JSON, test fixtures, and integration payloads. Keep typed Rust enums/structs in the hot path. |
| `serde_json` | 1.0.149 | Event metadata and optional JSONB payload format | Best default for a template because it is inspectable and plays well with PostgreSQL JSONB. For a future performance phase, allow a binary codec behind a trait. |
| `uuid` | 1.23.0 latest observed by `cargo search`; local cached info showed 1.20.0 with latest 1.23.0 | Event IDs, command IDs, correlation IDs | Use UUIDv7 for ordered identifiers where generated in Rust. PostgreSQL 18 also has built-in `uuidv7()`, useful when the DB creates IDs. |
| `time` | 0.3.47 latest observed by crate metadata; local cached info showed 0.3.44 with latest 0.3.47 | Timestamps/durations | Prefer over `chrono` for new Rust services. Store DB times as `timestamptz`; use monotonic `Instant` only for latency measurement. |
| `bytes` | Check latest during implementation | Buffer ownership at adapter/broker boundaries | Useful for avoiding unnecessary copies in adapters and publishers. Do not leak raw bytes into domain decisions. |

### Event Store, Projections, and Outbox

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| PostgreSQL | 18.3 current minor as of official support page on 2026-02-26 | Durable event store, snapshots, command deduplication, outbox, projector offsets, read models | Best default for a template: ACID transactions let domain events, stream revision, command dedupe, and outbox rows commit atomically. PostgreSQL 18 adds built-in `uuidv7()` and performance improvements relevant to append-heavy storage. |
| `sqlx` | Recommend 0.8.6 stable; 0.9.0-alpha.1 is newest crate but alpha | Async PostgreSQL access and migrations | Use `sqlx` because it provides compile-time checked SQL without an ORM. Event sourcing wants explicit SQL for append/OCC/outbox transactions. Avoid the 0.9 alpha unless the implementation explicitly accepts alpha risk. |
| `sqlx` migrations | 0.8.6 feature | Schema migration management | Keep event-store schema transparent in repo. Migrations should define `events`, `streams`, `snapshots`, `command_dedup`, `projector_offsets`, `outbox`, and sample read-model tables. |
| `testcontainers` | 0.27.3 latest observed by crate metadata; local cached info showed 0.25.0 with latest 0.27.3 | PostgreSQL integration tests | Use real PostgreSQL for append/OCC/outbox/projector tests. SQLite-only tests will miss transaction and locking behavior. |

Recommended storage posture:

- Write a small project-owned event-store abstraction instead of adopting a generic Rust CQRS framework.
- Use one PostgreSQL transaction for stream version check, event rows, dedupe row/result, and outbox rows.
- Projectors and outbox dispatchers should read committed event/global positions or durable outbox rows, not disruptor sequences.
- Keep payload schema evolution explicit with `event_type`, `schema_version`, `payload`, and `metadata`.

### Adapters and Service Boundary

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| `axum` | 0.8.9 | First HTTP adapter | Modern Tokio-native HTTP stack with Tower middleware. Good for a template because the adapter can stay thin and ergonomic. |
| `tower` | 0.5.3 | Backpressure, timeout, load-shed, middleware traits | Use at ingress edges. Bounded admission belongs here, before commands enter shard queues. |
| `tower-http` | 0.6.8 | HTTP tracing, CORS, compression, request IDs if needed | Useful adapter utilities. Keep it out of domain crates. |
| `tonic` | 0.14.5 | Optional internal gRPC adapter | Add when the template needs strongly typed service-to-service RPC. Do not build gRPC first unless consumers require it. |
| `prost` | Via `tonic` stack | Protobuf codegen | Only for gRPC contracts or broker schemas, not for internal aggregate logic. |
| `clap` | 4.6.1 latest observed by crate metadata; local cached info showed 4.5.61 with latest 4.6.1 | CLI for migrations, replay, projector rebuild, benchmark control | Operational commands are table stakes for an event-sourced template. |
| `config` | 0.15.22 latest observed by crate metadata; local cached info showed 0.15.20 with latest 0.15.22 | Layered config | Good enough for local/template use. Keep runtime-critical knobs explicit and typed. |

### Observability

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| `tracing` | 0.1.44 | Structured spans/events | Standard Rust application instrumentation. Use correlation/causation IDs as span fields. |
| `tracing-subscriber` | Check latest during implementation | Formatting/filtering/export layer | Centralize JSON logs, env filters, and OTLP integration. |
| `opentelemetry` | 0.31.0 | Metrics/traces API | Current Rust OpenTelemetry API. Use for stage latency, queue depth, lag, and trace export. |
| `opentelemetry-otlp` | 0.31.1 | OTLP exporter to collector | Prefer exporting to an OpenTelemetry Collector rather than coupling the service to a vendor. |
| `hdrhistogram` | 7.5.4 | Latency histograms in benchmarks and optional runtime summaries | Required for p95/p99/max accuracy. Average latency is not enough for this project. |

Minimum metrics to bake into the template:

- Adapter admission: accepted, rejected, queue depth, request latency.
- Router/shard: route latency, shard queue depth, ring wait, command decision time.
- Storage: append latency, OCC conflict count, dedupe hits, transaction retries.
- CQRS: projector lag by projector name, rebuild progress, read-model transaction latency.
- Outbox: pending count, oldest pending age, publish latency, attempts, dead-letter count.

### Benchmarks and Testing

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| `criterion` | 0.8.2 | Statistical microbenchmarks | Use for domain-only, serialization, hash/routing, and storage helper benchmarks. |
| `divan` | 0.1.21 | Lightweight benchmark harness | Useful for simple hot-path benchmarks; Criterion remains better for statistically rich reports. |
| `hdrhistogram` | 7.5.4 | Tail-latency recording | Use in full E2E, soak, and burst tests. |
| `proptest` | 1.11.0 latest observed by crate metadata; local cached info showed 1.9.0 with latest 1.11.0 | Property tests for aggregate invariants, routing stability, idempotency | Event-sourced systems benefit from generated command sequences and replay invariants. |
| `insta` | 1.47.2 | Snapshot tests for serialized event fixtures and API responses | Useful for schema/versioning checks. Do not snapshot unstable timestamps/IDs without redaction. |
| `loom` | Check latest during implementation | Concurrency model tests for custom shard/channel code | Use only for small concurrency primitives. Do not attempt to loom-test the entire service. |
| `cargo-nextest` | CLI tool, check latest during implementation | Test runner | Better test isolation/reporting for integration-heavy suites. |
| `cargo-llvm-cov` | CLI tool, check latest during implementation | Coverage | Use for domain kernel and storage contract coverage. |
| `cargo-deny` | CLI tool, check latest during implementation | License/advisory/duplicate dependency checks | Include from day one because this is a reusable template. |

Benchmark layers should be separate targets:

1. `ring_only`: disruptor publish/consume overhead.
2. `domain_only`: command `decide/apply` with in-memory state.
3. `routing_only`: partition hash, bounded admission, reply channel overhead.
4. `storage_only`: append transaction, snapshot load/save, global tail reads.
5. `adapter_only`: HTTP/gRPC decode and bounded send without DB.
6. `full_e2e`: request through durable commit reply.
7. `projector_outbox`: catch-up, rebuild, lag, broker-down behavior.
8. `soak_chaos`: hot keys, burst overload, DB slowdown, projector outage, dispatcher retries.

### Optional Broker Adapter

| Technology | Version/currentness | Purpose | Why |
|------------|---------------------|---------|-----|
| `async-nats` | 0.47.0 latest observed by crate metadata; local cached info showed 0.46.0 with latest 0.47.0 | Optional NATS/JetStream publisher adapter | Good first external publisher because it is pure Rust and operationally lighter than Kafka. Still publish only from the durable outbox. |
| `rdkafka` | 0.39.0 | Optional Kafka/Redpanda publisher adapter | Use when Kafka compatibility is required. It wraps `librdkafka`, so keep it behind a feature flag and do not make it a core template dependency. |

Do not choose a production broker in v1. Define a `Publisher` trait and include a logging/in-memory publisher plus one optional adapter. The outbox contract is the real architectural decision.

## Workspace Layout

Use a Rust workspace that makes hot-path dependencies visible:

```text
crates/
  es-core/              # IDs, metadata, stream/revision types, errors
  es-kernel/            # Aggregate traits, typed command/event contracts
  es-runtime/           # router, shard runtime, disruptor integration
  es-store-postgres/    # sqlx event store, snapshots, dedupe, outbox schema
  es-projection/        # projector traits, checkpoints, rebuild runtime
  es-outbox/            # dispatcher and publisher trait
  example-commerce/     # User/Product/Order fixture domain
  adapter-http/         # axum API, DTO conversion, bounded ingress only
  adapter-grpc/         # optional tonic API
  app/                  # binary composition, config, telemetry
benches/
tests/
migrations/
```

Dependency rule: lower-level crates must not depend on adapters, `axum`, `tonic`, broker crates, or OpenTelemetry exporters. Domain crates may depend on `serde`, `thiserror`, `uuid`, and `time`, but should not depend on `sqlx`, `tokio`, or HTTP/gRPC types.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Disruptor crate | `disruptor` 4.0.0 | `disruptor-rs` 0.1.1 | The literal `disruptor-rs` crate is much less mature by version signal. Use only if a spike finds a required API/semantic advantage. |
| Event store | PostgreSQL + project-owned schema | Generic Rust CQRS/event-sourcing framework crates | Current Rust CQRS crates exist, but this template needs precise transaction, outbox, benchmark, and partition semantics. A small owned abstraction is safer. |
| SQL access | `sqlx` 0.8.6 stable | Diesel/SeaORM | Event store append/OCC wants explicit SQL and async integration. ORMs add abstraction where the design needs exact transaction control. |
| `sqlx` version | 0.8.6 | 0.9.0-alpha.1 | 0.9 alpha is newest but not appropriate as the default for a reusable service template. Recheck before implementation. |
| Primary adapter | `axum` | Actix Web | Axum aligns naturally with Tower/Tokio and keeps adapters thin. Actix is viable but adds a separate framework model without clear benefit here. |
| Internal RPC | Optional `tonic` | gRPC-first service | gRPC is useful for service-to-service contracts, but the v1 template should prove command/runtime/storage first. |
| Broker | Outbox trait + optional NATS | Direct Kafka/NATS publish from command handler | Direct publish reintroduces double-write failure modes and couples broker health to command success. |
| Observability | `tracing` + OpenTelemetry OTLP | Vendor SDK directly | OTLP keeps the template portable and lets operators choose collector/exporter backends. |
| Benchmarking | Separate Criterion/Divan/HdrHistogram suites | One full-stack load test | A single E2E number hides whether the bottleneck is ring, domain, DB, adapter, projection, or broker. |

## Installation Sketch

Use `cargo add` once the workspace exists. Prefer stable versions over alphas unless a phase explicitly validates the upgrade.

```bash
# Core runtime / domain
cargo add disruptor@4 tokio@1 --features tokio/rt-multi-thread,tokio/macros,tokio/sync,tokio/time
cargo add serde serde_json thiserror anyhow uuid time bytes

# Storage
cargo add sqlx@0.8.6 --features runtime-tokio-rustls,postgres,uuid,time,json,migrate,macros

# HTTP / optional gRPC
cargo add axum@0.8 tower@0.5 tower-http@0.6
cargo add tonic@0.14 --optional

# Observability
cargo add tracing tracing-subscriber opentelemetry@0.31 opentelemetry-otlp@0.31 hdrhistogram

# Optional publishers
cargo add async-nats --optional
cargo add rdkafka@0.39 --optional

# Dev dependencies
cargo add --dev criterion@0.8 divan@0.1 proptest insta testcontainers hdrhistogram
```

## Confidence Notes

| Area | Confidence | Notes |
|------|------------|-------|
| Rust/Tokio/Axum/Tower stack | HIGH | Current crate metadata confirms active stable versions and aligned ecosystem. |
| PostgreSQL + `sqlx` event store | HIGH | PostgreSQL official docs confirm 18.3 as current supported minor; `sqlx` is the right fit for explicit async SQL. |
| `disruptor` crate choice | MEDIUM | Metadata favors `disruptor` 4.0.0 over `disruptor-rs` 0.1.1, but the project must validate wait strategies, producer/consumer model, and allocation behavior with ring-only benchmarks. |
| Generic event-sourcing framework rejection | MEDIUM | Based on crate discovery and project requirements. Revalidate only if roadmap later shifts from a template/kernel project to a faster application prototype. |
| Broker recommendation | MEDIUM | Broker choice is intentionally deferred; outbox interface is high-confidence, specific broker adapter is workload-dependent. |

## Sources

- Rust 2024 Edition Guide: https://doc.rust-lang.org/edition-guide/rust-2024/index.html
- Rust 1.85.0 release / Rust 2024 stabilization: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/
- PostgreSQL versioning policy and current supported releases: https://www.postgresql.org/support/versioning/
- PostgreSQL 18 release notes/news: https://www.postgresql.org/about/news/postgresql-18-released-3142/
- `disruptor` crate metadata/docs: https://crates.io/crates/disruptor and https://docs.rs/disruptor/4.0.0
- `disruptor-rs` crate metadata/docs: https://crates.io/crates/disruptor-rs and https://docs.rs/disruptor-rs
- `tokio` crate metadata/docs: https://crates.io/crates/tokio and https://docs.rs/tokio/1.52.0
- `sqlx` crate metadata/docs: https://crates.io/crates/sqlx and https://docs.rs/sqlx/0.8.6
- `axum` crate metadata/docs: https://crates.io/crates/axum and https://docs.rs/axum/0.8.9
- `tower` crate metadata/docs: https://crates.io/crates/tower and https://docs.rs/tower/0.5.3
- `tonic` crate metadata/docs: https://crates.io/crates/tonic and https://docs.rs/tonic/0.14.5
- `tracing` crate metadata/docs: https://crates.io/crates/tracing and https://docs.rs/tracing/0.1.44
- `opentelemetry` crate metadata/docs: https://crates.io/crates/opentelemetry and https://docs.rs/opentelemetry/0.31.0
- `opentelemetry-otlp` crate metadata/docs: https://crates.io/crates/opentelemetry-otlp and https://docs.rs/opentelemetry-otlp/0.31.1
- `criterion`, `divan`, `hdrhistogram`, `proptest`, `testcontainers`, `insta` crate metadata from `cargo search` / `cargo info` on 2026-04-16.

# Phase 07 Source Audit

SOURCE | ID | Feature/Requirement | Plan | Status | Notes
--- | --- | --- | --- | --- | ---
GOAL | - | Template is usable from thin HTTP boundaries, observable under load, verified against real storage paths, benchmarked by layer, and documented with architecture rules | 07-01, 07-02, 07-03, 07-04, 07-05, 07-06 | COVERED | Each goal clause maps to a focused plan.
REQ | API-01 | Thin HTTP adapter decodes requests, attaches metadata, sends through bounded ingress, and awaits command replies | 07-01 | COVERED | Commerce routes use `CommandEnvelope` and `CommandGateway::try_submit`.
REQ | API-02 | Adapter code does not mutate aggregate, projector, or outbox state directly | 07-01 | COVERED | Dependency and source boundary tests are required.
REQ | API-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads | 07-01 | COVERED | `CommandSuccess` and `ApiError` contract required.
REQ | API-04 | Documentation explains WebSocket/gRPC gateway boundaries | 07-06 | COVERED | `docs/template-guide.md` has explicit gateway sections.
REQ | OBS-01 | Structured traces include command identity, tenant, stream, shard, and global position where available | 07-02 | COVERED | Runtime/storage spans and app subscriber setup planned.
REQ | OBS-02 | Metrics expose ingress/shard depth, ring wait, decision/append latency, conflicts, dedupe, lag, and p95/p99 latency | 07-02, 07-05 | COVERED | Metric catalog and stress report fields cover runtime and summary metrics.
REQ | TEST-02 | PostgreSQL integration tests verify append, conflicts, dedupe, snapshots, projector checkpoints, and outbox dispatch | 07-03 | COVERED | New `phase7_integration.rs` test file planned.
REQ | TEST-03 | Benchmarks separate ring, domain, adapter, storage, integrated, full E2E, projector/outbox, hot-key, burst, degraded dependencies | 07-04, 07-05 | COVERED | Layer bench files cover ring/domain/adapter/storage/projector/outbox; stress scenarios cover integrated/full E2E/hot-key/burst/degraded-dependency behavior.
REQ | TEST-04 | Single-service integrated stress reports throughput, p50/p95/p99, depths, append latency, lag, rejects, CPU/core | 07-05 | COVERED | `StressReport` requires exact fields.
REQ | DOC-01 | Documentation states hot-path rules, forbidden patterns, service-boundary guidance, and new-domain guidance | 07-06 | COVERED | Three docs planned.
RESEARCH | HTTP adapter uses Axum/Tower and remains thin over runtime gateway | 07-01 | COVERED | Workspace deps and router/handler tasks use Axum/Tower.
RESEARCH | Avoid latest Criterion/sysinfo versions that exceed Rust 1.85 | 07-01, 07-04, 07-05 | COVERED | Plan pins `criterion = "0.7.0"` and `sysinfo = "0.36.1"`.
RESEARCH | Use metrics facade and app-level OpenTelemetry/Prometheus setup | 07-02 | COVERED | `observability.rs` owns exporters and lower crates use `metrics`.
RESEARCH | Keep metric labels bounded and avoid sensitive ID leakage | 07-02 | COVERED | Forbidden label tests required.
RESEARCH | Reuse existing PostgreSQL Testcontainers harness | 07-03 | COVERED | Plan requires `common::start_postgres`.
RESEARCH | Separate Criterion layer benches from integrated stress runner | 07-04, 07-05, 07-06 | COVERED | Layer bench artifacts, stress scenarios, and docs explicitly separate meanings.
CONTEXT | - | No Phase 7 CONTEXT.md exists | - | COVERED | No locked discuss decisions to implement; distributed ownership remains excluded by the roadmap.

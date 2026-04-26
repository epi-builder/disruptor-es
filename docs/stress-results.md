# Stress Results

Use this guide when recording or reviewing benchmark and stress output from this template. Keep raw results labeled by layer so a local handoff benchmark is not confused with production-shaped service behavior.

## Ring-Only Benchmarks

Ring-only microbenchmarks measure local disruptor handoff cost, not service throughput.

The `ring_only` bench exercises `DisruptorPath` publication and polling. It intentionally avoids adapter DTO work, domain `decide`/`apply`, storage append, projection catch-up, outbox dispatch, query waits, and network overhead.

Use ring-only numbers to compare local ring configuration, event shape, wait strategy assumptions, and hot-key handoff behavior. Do not use them as HTTP, WebSocket, gRPC, command-engine, PostgreSQL, projector, or outbox throughput numbers.

## Layer Benchmarks

Layer benchmarks isolate one cost center at a time:

- `domain_only` measures synchronous commerce aggregate `decide`/`apply` behavior in memory.
- `adapter_only` measures HTTP-shaped DTO decode, `CommandEnvelope` creation, and bounded `CommandGateway` admission.
- `storage_only` measures PostgreSQL event-store operations against an explicit `DATABASE_URL`.
- `projector_outbox` measures PostgreSQL projector catch-up plus durable outbox claim and publish behavior with a PostgreSQL 18 Testcontainers harness.

These benchmarks are diagnostic. Use them to identify which layer changed after a code or schema edit. They are not a replacement for single-service integrated stress.

## In-Process Integrated Stress

In-process integrated stress includes adapter DTO work, bounded ingress, runtime execution, append behavior, projection lag, and outbox lag in one process.

The `app stress-smoke` path drives the production-shaped composition in-process: bounded `CommandGateway`, `CommandEngine`, shard execution, PostgreSQL event store, projection store, outbox store, and lag sampling. Command success is counted from durable command replies after append. Projection and outbox lag are sampled after command replies so they remain visible without becoming command success gates.

In-process integrated stress is the right local signal for template shape, queue pressure, reject behavior, and whether single-owner hot state plus durable append still fit together under realistic load.

## External-Process HTTP Stress And Benchmark

External-process HTTP measurements launch the real `app serve` binary, then submit canonical order commands over HTTP from outside that process. This is the archive-facing lane for client plus service-process overhead.

Use:

- `cargo run -p app -- http-stress`
- `cargo bench --bench external_process_http -- --sample-size 10`

These runs are intentionally separate from `app stress-smoke`. The external-process lane reuses the same required report fields and adds real HTTP client, process boundary, socket, and server bootstrap overhead. Record `throughput_per_second`, `p50_micros`, `p95_micros`, `p99_micros`, `ingress_depth_max`, `shard_depth_max`, `append_latency_p95_micros`, `projection_lag`, `outbox_lag`, `reject_rate`, `cpu_utilization_percent`, and `core_count` alongside host, commit, and configuration metadata.

## Runnable HTTP Service vs. In-Process Stress

`app serve` is now the official executable HTTP service path. It boots the real `adapter_http::router(HttpState)` surface, exposes `/healthz`, and is the process Phase 12 should drive for external-process smoke, E2E, stress, and benchmark work.

`app stress-smoke` is still valuable, but it is **not** the same thing as `app serve`:

- `app serve` = long-lived HTTP server process used for readiness probes and real-process client traffic
- `app stress-smoke` = in-process integrated harness used to measure production-shaped composition without external HTTP client/process overhead

Keep these labels explicit in reports so in-process results are not presented as external-process HTTP end-to-end measurements.

## Required Report Fields

Every in-process integrated or external-process HTTP stress report should include these fields:

- `throughput_per_second`
- `p50_micros`
- `p95_micros`
- `p99_micros`
- `ingress_depth_max`
- `shard_depth_max`
- `append_latency_p95_micros`
- `projection_lag`
- `outbox_lag`
- `reject_rate`
- `cpu_utilization_percent`
- `core_count`

Also record scenario name, command count, concurrency, shard count, ingress capacity, ring size, tenant count, host details, commit hash, and any database or container settings used for the run.

## Reading Projection And Outbox Lag

`projection_lag` is the distance between committed event-store global positions and the projector offset observed during the run. It describes read-model freshness, not whether commands succeeded.

`outbox_lag` is pending durable publication work after command append. It describes external integration delay, not whether domain events were committed.

Lag can rise while command throughput remains healthy. That means the hot command path is accepting and appending work faster than projectors or publishers are catching up. Tune projector batches, outbox dispatcher limits, publisher retries, database indexes, or deployment split before changing command success semantics.

## Do Not Compare

Do not compare:

- `ring_only` operations per second against `throughput_per_second` from single-service integrated stress.
- `domain_only` `decide`/`apply` latency against HTTP or gRPC request latency.
- `adapter_only` bounded ingress results against PostgreSQL append throughput.
- `storage_only` append latency against projector/outbox dispatch latency.
- Projection catch-up completion against command success.
- Outbox publish completion against command success.

If a result set needs a headline, label it with the scenario and layer: ring-only handoff, domain-only decision, adapter-only admission, storage-only append, projector/outbox dispatch, or single-service integrated stress.

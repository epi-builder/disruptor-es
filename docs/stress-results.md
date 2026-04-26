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

## Layer Comparison

Phase 13.1 adds one repeatable comparison entrypoint:

```bash
PHASE13_1_COMPARE_MODE=smoke bash scripts/compare-stress-layers.sh
PHASE13_1_COMPARE_MODE=baseline bash scripts/compare-stress-layers.sh
```

The script writes these fixed outputs under `target/phase-13.1/layer-comparison`:

- `ring-only.txt`
- `adapter-only.txt`
- `storage-only.txt`
- `in-process-runtime.json`
- `live-http-unique.json`
- `live-http-single-hot-key.json`

Interpret the outputs by layer:

- `ring-only.txt` measures local disruptor handoff cost only and must not be read as durable-service throughput.
- `adapter-only.txt` measures DTO decode, `CommandEnvelope` construction, and bounded ingress admission without durable append.
- `storage-only.txt` measures PostgreSQL event-store operations without HTTP adapter overhead.
- `in-process-runtime.json`, `live-http-unique.json`, and `live-http-single-hot-key.json` include durable PostgreSQL append, HTTP or runtime overhead, and bounded admission behavior on the command path.

Projector and outbox lag stay observational in these command-path runs. If an operator wants an active projection or outbox bottleneck lane, that extra projector/outbox pressure must be enabled separately instead of inferred from the default comparison outputs.

For live HTTP comparison review, require these report keys:

- `throughput_per_second`
- `p50_micros`
- `p95_micros`
- `p99_micros`
- `max_micros`
- `commands_rejected`
- `commands_failed`
- `ingress_depth_max`
- `shard_depth_max`
- `append_latency_p95_micros`
- `ring_wait_p95_micros`
- `metrics_scrape_successes`
- `metrics_scrape_failures`
- `metrics_sample_count`
- `workload_shape`
- `hot_set_size`

Explain the throughput ceiling in terms of the slowest measured layer: durable PostgreSQL append, HTTP overhead, bounded admission, or command-path side effects. Do not attribute every shortfall to the disruptor ring when the slower lane is elsewhere.

## In-Process Integrated Stress

In-process integrated stress includes adapter DTO work, bounded ingress, runtime execution, append behavior, projection lag, and outbox lag in one process.

The `app stress-smoke` path drives the production-shaped composition in-process: bounded `CommandGateway`, `CommandEngine`, shard execution, PostgreSQL event store, projection store, outbox store, and lag sampling. Command success is counted from durable command replies after append. Projection and outbox lag are sampled after command replies so they remain visible without becoming command success gates.

In-process integrated stress is the right local signal for template shape, queue pressure, reject behavior, and whether single-owner hot state plus durable append still fit together under realistic load.

## External-Process HTTP Stress And Benchmark

External-process HTTP measurements launch the real `app serve` binary, then submit canonical order commands over HTTP from outside that process. Phase 13 splits this lane into one authoritative steady-state runner and one secondary Criterion smoke wrapper.

### Phase 13 Steady-State Live HTTP

`app http-stress` is the archive-facing steady-state lane for sustained live-service throughput and latency claims. Supported profiles are:

- `smoke`
- `baseline`
- `burst`
- `hot-key`

Use:

- `cargo run -p app -- http-stress --profile smoke`
- `cargo run -p app -- http-stress --profile baseline --warmup-seconds 5 --measure-seconds 30 --concurrency 8`
- `cargo run -p app -- http-stress --profile burst`
- `cargo run -p app -- http-stress --profile hot-key`

The runner starts `app serve` once, then performs readiness, warmup, and measurement on that same process. PostgreSQL container startup, migrations, readiness probing, binary compilation, and warmup traffic are outside the measured window. Only the measured window contributes to throughput, latency, reject, lag, and resource counters.

Measured-window deadline semantics are fixed for comparability: the runner stops submitting at the measured deadline, drains in-flight work for up to 5 seconds, and reports that policy through `deadline_policy` and `drain_timeout_seconds`.

### Phase 13.1 Result

See [13.1-03-SUMMARY.md](/Users/epikem/dev/projects/disruptor-es/.planning/phases/13.1-disruptor-throughput-bottleneck-investigation-and-runtime-st/13.1-03-SUMMARY.md) for the archived layer comparison, shard-count evidence, and the current dominant throughput ceiling classification.

Required steady-state output fields:

- `throughput_per_second`
- `p50_micros`
- `p95_micros`
- `p99_micros`
- `max_micros`
- `commands_succeeded`
- `commands_rejected`
- `commands_failed`
- `reject_rate`
- `append_latency_p95_micros`
- `ingress_depth_max`
- `shard_depth_max`
- `projection_lag`
- `outbox_lag`
- `cpu_utilization_percent`
- `core_count`
- `profile_name`
- `warmup_seconds`
- `measurement_seconds`
- `run_duration_seconds`
- `concurrency`
- `deadline_policy`
- `drain_timeout_seconds`
- `host_os`
- `host_arch`
- `cpu_brand`

### Criterion Smoke / Baseline Comparison

`cargo bench --bench external_process_http -- --sample-size 10` remains available for short smoke or baseline comparisons, but it is not the authoritative Phase 13 steady-state report. Criterion still includes iteration-model behavior, and the older Phase 12 benchmark lane may also surface build or startup effects depending on local state. Use Phase 13 steady-state JSON as the live-service measurement source when recording sustained throughput and latency evidence.

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

For Phase 13 steady-state live HTTP runs, also record `profile_name`, `warmup_seconds`, `measurement_seconds`, `run_duration_seconds`, `concurrency`, `deadline_policy`, `drain_timeout_seconds`, `host_os`, `host_arch`, and `cpu_brand` alongside commit hash and any database or container settings used for the run.

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

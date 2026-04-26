---
phase: 13-live-external-process-http-steady-state-stress-testing
reviewed: 2026-04-26T06:31:47Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - benches/external_process_http.rs
  - crates/app/src/http_stress.rs
  - crates/app/src/main.rs
  - crates/app/src/stress.rs
  - docs/stress-results.md
  - docs/template-guide.md
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 13: Code Review Report

**Reviewed:** 2026-04-26T06:31:47Z
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Reviewed the Phase 13 external-process HTTP stress lane, its CLI/report shell, and the paired documentation. The main problems are in the steady-state measurement semantics inside `crates/app/src/http_stress.rs`: the measured run reuses warmup request identities, the metric sampler can stop before measurement ends when warmup is long, and the reported append p95 is cumulative across warmup despite the docs claiming measurement-only numbers.

## Warnings

### WR-01: Warmup Reuses The Same Request Identities As Measurement

**File:** `crates/app/src/http_stress.rs:300-318`
**Issue:** `run_external_process_http_stress` executes warmup and measurement against the same `app serve` process, and each `execute_http_window` starts `next_index` at `0`. Because `canonical_place_order_request("external-http-stress", next_index)` derives stable `idempotency_key`, `command_id`, `correlation_id`, and entity IDs from that index, the measured window repeats the same commands already sent during warmup. That turns a large part of the measured run into replay/dedup traffic instead of fresh durable appends, which makes the archive-facing throughput, reject, lag, and append-latency numbers materially misleading.
**Fix:**
```rust
let warmup_submitted = execute_http_window(
    &harness,
    &config,
    Duration::from_secs(config.warmup_seconds),
    None,
    false,
    0,
).await?;

let counters = execute_http_window(
    &harness,
    &config,
    Duration::from_secs(config.measurement_seconds),
    Some(&mut latency),
    true,
    warmup_submitted.commands_submitted,
).await?;
```
Update `execute_http_window` to accept a starting index, or generate fresh UUID/idempotency values per window.

### WR-02: Metric Sampling Deadline Is Anchored Before Warmup

**File:** `crates/app/src/http_stress.rs:293-298`
**Issue:** The sampler starts before warmup and `spawn_metric_sampler` uses `measurement_seconds.max(1) + 10` as its total lifetime. With the allowed bounds (`warmup_seconds` up to 600), any warmup longer than 10 seconds causes the sampler to stop before the measured window finishes. The JSON report can then understate `ingress_depth_max`, `shard_depth_max`, `projection_lag`, `outbox_lag`, and CPU samples for exactly the high-duration runs the CLI accepts.
**Fix:**
```rust
let sampler = spawn_metric_sampler(
    harness.client.clone(),
    harness.prometheus_addr,
    measured.clone(),
    config.warmup_seconds + config.measurement_seconds + 10,
);
```
Preferably, start the sampler after warmup or pass separate warmup/measurement durations so its deadline is tied to the real run lifecycle.

### WR-03: `append_latency_p95_micros` Includes Warmup Traffic

**File:** `crates/app/src/http_stress.rs:565-599`
**Issue:** The report claims warmup is excluded from measured results, but `append_latency_p95_micros` is derived from Prometheus histogram buckets scraped from the child process. Those buckets are cumulative for the life of `app serve`, and `reset_measured_state` only clears local aggregates, not the child histogram. As a result, warmup appends still affect the reported p95, so the JSON is not actually measurement-only for one of its required steady-state fields.
**Fix:**
```rust
let baseline = scrape_metrics(&client, prometheus_addr).await?;
// run measured window
let after = scrape_metrics(&client, prometheus_addr).await?;
let append_latency_p95_micros =
    histogram_p95_delta(&baseline_body, &after_body, "es_append_latency_seconds", Some(("outcome", "committed")))?;
```
Either subtract a post-warmup baseline from the histogram buckets, expose a resettable benchmark metric namespace, or move append latency collection into a measurement-scoped recorder instead of reading cumulative process metrics.

---

_Reviewed: 2026-04-26T06:31:47Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

# Phase 13: Live External-Process HTTP Steady-State Stress Testing - Research

**Researched:** 2026-04-26
**Domain:** Live-service HTTP stress measurement, steady-state workload control, and report fidelity for the runnable Rust service
**Confidence:** HIGH

## User Constraints

### Locked Phase Scope
- Phase 13 exists to add a long-lived `app serve` HTTP stress lane that keeps one service process alive across warmup and measurement windows and excludes startup/setup cost from the measured interval. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: user prompt]
- The phase must satisfy `TEST-03`, `TEST-04`, and `OBS-02` without conflating the new lane with Criterion microbenchmarks or the older in-process integrated stress path. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: user prompt]
- The report must include throughput, p50/p95/p99/max latency, success/error/reject counts, reject rate, append latency, ingress depth, shard depth, projection lag, outbox lag, CPU/core count, run duration, concurrency, and environment metadata. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: user prompt]
- The lane must support at least smoke, baseline, burst, and hot-key traffic profiles. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: user prompt]

### Project Constraints
- The event store remains the source of truth; this phase may observe lag and append metrics, but it must not treat in-memory runtime state as durable measurement truth. [VERIFIED: AGENTS.md] [VERIFIED: .planning/REQUIREMENTS.md]
- The official external-process boundary is `app serve`; Phase 13 should reuse the existing runnable-service and external-process harness work instead of composing a second service path. [VERIFIED: .planning/phases/11-evidence-recovery-and-runnable-http-service/11-02-SUMMARY.md] [VERIFIED: .planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-01-SUMMARY.md] [VERIFIED: crates/app/src/main.rs]
- The repository already distinguishes ring-only, in-process integrated, and external-process HTTP results in docs; Phase 13 should sharpen that separation rather than replace prior lanes. [VERIFIED: docs/stress-results.md] [VERIFIED: docs/template-guide.md]
- There is no phase-local `13-CONTEXT.md` yet, so Phase 13 research is constrained by the roadmap, state, requirements, and the user’s explicit success criteria rather than a prior discuss artifact. [VERIFIED: init phase-op 13 output] [VERIFIED: .planning/STATE.md]

### Deferred Ideas
- Final milestone debt closure and archive sign-off remain Phase 14 work, not Phase 13 scope. [VERIFIED: .planning/ROADMAP.md]
- This phase should not expand into distributed load generation, broker benchmarking, or production deployment automation. [VERIFIED: .planning/ROADMAP.md] [ASSUMED]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TEST-03 | Benchmark harnesses separately measure ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded dependency scenarios. [VERIFIED: .planning/REQUIREMENTS.md] | Keep the existing Criterion and in-process lanes, but add a duration-driven steady-state HTTP lane as a separate executable/reporting path with explicit smoke, baseline, burst, and hot-key profiles. [VERIFIED: docs/stress-results.md] [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| TEST-04 | A single-service integrated stress test reports throughput, latency, depth, lag, reject rate, and CPU/core signals under realistic traffic. [VERIFIED: .planning/REQUIREMENTS.md] | Phase 13 should mirror the report shape already used by the repo, but measure it against a long-lived external `app serve` process after warmup rather than during startup. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| OBS-02 | Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, OCC conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency. [VERIFIED: .planning/REQUIREMENTS.md] | The external-process lane should keep scraping Prometheus for queue/lag/append signals and continue reporting them alongside HTTP latency percentiles and reject counts. [VERIFIED: crates/app/src/observability.rs] [VERIFIED: crates/app/src/http_stress.rs] |

</phase_requirements>

## Summary

Phase 12 already built the right substrate for this phase: a real `app serve` child-process harness, canonical request fixtures, Prometheus scraping, and explicit external-process labels. The remaining gap is measurement shape, not process realism. The current `run_external_process_http_stress` path starts timing immediately after harness spawn and measures a finite `command_count`, so PostgreSQL container startup, migration, readiness probing, binary compilation fallback, and first-connection effects can leak into the reported number set. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: .planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-02-SUMMARY.md]

The standard implementation pattern for this phase is a duration-driven live lane inside `crates/app/src/http_stress.rs`: spawn once, warm up, reset counters/histograms, measure for a fixed duration at a selected concurrency/profile, then emit one JSON report. Keep Criterion only for microbenchmarks or a tiny external-process smoke bench; do not use it as the authoritative steady-state report because Criterion intentionally performs its own warmup and per-iteration sampling loop, which is a poor fit for “one long-lived server, one measured window” semantics. [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] [CITED: https://criterion-rs.github.io/book/user_guide/command_line_output.html] [VERIFIED: benches/external_process_http.rs]

**Primary recommendation:** implement Phase 13 as a reusable duration-window HTTP stress runner in `app` code, with profile presets, explicit warmup/measurement separation, reused `reqwest::Client`, interval-based metric sampling, and machine-readable environment metadata in the final report. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] [CITED: https://docs.rs/tokio/latest/tokio/time/struct.Interval.html]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Long-lived service process lifecycle | `crates/app/src/http_stress.rs` | existing child-process harness helpers | Reuse the real `app serve` bootstrap and keep process ownership in app-level code. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/app/src/main.rs] |
| Warmup / measurement / cooldown windowing | `crates/app/src/http_stress.rs` | `tokio::time` interval/timers | This is the missing behavior in the current Phase 12 lane. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/tokio/latest/tokio/time/struct.Interval.html] |
| Canonical traffic profile presets | `crates/app/src/http_stress.rs` | `crates/app/src/stress.rs` profile naming precedent | Keep profile naming aligned with existing burst/hot-key semantics. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: user prompt] |
| Depth/lag/append signal collection | Prometheus scrape path | `app::observability` metric catalog | Preserve the current external-process metric source rather than reaching into runtime internals. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/app/src/observability.rs] |
| Benchmark-lane separation | docs + CLI surface | workspace benches | The live lane should be documented and runnable separately from Criterion benches. [VERIFIED: docs/stress-results.md] [VERIFIED: docs/template-guide.md] |

## Standard Stack

### Core

| Library / Crate | Version | Purpose | Why Standard |
|-----------------|---------|---------|--------------|
| `app` crate | workspace `0.1.0` [VERIFIED: crates/app/Cargo.toml] | Own the steady-state runner, profile presets, JSON report, and CLI entrypoint. | The repo already centralizes external-process HTTP logic here. [VERIFIED: crates/app/src/http_stress.rs] |
| `reqwest` | workspace pin `0.12.24`; latest observed `0.12.28` [VERIFIED: Cargo.toml] [VERIFIED: cargo info reqwest --locked] | Reused async HTTP client for sustained load. | Official docs state `Client` holds a connection pool internally and should be reused. [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] |
| `tokio` | workspace `1.52.0` [VERIFIED: Cargo.toml] | Timer windows, sampling tasks, concurrent request submission, child-process coordination. | Already enabled with `time`, `net`, and `signal` features in the workspace. [VERIFIED: Cargo.toml] |
| `hdrhistogram` | workspace `7.5.4`; latest observed `7.5.4` [VERIFIED: Cargo.toml] [VERIFIED: cargo info hdrhistogram --locked] | Accurate p50/p95/p99/max latency reporting for the measured window. | The repo already uses it for stress percentile output; keep one percentile implementation. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/src/http_stress.rs] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `sysinfo` | workspace `0.36.1`; latest observed `0.36.1` for lockfile query, newer crate exists `0.38.4` [VERIFIED: Cargo.toml] [VERIFIED: cargo info sysinfo --locked] | CPU utilization and logical core count in the final report. | Reuse a long-lived `System` instance and respect `MINIMUM_CPU_UPDATE_INTERVAL` during sampling. [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html] |
| `metrics-exporter-prometheus` | workspace `0.18.1`; latest observed `0.18.1` [VERIFIED: Cargo.toml] [VERIFIED: cargo info metrics-exporter-prometheus --locked] | `/metrics` scrape endpoint for queue depth, lag, and append histogram signals. | Keep this as the external-process observation surface instead of adding a second metrics path. [VERIFIED: crates/app/src/observability.rs] |
| `testcontainers` | workspace `0.25.0`; latest observed `0.25.0` for lockfile query, newer crate exists `0.27.3` [VERIFIED: Cargo.toml] [VERIFIED: cargo info testcontainers --locked] | PostgreSQL container lifecycle for local live runs. | Continue using readiness-aware startup rather than fixed sleeps. [CITED: https://rust.testcontainers.org/features/wait_strategies/] [VERIFIED: crates/app/src/http_stress.rs] |
| Criterion | workspace `0.7.0`; latest observed `0.7.0` for lockfile query, newer crate exists `0.8.2` [VERIFIED: Cargo.toml] [VERIFIED: cargo info criterion --locked] | Keep a tiny external-process smoke bench if needed. | Use it only for benchmark registration/smoke, not as the authoritative steady-state lane. [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] [VERIFIED: benches/external_process_http.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Duration-driven runner in `app` code | Keep measuring a finite `command_count` from process spawn | Simpler, but it preserves the exact startup contamination this phase exists to remove. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: .planning/ROADMAP.md] |
| Reused `reqwest::Client` | New client per request/task | Defeats connection pooling and exaggerates setup overhead in latency numbers. [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] |
| Prometheus scrape for lag/depth signals | Reach into runtime internals from the harness | Violates the external-process boundary and makes the live lane less representative. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: .planning/REQUIREMENTS.md] |
| Live CLI/test lane | Criterion as the only steady-state driver | Criterion performs its own warmup/sampling loop; that is useful for microbenching, not for one explicit measured service window. [CITED: https://criterion-rs.github.io/book/user_guide/command_line_output.html] [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] |

**Installation / existing deps:**
```bash
cargo test -p app external_process_http_stress_smoke -- --nocapture
cargo run -p app -- http-stress
```

**Version verification:** workspace pins were checked in `Cargo.toml`, and crate currentness was checked with `cargo info` on 2026-04-26. [VERIFIED: Cargo.toml] [VERIFIED: cargo info criterion --locked] [VERIFIED: cargo info hdrhistogram --locked] [VERIFIED: cargo info reqwest --locked] [VERIFIED: cargo info sysinfo --locked] [VERIFIED: cargo info metrics-exporter-prometheus --locked] [VERIFIED: cargo info testcontainers --locked]

## Architecture Patterns

### Recommended Project Structure

```text
crates/app/src/
├── http_stress.rs        # external-process harness, profiles, and steady-state runner
├── main.rs               # thin CLI dispatch for serve/stress commands
└── stress.rs             # existing in-process integrated stress lane

docs/
├── stress-results.md     # layer interpretation and report-field guidance
└── template-guide.md     # runnable commands and operator guidance
```

### Pattern 1: Three-phase live run lifecycle

**What:** Run `spawn -> warmup -> reset metrics/histograms -> measured window -> report` in one child-process lifetime. [VERIFIED: user prompt] [VERIFIED: .planning/ROADMAP.md]

**When to use:** Always for the new steady-state lane; never start the measured clock before readiness and warmup complete. [VERIFIED: user prompt] [VERIFIED: crates/app/src/http_stress.rs]

**Example:**
```rust
// Source: repo pattern + Tokio interval docs
let harness = ExternalProcessHarness::spawn(&config).await?;
run_profile_window(&harness, &config.warmup).await?;
let mut stats = MeasurementState::new();
stats.reset_after_warmup();
run_profile_window(&harness, &config.measurement).await?;
let report = stats.finish();
```
[VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/tokio/latest/tokio/time/struct.Interval.html]

### Pattern 2: Duration-based request production, not count-only submission

**What:** Drive requests until a wall-clock deadline using bounded concurrency, and record only outcomes that finish inside the measured window or a clearly documented drain policy. [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] [ASSUMED]

**When to use:** For baseline, burst, and hot-key profiles where concurrency and time matter more than a fixed total request count. [VERIFIED: user prompt]

**Example:**
```rust
// Source: Criterion iter_custom rationale + repo stress semantics
let deadline = Instant::now() + measurement_duration;
while Instant::now() < deadline {
    submit_one_more_request_if_slot_available().await;
}
await drain_inflight_with_timeout().await?;
```
[CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] [ASSUMED]

### Pattern 3: Reused client and sampler tasks

**What:** Create one `reqwest::Client` for the run and keep separate periodic sampler tasks for Prometheus metrics and host CPU state. [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]

**When to use:** Always; repeated client construction or one-shot CPU reads distort latency and resource numbers. [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]

**Example:**
```rust
// Source: reqwest client docs + sysinfo docs
let client = reqwest::Client::builder().build()?;
let mut system = sysinfo::System::new_all();
let mut ticker = tokio::time::interval(sample_period);
ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
```
[CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html] [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]

### Pattern 4: Profile presets with one report schema

**What:** Encode `smoke`, `baseline`, `burst`, and `hot-key` as preset config builders that all emit the same JSON schema. [VERIFIED: user prompt] [VERIFIED: crates/app/src/stress.rs]

**When to use:** Always; planners and docs should vary input profile, not invent a new report shape per scenario. [VERIFIED: user prompt]

**Example:**
```rust
pub fn smoke() -> Self { /* short warmup + short measurement */ }
pub fn baseline() -> Self { /* moderate duration + stable concurrency */ }
pub fn burst() -> Self { /* high concurrency spike */ }
pub fn hot_key() -> Self { /* narrow key spread */ }
```
[VERIFIED: crates/app/src/stress.rs] [ASSUMED]

### Anti-Patterns to Avoid
- **Measuring from child-process spawn:** this mixes container boot, migrations, readiness polling, and first-request effects into “steady-state” latency. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: .planning/ROADMAP.md]
- **Using Criterion as the canonical live-service report path:** Criterion adds its own warmup and iteration model and is better kept as a separate bench surface. [CITED: https://criterion-rs.github.io/book/user_guide/command_line_output.html] [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html]
- **Sampling CPU with a fresh `System` after the run only:** sysinfo documents that CPU usage needs a prior measure and a minimum update interval for useful information. [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html] [VERIFIED: crates/app/src/http_stress.rs]
- **Fixed `sleep` for readiness or metrics cadence:** use readiness probes and `tokio::time::Interval` instead of hoping a hardcoded delay matches reality. [CITED: https://rust.testcontainers.org/features/wait_strategies/] [CITED: https://docs.rs/tokio/latest/tokio/time/struct.Interval.html]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP connection reuse | ad-hoc socket/client pool | one reused `reqwest::Client` | Reqwest already provides pooled connections and cheap cloning. [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] |
| Percentile math | custom p95/p99 implementation | `hdrhistogram` | Tail latency reporting is already standardized in the repo. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| Timer catch-up behavior | manual sleep bookkeeping | `tokio::time::Interval` with explicit missed-tick policy | Official API already models burst/delay/skip semantics. [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html] |
| Container startup guessing | fixed sleep loop | readiness probe + Testcontainers wait strategy | More robust and already aligned with the project’s harness style. [CITED: https://rust.testcontainers.org/features/wait_strategies/] [VERIFIED: crates/app/src/http_stress.rs] |
| Metrics parsing source | direct runtime state access | Prometheus scrape path already exposed by the app | Preserves the real external-process boundary. [VERIFIED: crates/app/src/observability.rs] [VERIFIED: crates/app/src/http_stress.rs] |

**Key insight:** the hard part in this phase is not generating load, it is preserving measurement semantics. Reuse the existing service path, client pooling, histogram library, timer primitives, and metrics surface so the plan spends effort on windowing and report correctness instead of rebuilding solved plumbing. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] [CITED: https://docs.rs/tokio/latest/tokio/time/struct.Interval.html]

## Common Pitfalls

### Pitfall 1: startup cost leaks into the “steady-state” report
**What goes wrong:** the runner starts measuring immediately after spawn and reports numbers inflated by migrations, readiness probing, or binary compilation fallback. [VERIFIED: crates/app/src/http_stress.rs]
**Why it happens:** the current implementation has one `started = Instant::now()` around the whole request batch and no separate warmup/reset phase. [VERIFIED: crates/app/src/http_stress.rs]
**How to avoid:** introduce explicit warmup and measured windows, and reset latency histograms/counters after warmup. [VERIFIED: user prompt] [ASSUMED]
**Warning signs:** the first steady-state run is much slower than later runs with the same profile, or the report duration closely matches service startup time. [ASSUMED]

### Pitfall 2: CPU utilization is noisy or misleading
**What goes wrong:** the report samples CPU only after the run with a new `System` instance and a short sleep, producing low-fidelity numbers. [VERIFIED: crates/app/src/http_stress.rs]
**Why it happens:** sysinfo computes CPU usage from prior measurements and documents a minimum update interval. [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]
**How to avoid:** create one `System` before warmup, refresh it throughout the run, and report max or averaged measured-window CPU rather than one trailing sample. [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html] [ASSUMED]
**Warning signs:** identical runs show unstable CPU values while throughput/latency barely move. [ASSUMED]

### Pitfall 3: sampler tasks burst after stalls
**What goes wrong:** delayed timer ticks can fire back-to-back, over-sampling metrics or skewing host resource readings. [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html]
**Why it happens:** Tokio `Interval` defaults to `Burst` missed-tick behavior. [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html]
**How to avoid:** set `MissedTickBehavior::Delay` for sampler intervals so the next poll is scheduled from the actual call time. [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html]
**Warning signs:** multiple metrics samples land immediately after a long pause, or sampled max depth jumps without matching request activity. [ASSUMED]

### Pitfall 4: live HTTP results get compared to Criterion or in-process numbers as if they were identical
**What goes wrong:** readers treat all throughput numbers as equivalent and draw the wrong conclusions about runtime changes. [VERIFIED: docs/stress-results.md]
**Why it happens:** workload labels and docs do not keep process boundary and measurement method explicit enough. [VERIFIED: docs/stress-results.md] [VERIFIED: docs/template-guide.md]
**How to avoid:** document the live lane separately, include `scenario`, `profile`, `run_duration`, and environment metadata in the report, and keep Criterion labeled as a bench lane only. [VERIFIED: user prompt] [CITED: https://criterion-rs.github.io/book/user_guide/command_line_output.html]
**Warning signs:** one results table combines ring-only, in-process, and live HTTP values with no boundary labels. [ASSUMED]

## Code Examples

Verified patterns from official sources:

### Criterion: custom timing for external processes
```rust
// Source: Criterion timing loops docs
criterion.bench_function("external_process_http_smoke", |b| {
    b.iter_custom(|iters| run_external_process_iterations(iters))
});
```
[CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html]

### Reuse one reqwest client
```rust
// Source: reqwest Client docs
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(5))
    .build()?;
```
[CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html]

### Stable sampler interval
```rust
// Source: tokio Interval docs
let mut interval = tokio::time::interval(Duration::from_millis(250));
interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
```
[CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html]

### CPU refresh needs a prior sample and interval
```rust
// Source: sysinfo docs
let mut system = sysinfo::System::new_all();
system.refresh_cpu_usage();
std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
system.refresh_cpu_usage();
let cpu = system.global_cpu_usage();
```
[CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Spawn service and count a small fixed batch from time zero | Spawn once, warm up, then measure one explicit live window | Current Phase 13 gap closure [VERIFIED: .planning/ROADMAP.md] | Produces numbers that better approximate live service behavior. [ASSUMED] |
| Treat Criterion as the main external-process measurement driver | Use Criterion only where its sampling model fits; use explicit measured windows for long-lived service tests | Criterion current docs [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] | Avoids double-warmup and per-iteration semantics that do not match this phase. [CITED: https://criterion-rs.github.io/book/user_guide/command_line_output.html] |
| Sample CPU once after the run | Sample CPU across the run with a long-lived `System` and valid refresh interval | sysinfo current docs [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html] | Makes resource numbers defensible instead of incidental. [ASSUMED] |
| New HTTP client per request or task | Reuse one pooled client across the run | reqwest current docs [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] | Reduces artificial connection setup overhead. [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] |

**Deprecated/outdated:**
- Using the Phase 12 `external_process_http` Criterion bench result as the headline live-service performance number is outdated for this repo once Phase 13 exists, because that bench still runs the whole external-process harness per measurement iteration. [VERIFIED: benches/external_process_http.rs] [VERIFIED: .planning/ROADMAP.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 13 should remain app-local and not introduce a new crate just for stress orchestration. | Standard Stack / Architecture | Low; existing code already lives in `app`. |
| A2 | The measured window should either drain in-flight requests with a timeout or clearly discard late completions, but it must document one policy explicitly. | Architecture Patterns | Medium; ambiguous cutoff rules would make comparisons noisy. |
| A3 | Average or max measured-window CPU is more useful than one trailing sample for this phase’s operator report. | Common Pitfalls | Medium; report semantics should be confirmed in planning. |
| A4 | Phase 13 does not need a new external library beyond the existing workspace stack. | Standard Stack | Low; current gaps are orchestration and reporting, not missing dependencies. |
| A5 | Excluding distributed load generation from this phase remains correct. | User Constraints | Low; roadmap scope is local live-service evidence. |

## Open Questions

1. **How should the measured window treat in-flight requests at the deadline?**
   - What we know: the current lane submits a fixed batch and waits for all completions. [VERIFIED: crates/app/src/http_stress.rs]
   - What's unclear: whether steady-state mode should drain all in-flight work, drain with timeout, or hard-stop at the deadline.
   - Recommendation: choose one explicit policy in planning and include it in the JSON report so runs remain comparable. [ASSUMED]

2. **Should CPU be reported as average, max, or both?**
   - What we know: success criteria require CPU/core count, and sysinfo supports repeated sampling. [VERIFIED: user prompt] [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]
   - What's unclear: the operator-facing summary preference for this repo.
   - Recommendation: record both `cpu_utilization_avg_percent` and `cpu_utilization_max_percent` internally, then decide during planning whether both belong in the public report. [ASSUMED]

3. **Should the existing Criterion bench be rewritten with `iter_custom`, or left as a tiny smoke bench and de-emphasized in docs?**
   - What we know: Criterion explicitly supports `iter_custom` for separate-process timing, and the current bench reruns the full harness each iteration. [CITED: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html] [VERIFIED: benches/external_process_http.rs]
   - What's unclear: whether Phase 13 wants any bench refactor beyond doc clarification.
   - Recommendation: keep the live lane authoritative and treat any Criterion change as secondary unless planning sees a cheap win. [ASSUMED]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | build/run/test commands | ✓ | `1.85.1` [VERIFIED: cargo --version] | — |
| `rustc` | compiling app/test harness | ✓ | `1.85.1` [VERIFIED: rustc --version] | — |
| Docker daemon | Testcontainers PostgreSQL harness | ✓ | `29.0.1` server reachable [VERIFIED: docker info --format '{{.ServerVersion}}'] | If unavailable, only compile/no-run verification is possible. [ASSUMED] |
| Testcontainers crate | local live DB startup | ✓ | workspace `0.25.0` [VERIFIED: Cargo.toml] [VERIFIED: cargo info testcontainers --locked] | No good realism-equivalent fallback inside this phase. [ASSUMED] |
| Prometheus metrics endpoint | lag/depth/append signal scraping | ✓ | repo-local app endpoint [VERIFIED: crates/app/src/observability.rs] [VERIFIED: crates/app/src/http_stress.rs] | Without it, Phase 13 would fail success criterion 3. [VERIFIED: user prompt] |

**Missing dependencies with no fallback:**
- None found on this machine for local planning research. [VERIFIED: cargo --version] [VERIFIED: rustc --version] [VERIFIED: docker info --format '{{.ServerVersion}}']

**Missing dependencies with fallback:**
- `cargo-nextest` was not detected, but this phase does not require it for its primary validation path. [VERIFIED: command -v cargo-nextest]

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust integration tests + CLI smoke commands + optional Criterion bench [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/main.rs] |
| Config file | workspace `Cargo.toml` and `crates/app/Cargo.toml` [VERIFIED: Cargo.toml] [VERIFIED: crates/app/Cargo.toml] |
| Quick run command | `cargo test -p app external_process_http_stress_smoke -- --nocapture` [VERIFIED: crates/app/src/http_stress.rs] |
| Full suite command | `cargo test --workspace && cargo run -p app -- http-stress` [VERIFIED: crates/app/src/main.rs] |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEST-03 | Separate live HTTP lane with explicit profiles and non-Criterion interpretation | CLI smoke + docs grep | `cargo run -p app -- http-stress` and `rg -n "steady-state|baseline|burst|hot-key|Criterion" docs/stress-results.md docs/template-guide.md crates/app/src` | ❌ Wave 0 |
| TEST-04 | One long-lived `app serve` process survives warmup and measured window and reports full live metrics | integration/stress | `cargo test -p app external_process_http_stress_smoke -- --nocapture` | ✅ |
| OBS-02 | Report includes queue depth, append latency, projection lag, outbox lag, and reject-rate signals from Prometheus | integration/stress | `cargo test -p app external_process_http_stress_smoke -- --nocapture && rg -n "append_latency|projection_lag|outbox_lag|reject_rate|ingress_depth_max|shard_depth_max" crates/app/src/http_stress.rs docs/stress-results.md` | Partial |

### Sampling Rate
- **Per task commit:** run the targeted app stress smoke and any docs grep tied to the edited profile/report fields. [VERIFIED: crates/app/src/http_stress.rs]
- **Per wave merge:** rerun the targeted stress smoke plus the CLI command that prints the live JSON report. [VERIFIED: crates/app/src/main.rs]
- **Phase gate:** full workspace tests green, then one documented live HTTP run that demonstrates warmup and measured-window separation. [VERIFIED: user prompt] [ASSUMED]

### Wave 0 Gaps
- [ ] Add explicit warmup duration and measurement duration config to `HttpStressConfig`. [VERIFIED: crates/app/src/http_stress.rs]
- [ ] Add profile presets beyond the current `smoke()` and `bench()` helpers. [VERIFIED: crates/app/src/http_stress.rs]
- [ ] Reset histogram/counter state after warmup before starting the measured window. [VERIFIED: crates/app/src/http_stress.rs]
- [ ] Improve CPU sampling to use repeated measured-window refreshes instead of one trailing sample. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html]
- [ ] Update docs so live steady-state output is interpreted separately from Phase 12 external-process bench output. [VERIFIED: docs/stress-results.md] [VERIFIED: docs/template-guide.md]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no [ASSUMED] | Not a phase focus; the harness should keep targeting the local spawned app rather than introducing auth features. [ASSUMED] |
| V3 Session Management | no [ASSUMED] | Not applicable to the stress harness itself. [ASSUMED] |
| V4 Access Control | yes [ASSUMED] | Keep the harness pointed at its own spawned localhost service and avoid a generic arbitrary-target load tool. [ASSUMED] |
| V5 Input Validation | yes [VERIFIED: user prompt] | Validate profile names, durations, concurrency, and count values before the run starts. [ASSUMED] |
| V6 Cryptography | no [ASSUMED] | No new cryptographic behavior should be introduced in this phase. [ASSUMED] |

### Known Threat Patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Accidental load against the wrong target | Denial of Service | Derive listen addresses from the spawned harness and do not accept arbitrary external target URLs in Phase 13. [ASSUMED] |
| Sensitive environment leakage in reports | Information Disclosure | Report configuration and host metadata, but never echo full `DATABASE_URL` or secret env vars. [ASSUMED] |
| Metrics-cardinality blow-up | Denial of Service | Keep using bounded labels from existing observability and avoid adding request-identity labels for stress-specific metrics. [VERIFIED: crates/app/src/observability.rs] |

## Sources

### Primary (HIGH confidence)
- `Cargo.toml`, `crates/app/Cargo.toml` - workspace dependency pins and bench registration. [VERIFIED: repo files]
- `crates/app/src/http_stress.rs` - current external-process harness, metric scraping, and live gap evidence. [VERIFIED: repo file]
- `crates/app/src/stress.rs` - existing report schema and profile naming precedent. [VERIFIED: repo file]
- `crates/app/src/main.rs` - current CLI entrypoints. [VERIFIED: repo file]
- `crates/app/src/observability.rs` - Prometheus metric catalog and scrape surface. [VERIFIED: repo file]
- `docs/stress-results.md`, `docs/template-guide.md` - current workload-layer guidance. [VERIFIED: repo files]
- Criterion timing loops: https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html [CITED]
- Criterion command-line output: https://criterion-rs.github.io/book/user_guide/command_line_output.html [CITED]
- Reqwest client docs/source: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html [CITED]
- Tokio interval docs: https://docs.rs/tokio/latest/tokio/time/struct.Interval.html and https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html [CITED]
- Sysinfo docs: https://docs.rs/sysinfo/latest/sysinfo/index.html [CITED]
- Testcontainers wait strategy docs: https://rust.testcontainers.org/features/wait_strategies/ [CITED]

### Secondary (MEDIUM confidence)
- `cargo info criterion --locked`, `cargo info hdrhistogram --locked`, `cargo info reqwest --locked`, `cargo info sysinfo --locked`, `cargo info metrics-exporter-prometheus --locked`, `cargo info testcontainers --locked` - currentness checks relative to workspace pins. [VERIFIED: cargo info commands]

### Tertiary (LOW confidence)
- None beyond claims explicitly tagged `[ASSUMED]`.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - the phase should reuse the existing app/reqwest/tokio/hdrhistogram/sysinfo/prometheus stack already in the repo. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/http_stress.rs]
- Architecture: HIGH - the codebase and official docs point to the same answer: duration windows, reused clients, explicit timer policy, and external-process separation. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/reqwest/latest/src/reqwest/async_impl/client.rs.html] [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html]
- Pitfalls: HIGH - the two main ones are directly visible in current code and confirmed by official docs. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/sysinfo/latest/sysinfo/index.html] [CITED: https://criterion-rs.github.io/book/user_guide/command_line_output.html]

**Research date:** 2026-04-26
**Valid until:** 2026-05-26

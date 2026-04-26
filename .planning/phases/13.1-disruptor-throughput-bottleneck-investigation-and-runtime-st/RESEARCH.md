# Phase 13.1: Disruptor Throughput Bottleneck Investigation and Runtime Stress Optimization - Research

**Researched:** 2026-04-26 [VERIFIED: local system date]
**Domain:** Rust command-runtime throughput diagnosis across HTTP adapter, routing, shard execution, PostgreSQL append, and stress measurement [VERIFIED: user prompt]
**Confidence:** HIGH [VERIFIED: repo code review] [VERIFIED: local measurements] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html]

## User Constraints

No phase-local `13.1-CONTEXT.md` exists, so the effective constraints below are derived from the roadmap, requirements, state, AGENTS instructions, and the user prompt. [VERIFIED: init phase-op 13.1] [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/STATE.md] [VERIFIED: user prompt]

### Locked Decisions

- Phase 13.1 exists because Phase 13 steady-state live HTTP throughput was much lower than expected for a disruptor-shaped runtime, and archive sign-off now depends on bottleneck analysis plus targeted optimization. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/STATE.md]
- The phase must classify the dominant limit by layer: HTTP client/server, adapter admission, command routing, shard queueing, disruptor execution, aggregate decision, event-store append, projection/outbox work, or measurement configuration. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: user prompt]
- The phase must compare ring-only, runtime-only, storage-only, adapter-only, and live HTTP evidence enough to show where throughput is lost instead of assuming the disruptor path is the problem. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: user prompt]
- The phase must either identify and fix at least one concrete bottleneck or explicitly document why no safe code change is justified yet. [VERIFIED: .planning/ROADMAP.md]
- Updated stress output must include throughput, p50/p95/p99/max latency, reject/error counts, queue depth, append latency, and relevant resource metadata. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: user prompt]
- The explanation must separate ring capability from full-service limits such as PostgreSQL append, HTTP overhead, or configured backpressure. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: docs/stress-results.md]

### Claude's Discretion

- Choose the highest-confidence code and harness changes that improve measured throughput without violating the event-store-as-source-of-truth architecture. [VERIFIED: AGENTS instructions] [VERIFIED: .planning/REQUIREMENTS.md]
- Prefer fixes that preserve the existing stack and repo boundaries over introducing new libraries. [VERIFIED: AGENTS instructions] [VERIFIED: Cargo.toml]
- Add measurement improvements when current fields are present but not trustworthy enough to support diagnosis. [VERIFIED: local measurements] [VERIFIED: crates/app/src/http_stress.rs]

### Deferred Ideas (OUT OF SCOPE)

- Distributed partition ownership, broker benchmarking, and production deployment automation remain out of scope for this phase. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/ROADMAP.md]
- Final milestone debt closure and archive sign-off remain Phase 14 work. [VERIFIED: .planning/ROADMAP.md]

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| RUNTIME-01 | Adapter requests enter through bounded ingress with explicit overload behavior. [VERIFIED: .planning/REQUIREMENTS.md] | Keep bounded ingress, but move processing behind real per-shard workers so admission stays bounded without serializing all shards behind one engine loop. [VERIFIED: crates/es-runtime/src/gateway.rs] [VERIFIED: crates/es-runtime/src/engine.rs] |
| RUNTIME-02 | Same partition key must route to the same shard owner. [VERIFIED: .planning/REQUIREMENTS.md] | Preserve the current `PartitionRouter`; the problem is not unstable routing, it is that routed work still converges on one consumer task. [VERIFIED: crates/es-runtime/src/router.rs] [VERIFIED: local 1-shard vs 8-shard measurement] |
| RUNTIME-05 | Replies are sent only after durable append commit succeeds. [VERIFIED: .planning/REQUIREMENTS.md] | Keep commit-gated replies; optimize around shard execution structure and new-stream read avoidance rather than bypassing storage durability. [VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/es-store-postgres/src/sql.rs] |
| TEST-03 | Bench harnesses separately measure ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded scenarios. [VERIFIED: .planning/REQUIREMENTS.md] | Phase 13.1 should refresh layer-comparison output under one script and fix the current false-hot-key live HTTP profile before trusting scenario comparisons. [VERIFIED: benches/ring_only.rs] [VERIFIED: benches/adapter_only.rs] [VERIFIED: benches/storage_only.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| TEST-04 | Realistic single-service/external-process stress reports throughput, latency, queue depth, append latency, lag, reject rate, and CPU/core data. [VERIFIED: .planning/REQUIREMENTS.md] | Phase 13.1 must fix zero-value diagnostic fields, record scrape health, and include layer labels so the report can explain where throughput is lost. [VERIFIED: local external-process measurements] [VERIFIED: crates/app/src/http_stress.rs] |
| OBS-02 | Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, OCC conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency. [VERIFIED: .planning/REQUIREMENTS.md] | The metric catalog exists, but the live report currently hides some failures with zeros and does not surface ring wait or scrape success. Phase 13.1 should make those diagnostics trustworthy. [VERIFIED: crates/app/src/observability.rs] [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: local external-process measurements] |

</phase_requirements>

## Summary

The dominant throughput limit is not the disruptor ring itself. The current runtime shape routes commands to shards, but all routed work for a given aggregate type still flows through one Tokio `mpsc::Receiver` and one `process_one()` loop task, so increasing `shard_count` does not create parallel command execution. [VERIFIED: crates/es-runtime/src/engine.rs] [VERIFIED: crates/app/src/serve.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] Local evidence matches that architecture: a 5-second external HTTP baseline at concurrency 8 produced `386` successes at about `75.7 req/s` with `shard_count=1`, and the same workload with `shard_count=8` produced the same `386` successes at about `75.7 req/s`. [VERIFIED: local external-process measurements]

The second major limit is the database-heavy hot path for unique new-stream commands. Every unique `PlaceOrder` request does a durable replay lookup, then a cache-miss rehydration that always loads the latest snapshot and then reads stream events, and only then begins the append transaction that acquires advisory locks, checks dedupe, checks stream revision, updates the stream row, inserts events, writes dedupe, and commits. [VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/es-store-postgres/src/rehydrate.rs] [VERIFIED: crates/es-store-postgres/src/sql.rs] For the current live HTTP workload, `PlaceOrder` also uses a fresh `order_id` every time and expects `NoStream`, so those rehydration reads are usually redundant work against default state. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs]

The stress harness also has correctness gaps that matter for diagnosis. The live runner throttles submissions with a 1 ms interval, which caps offered load near 1000 requests/sec even if the service becomes faster. [VERIFIED: crates/app/src/http_stress.rs] The advertised `hot-key` profile is not actually hot-key traffic because the request fixture generates a fresh `order_id` for every request and order routing hashes the order ID as the partition key. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/es-runtime/src/router.rs] The current live report can also emit implausible zeros for `shard_depth_max` and `append_latency_p95_micros` while CPU samples are present, which means the report fields exist but are not yet trustworthy enough for bottleneck classification. [VERIFIED: local external-process measurements] [VERIFIED: crates/app/src/http_stress.rs]

**Primary recommendation:** treat Phase 13.1 as a runtime-architecture-plus-measurement phase, not a library-selection phase: 1) split aggregate execution into real per-shard workers, 2) add a safe `ExpectedRevision::NoStream` fast path that skips cache-miss rehydration for create/place/register-style commands, and 3) fix the live stress harness so hot-key, shard-depth, append-latency, and offered-load measurements are semantically correct. [VERIFIED: repo code review] [VERIFIED: local measurements] [ASSUMED]

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `disruptor` | workspace `4.0.0`; current crate `4.0.0` [VERIFIED: Cargo.toml] [VERIFIED: cargo info disruptor --locked] | Local shard handoff primitive only. | Keep it for ordered in-process release/poll semantics; the current problem is the execution topology around it, not the crate choice. [VERIFIED: crates/es-runtime/src/disruptor_path.rs] [CITED: https://docs.rs/disruptor/latest/disruptor/] |
| `tokio` | workspace `1.52.0`; current crate `1.52.1` [VERIFIED: Cargo.toml] [VERIFIED: cargo info tokio --locked] | Runtime tasks, bounded channels, intervals, and HTTP stress orchestration. | Tokio already provides the bounded `mpsc` and task model needed for per-shard workers and measured load generation. [VERIFIED: Cargo.toml] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| `sqlx` | workspace `0.8.6`; current crate `0.8.6` stable [VERIFIED: Cargo.toml] [VERIFIED: cargo info sqlx --locked] | PostgreSQL event-store and transaction path. | Keep explicit SQL and transaction control; Phase 13.1 should reduce redundant queries before changing the store stack. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Pool.html] |
| `reqwest` | workspace `0.12.24`; current crate `0.12.28` [VERIFIED: Cargo.toml] [VERIFIED: cargo info reqwest --locked] | External HTTP load client. | Reuse one pooled client per run instead of inventing custom client plumbing. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/reqwest/latest/reqwest/struct.Client.html] |
| `hdrhistogram` | workspace `7.5.4`; current crate `7.5.4` [VERIFIED: Cargo.toml] [VERIFIED: cargo info hdrhistogram --locked] | Tail-latency percentiles for all stress lanes. | Keep one percentile implementation across in-process and external-process reports. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/src/http_stress.rs] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `metrics` + `metrics-exporter-prometheus` | workspace `0.24.3` + `0.18.1` [VERIFIED: Cargo.toml] | Runtime/store depth and latency export. | Use the existing `/metrics` surface, but add scrape-health reporting and fix missing/zero diagnostic fields. [VERIFIED: crates/app/src/observability.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| `sysinfo` | workspace `0.36.1`; current crate `0.36.1` [VERIFIED: Cargo.toml] [VERIFIED: cargo info sysinfo --locked] | CPU sampling during measured windows. | Keep it, but respect `MINIMUM_CPU_UPDATE_INTERVAL` semantics and report sample counts so consumers know whether CPU data is meaningful. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/sysinfo/latest/sysinfo/] |
| Existing layer benches | repo-local benches [VERIFIED: benches/ring_only.rs] [VERIFIED: benches/adapter_only.rs] [VERIFIED: benches/storage_only.rs] | Ring-only, adapter-only, storage-only, and projector/outbox isolation. | Reuse them under one comparison script rather than adding another benchmark framework. [VERIFIED: .planning/REQUIREMENTS.md] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Runtime architecture fix | Swap disruptor crates or add a new executor library | The measured evidence points to serialization and extra DB round-trips, not missing ring features. [VERIFIED: local measurements] [VERIFIED: crates/es-runtime/src/engine.rs] |
| Safe new-stream fast path | Remove durability checks or bypass PostgreSQL | That would violate the event-store-as-source-of-truth contract. [VERIFIED: AGENTS instructions] [VERIFIED: .planning/REQUIREMENTS.md] |
| Fixing the existing live harness | Introduce another load-testing binary first | The repo already has the canonical live runner; Phase 13.1 should make it truthful before adding more surfaces. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: docs/stress-results.md] |

**Installation:**
```bash
# No new crates are required for the first optimization pass.
# Reuse the existing workspace stack.
cargo test --workspace
```
[VERIFIED: Cargo.toml]

**Version verification:** workspace pins were read from `Cargo.toml`, and current crate metadata was checked with `cargo info` on 2026-04-26 for `disruptor`, `tokio`, `reqwest`, `sqlx`, `hdrhistogram`, and `sysinfo`. [VERIFIED: Cargo.toml] [VERIFIED: cargo info disruptor --locked] [VERIFIED: cargo info tokio --locked] [VERIFIED: cargo info reqwest --locked] [VERIFIED: cargo info sqlx --locked] [VERIFIED: cargo info hdrhistogram --locked] [VERIFIED: cargo info sysinfo --locked]

## Architecture Patterns

### Recommended Project Structure

```text
crates/es-runtime/src/
├── gateway.rs          # bounded adapter ingress only
├── router.rs           # deterministic tenant+partition routing only
├── engine.rs           # ingress fan-out orchestration
├── shard.rs            # per-shard worker state and ordered processing
└── disruptor_path.rs   # release/poll primitive only

crates/app/src/
├── serve.rs            # one task per aggregate engine plus one task per shard worker
├── stress.rs           # in-process layer comparison
└── http_stress.rs      # external-process live lane and report generation
```
[VERIFIED: repo file layout] [ASSUMED]

### Pattern 1: Route once, execute on one worker per shard

**What:** Keep one bounded ingress surface, but hand routed commands to shard-owned worker tasks so shard count creates real parallelism. [VERIFIED: .planning/REQUIREMENTS.md] [ASSUMED]

**When to use:** Always for the command runtime path. The current single receiver plus single engine loop does not let separate shards progress independently. [VERIFIED: crates/es-runtime/src/engine.rs] [VERIFIED: crates/app/src/serve.rs]

**Example:**
```rust
// Source: recommended refactor derived from current engine/serve topology
let (gateway, routed_rx) = CommandGateway::new(router, ingress_capacity)?;
let shard_senders = spawn_shard_workers(shard_count, ring_size, store.clone(), codec.clone());

tokio::spawn(async move {
    while let Some(routed) = routed_rx.recv().await {
        shard_senders[routed.shard_id.value()].send(routed).await?;
    }
});
```
[VERIFIED: crates/es-runtime/src/engine.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] [ASSUMED]

### Pattern 2: Skip cache-miss rehydration for safe `NoStream` create paths

**What:** For commands whose aggregate contract already says `ExpectedRevision::NoStream`, decide against default state on a cold cache miss and let append OCC enforce non-existence. [VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/es-store-postgres/src/sql.rs] [ASSUMED]

**When to use:** For create/register/place commands that do not require historical reads to decide and already fail safely if the stream exists. [VERIFIED: crates/example-commerce/src/order.rs] [ASSUMED]

**Example:**
```rust
// Source: recommended fast path derived from current shard/store contracts
let current_state = if let Some(cached) = self.cache.get(&cache_key) {
    cached.clone()
} else if envelope.expected_revision == ExpectedRevision::NoStream {
    A::State::default()
} else {
    rehydrate_state(store, codec, &tenant_id, &stream_id).await?
};
```
[VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/example-commerce/src/order.rs] [ASSUMED]

### Pattern 3: Offered-load control must be explicit and profile-specific

**What:** Use a saturating worker/semaphore loop for high-throughput profiles and keep duration plus concurrency as the primary knobs. [VERIFIED: user prompt] [ASSUMED]

**When to use:** For baseline, burst, and any archive-facing throughput run. The current 1 ms submit tick is acceptable as a smoke limiter but not as the only offered-load policy. [VERIFIED: crates/app/src/http_stress.rs]

**Example:**
```rust
// Source: recommended live-harness change
while Instant::now() < deadline {
    while in_flight < config.concurrency {
        spawn_request();
    }
    join_next_completion().await?;
}
```
[VERIFIED: crates/app/src/http_stress.rs] [ASSUMED]

### Pattern 4: Stress profiles must change key distribution, not only concurrency

**What:** Encode key-shape explicitly: uniform spread, finite hot set, true single hot key, and duplicate-idempotency replay. [VERIFIED: user prompt] [ASSUMED]

**When to use:** Always when labeling a profile as `hot-key`, `burst`, or `replay-heavy`. The current Phase 13 live `HotKey` profile changes runtime knobs but not request key locality. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/es-runtime/src/router.rs]

**Example:**
```rust
// Source: recommended request-shape contract
match key_mode {
    KeyMode::Unique => order_id_for(index),
    KeyMode::HotSet(size) => order_id_for(index % size),
    KeyMode::SingleHotKey => fixed_order_id(),
}
```
[ASSUMED]

### Anti-Patterns to Avoid

- **Shards in name only:** one routed-command receiver plus one processing loop serializes all shard work. [VERIFIED: crates/es-runtime/src/engine.rs] [VERIFIED: crates/app/src/serve.rs]
- **Always rehydrating cold `NoStream` commands:** this adds avoidable snapshot and stream-read queries before an append that already checks `NoStream`. [VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/es-store-postgres/src/rehydrate.rs] [VERIFIED: crates/example-commerce/src/order.rs]
- **Calling a unique-stream workload `hot-key`:** a profile name is not evidence; the generated partition keys must actually repeat. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs]
- **Treating zero-valued metrics as real measurements:** the current sampler swallows scrape failures and can leave diagnostic fields at zero. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: local external-process measurements]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tail percentiles | custom p95/p99 math | `hdrhistogram` | Already used in both stress lanes and reliable for latency tails. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| HTTP connection reuse | per-request clients or manual socket pool | one reused `reqwest::Client` | Reqwest already pools connections internally. [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/reqwest/latest/reqwest/struct.Client.html] |
| Backpressure timers | ad-hoc sleeps | Tokio bounded channels plus explicit interval/semaphore policy | Tokio already provides the right primitives; the current bug is policy, not missing library support. [VERIFIED: crates/es-runtime/src/gateway.rs] [VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| Stream-existence truth | in-memory “seen stream” flags | PostgreSQL `ExpectedRevision::NoStream` + OCC | Durable append is the source of truth; fast paths should reduce reads, not invent alternate correctness state. [VERIFIED: crates/es-store-postgres/src/sql.rs] [VERIFIED: .planning/REQUIREMENTS.md] |
| Metric transport | runtime-internal peeks from the harness | existing Prometheus scrape surface | Keeps the live lane external-process-realistic. [VERIFIED: crates/app/src/observability.rs] [VERIFIED: crates/app/src/http_stress.rs] |

**Key insight:** do not spend Phase 13.1 swapping libraries. The highest-confidence gains are architectural and methodological: real shard parallelism, fewer cold-path SQL round-trips, and truthful layer diagnostics. [VERIFIED: repo code review] [VERIFIED: local measurements] [ASSUMED]

## Common Pitfalls

### Pitfall 1: `shard_count` looks configurable but does not improve throughput
**What goes wrong:** operators raise `APP_SHARD_COUNT`, but measured throughput barely moves. [VERIFIED: local 1-shard vs 8-shard measurement]
**Why it happens:** one `CommandEngine` owns one `mpsc::Receiver`, and `serve.rs` spawns only one engine loop task per aggregate type. [VERIFIED: crates/es-runtime/src/engine.rs] [VERIFIED: crates/app/src/serve.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html]
**How to avoid:** fan out routed commands to one worker per shard and make shard-level progress independent. [ASSUMED]
**Warning signs:** `shard_count=1` and `shard_count=8` produce essentially identical throughput and latency. [VERIFIED: local external-process measurements]

### Pitfall 2: unique create commands spend more time reading than deciding
**What goes wrong:** new-stream traffic pays replay lookup plus rehydration reads before the append transaction. [VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/es-store-postgres/src/rehydrate.rs]
**Why it happens:** cache misses always call `lookup_command_replay()` and then `load_rehydration()`, even for `ExpectedRevision::NoStream` commands that can decide from default state. [VERIFIED: crates/es-runtime/src/shard.rs] [VERIFIED: crates/example-commerce/src/order.rs]
**How to avoid:** add a safe cold-cache fast path for `NoStream` commands and keep append OCC as the correctness gate. [ASSUMED]
**Warning signs:** low CPU utilization, high end-to-end latency, and no throughput gain from more shards. [VERIFIED: local measurements] [ASSUMED]

### Pitfall 3: the stress runner under-drives the system
**What goes wrong:** the harness reports service throughput, but the runner itself limits submissions. [VERIFIED: crates/app/src/http_stress.rs]
**Why it happens:** `execute_http_window()` waits on a 1 ms tick before every submission. [VERIFIED: crates/app/src/http_stress.rs]
**How to avoid:** separate smoke pacing from baseline/burst offered-load saturation and record the chosen submission policy in the report. [ASSUMED]
**Warning signs:** throughput never exceeds roughly 1000 requests/sec even after service-side optimizations. [VERIFIED: crates/app/src/http_stress.rs] [ASSUMED]

### Pitfall 4: a `hot-key` profile that does not reuse keys misleads diagnosis
**What goes wrong:** the report claims hot-key behavior, but the runtime never sees repeated partition keys. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs]
**Why it happens:** the request fixture increments `order_id` for every request, and order routing hashes the order ID into the partition key. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/es-runtime/src/router.rs]
**How to avoid:** make key-distribution an explicit workload dimension and reuse a finite stream-id set for hot-key runs. [ASSUMED]
**Warning signs:** `hot-key` results differ only because of concurrency or shard-count overrides, not because of queue locality or cache reuse. [VERIFIED: crates/app/src/http_stress.rs]

### Pitfall 5: zero diagnostics are mistaken for healthy diagnostics
**What goes wrong:** `append_latency_p95_micros` or `shard_depth_max` can be zero in a non-trivial run, and readers treat that as “no latency” or “no queueing.” [VERIFIED: local external-process measurements]
**Why it happens:** the sampler ignores scrape errors, `shard_depth_max` depends on a metric that is updated after pop instead of at enqueue/release time, and the report has no scrape-success counter. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/es-runtime/src/shard.rs] [ASSUMED]
**How to avoid:** report scrape success/failure counts, surface raw sample counts, and update depth metrics at enqueue, release, and completion boundaries. [ASSUMED]
**Warning signs:** CPU samples exist, throughput is non-zero, but append or shard-depth metrics remain zero. [VERIFIED: local external-process measurements]

## Code Examples

Verified patterns from repo and official docs:

### One worker per shard after bounded ingress
```rust
// Source: recommended runtime refactor based on current engine/serve structure
let (gateway, routed_rx) = CommandGateway::new(router, ingress_capacity)?;
let shard_senders = spawn_shard_workers(shard_count, ring_size, store, codec);

tokio::spawn(async move {
    while let Some(routed) = routed_rx.recv().await {
        shard_senders[routed.shard_id.value()].send(routed).await?;
    }
});
```
[VERIFIED: crates/es-runtime/src/engine.rs] [VERIFIED: crates/app/src/serve.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] [ASSUMED]

### Reuse one HTTP client for the whole run
```rust
// Source: current harness + reqwest docs
let client = Client::builder()
    .timeout(Duration::from_secs(5))
    .build()?;
```
[VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/reqwest/latest/reqwest/struct.Client.html]

### Stable sampler timing policy
```rust
// Source: current harness + Tokio docs
let mut ticker = interval(Duration::from_millis(250));
ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
```
[VERIFIED: crates/app/src/http_stress.rs] [CITED: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html] [ASSUMED]

### Cold-cache `NoStream` fast path
```rust
// Source: recommended optimization derived from current shard/order/store contracts
if cache_miss && envelope.expected_revision == ExpectedRevision::NoStream {
    let state = A::State::default();
    let decision = A::decide(&state, envelope.command, &envelope.metadata)?;
    // append still enforces NoStream durably
}
```
[VERIFIED: crates/example-commerce/src/order.rs] [VERIFIED: crates/es-store-postgres/src/sql.rs] [ASSUMED]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| “Many shards” behind one logical consumer | Real per-shard worker ownership after routing | Current Rust async best practice; repo not there yet. [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] | Shard count starts affecting throughput instead of being mostly cosmetic. [ASSUMED] |
| Always rehydrate on cold cache miss | Skip rehydrate for safe `NoStream` create paths | Current repo still always rehydrates. [VERIFIED: crates/es-runtime/src/shard.rs] | Removes two read queries from the common new-stream path. [ASSUMED] |
| Profile labels as configuration presets only | Profile labels encode traffic shape, key shape, and admission policy | Current repo only changes some knobs for `HotKey`. [VERIFIED: crates/app/src/http_stress.rs] | Results become interpretable across ring, runtime, and live lanes. [ASSUMED] |
| Silent scrape failures mapped to zeros | Report scrape health and raw sample counts | Current repo swallows scrape errors. [VERIFIED: crates/app/src/http_stress.rs] | Prevents false confidence in append/depth/lag fields. [ASSUMED] |

**Deprecated/outdated:**

- Treating ring-only or live HTTP throughput as directly comparable numbers is outdated for this repo; the docs already describe them as different diagnostic layers and Phase 13.1 should keep that separation explicit. [VERIFIED: docs/stress-results.md]
- Treating the current `hot-key` live HTTP preset as a real hot-key workload is outdated because the request generator does not reuse partition keys. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | A safe `NoStream` cold-cache fast path can be introduced without violating existing correctness expectations for create/place/register-style commands. | Architecture Patterns / Common Pitfalls | Medium; if a command implicitly depends on historical reads, the fast path could misclassify existing streams until append conflict handling corrects it. |
| A2 | `shard_depth_max=0` and `append_latency_p95_micros=0` in live runs are caused by instrumentation/reporting issues rather than genuinely empty queues and zero-cost appends. | Summary / Common Pitfalls | Low; the values are implausible under observed throughput, but the exact root cause still needs implementation-time confirmation. |
| A3 | Switching sampler timing from `Skip` to a more diagnosis-friendly policy such as `Delay` will improve report interpretability without harming throughput conclusions. | Code Examples / Common Pitfalls | Low; this affects measurement cadence more than system correctness. |

## Open Questions (RESOLVED)

1. **Should Phase 13.1 optimize only the order engine path or generalize the runtime fix across all aggregate engines?**
   - What we know: `serve.rs` constructs the same `CommandEngine` topology for order, product, and user. [VERIFIED: crates/app/src/serve.rs]
   - Previously unclear: the current live HTTP workload exercises order placement only, so only the order path has direct performance evidence. [VERIFIED: crates/app/src/http_stress.rs]
   - RESOLVED: implement the shard-worker architecture once in `es-runtime`, not as an order-only fork. Plan 13.1-01 applies the topology change to the generic runtime and updates `serve.rs` wiring for order, product, and user engines.

2. **Should live HTTP stress include projector/outbox work in the same service process?**
   - What we know: the current `serve` composition starts HTTP plus command engines, but not projector or outbox worker loops. [VERIFIED: crates/app/src/serve.rs]
   - Previously unclear: whether Phase 13.1 wants full command+projection+outbox service throughput or only command-path throughput plus lag observability. [VERIFIED: user prompt]
   - RESOLVED: keep Phase 13.1 focused on command-path bottlenecks, but explicitly document that current live HTTP numbers exclude active projector/outbox side effects. Plan 13.1-03 documents projector/outbox pressure as a separate lane, not part of the command-path live HTTP ceiling.

3. **What exact hot-key semantics should the repo standardize?**
   - What we know: the current live preset does not reuse keys. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: crates/example-commerce/src/order.rs]
   - Previously unclear: whether “hot-key” should mean one stream, one tenant plus finite stream set, or one product/order correlation pattern. [VERIFIED: user prompt]
   - RESOLVED: standardize `Unique`, `HotSet(N)`, and `SingleHotKey` workload modes in the external-process live HTTP lane and document them in the comparison workflow. Plan 13.1-02 owns the CLI/config model and Plan 13.1-03 owns operator documentation.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | build, test, benches, live stress | ✓ | `1.85.1` [VERIFIED: local command] | — |
| `rustc` | compile runtime and benches | ✓ | `1.85.1` [VERIFIED: local command] | — |
| Docker | Testcontainers PostgreSQL for stress and integration paths | ✓ | `29.0.1` [VERIFIED: local command] | — |
| `cargo-nextest` | faster test execution | ✗ | — [VERIFIED: local command] | use `cargo test` |
| `cargo-llvm-cov` | coverage | ✗ | — [VERIFIED: local command] | skip coverage during this phase |
| `cargo-deny` | advisory/license checks | ✗ | — [VERIFIED: local command] | skip during throughput investigation |

**Missing dependencies with no fallback:**

- None. [VERIFIED: local environment audit]

**Missing dependencies with fallback:**

- `cargo-nextest`, `cargo-llvm-cov`, and `cargo-deny` are absent, but Phase 13.1 can proceed with `cargo test`, targeted stress commands, and bench commands. [VERIFIED: local environment audit]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` for automated checks, Criterion benches for layer isolation, `cargo run -p app -- http-stress ...` for live evidence. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/main.rs] |
| Config file | none; workspace conventions come from Cargo targets and Rust test modules. [VERIFIED: Cargo.toml] |
| Quick run command | `cargo test -p es-runtime runtime_flow -- --nocapture && cargo test -p app http_stress -- --nocapture` [VERIFIED: crates/es-runtime/tests/runtime_flow.rs] [VERIFIED: crates/app/src/http_stress.rs] |
| Full suite command | `cargo test --workspace` plus targeted live runs such as `cargo run -q -p app -- http-stress --profile baseline --warmup-seconds 1 --measure-seconds 5 --concurrency 8` [VERIFIED: crates/app/src/main.rs] [VERIFIED: local measurements] |

### Current Plan Task Map

| Task ID | Plan | Wave | Requirement | Behavior | Test Type | Automated Command |
|---------|------|------|-------------|----------|-----------|-------------------|
| 13.1-01-01 | 01 | 1 | RUNTIME-01, RUNTIME-02, RUNTIME-05 | accepted-but-undispatched shutdown paths resolve explicitly instead of stranding replies | integration | `cargo test -p es-runtime accepted_but_undispatched_commands_receive_unavailable_on_shutdown -- --nocapture` |
| 13.1-01-02 | 01 | 1 | RUNTIME-01, RUNTIME-02, RUNTIME-05 | runtime shutdown drains ingress and waits for in-flight shard work | integration | `cargo test -p es-runtime runtime_flow -- --nocapture` |
| 13.1-02-01 | 02 | 1 | TEST-04, OBS-02 | report regressions prevent synthetic observed queue-depth claims | unit | `cargo test -p app observed_ingress_depth_is_not_synthesized_from_concurrency -- --nocapture` |
| 13.1-02-02 | 02 | 1 | TEST-04, OBS-02 | live harness uses observed-versus-estimated metrics fields and diagnostic workload labeling | integration | `cargo test -p app http_stress -- --nocapture` |
| 13.1-03-01 | 03 | 2 | TEST-03, TEST-04 | storage-only lane produces real benchmark output and renames the repeated-stream lane to a diagnostic artifact | benchmark | `cargo bench --bench storage_only -- --sample-size 10` |
| 13.1-03-02 | 03 | 2 | TEST-03, TEST-04, OBS-02 | one baseline script run regenerates unique plus shard-count evidence before any non-`inconclusive` ceiling claim | live baseline + docs | `PHASE13_1_COMPARE_MODE=baseline bash scripts/compare-stress-layers.sh` |

### Sampling Rate

- **Per task commit:** run the task-local `<verify><automated>` command from the active revised plan. [VERIFIED: 13.1 plan set]
- **Per wave merge:** rerun `cargo test -p es-runtime runtime_flow -- --nocapture` and `cargo test -p app http_stress -- --nocapture` before moving past Wave 1. [ASSUMED]
- **Phase gate:** rerun `PHASE13_1_COMPARE_MODE=baseline bash scripts/compare-stress-layers.sh` and verify it produced `live-http-unique.json`, `live-http-shard-1.json`, and `live-http-shard-8.json` before `/gsd-verify-work`. [ASSUMED]

### Wave 0 Status

- [x] Existing test, benchmark, and script entrypoints cover the current six-task revised plan set; no extra Wave 0 scaffolding is required. [VERIFIED: 13.1-01-PLAN.md] [VERIFIED: 13.1-02-PLAN.md] [VERIFIED: 13.1-03-PLAN.md]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no [VERIFIED: current stress lane has no auth layer] | not in scope for this phase |
| V3 Session Management | no [VERIFIED: current stress lane has no session layer] | not in scope for this phase |
| V4 Access Control | no [VERIFIED: current stress lane targets internal benchmark-style endpoints without auth] | not in scope for this phase |
| V5 Input Validation | yes [VERIFIED: adapter DTOs convert into typed domain IDs and quantities] | existing typed DTO and newtype validation in `adapter-http` and `example-commerce` |
| V6 Cryptography | no [VERIFIED: throughput phase does not introduce crypto primitives] | keep existing UUID/idempotency patterns; do not hand-roll crypto |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection | Tampering | Keep explicit bound parameters via `sqlx`; Phase 13.1 should reduce query count, not switch to string-built SQL. [VERIFIED: crates/es-store-postgres/src/sql.rs] [CITED: https://docs.rs/sqlx/latest/sqlx/struct.Pool.html] |
| Overload by unbounded ingress | Denial of Service | Preserve bounded `mpsc` ingress and explicit overload errors while increasing shard worker concurrency behind it. [VERIFIED: crates/es-runtime/src/gateway.rs] [CITED: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html] |
| High-cardinality telemetry | Denial of Service | Keep using bounded labels from `observability.rs`; do not add per-command IDs to stress-specific metrics. [VERIFIED: crates/app/src/observability.rs] |
| Silent false-negative performance reports | Repudiation | Surface scrape failures and workload-shape metadata so “0 latency” or “hot-key” cannot be misread as factual. [VERIFIED: local measurements] [VERIFIED: crates/app/src/http_stress.rs] [ASSUMED] |

## Sources

### Primary (HIGH confidence)

- Repo code: `crates/es-runtime/src/engine.rs`, `crates/es-runtime/src/shard.rs`, `crates/es-runtime/src/router.rs`, `crates/es-store-postgres/src/sql.rs`, `crates/es-store-postgres/src/rehydrate.rs`, `crates/app/src/serve.rs`, `crates/app/src/http_stress.rs`, `crates/example-commerce/src/order.rs`, `crates/app/src/observability.rs`, `docs/stress-results.md`. [VERIFIED: repo files]
- Planning artifacts: `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/phases/13-live-external-process-http-steady-state-stress-testing/13-VERIFICATION.md`. [VERIFIED: repo files]
- Local measurements run during this research: `cargo run -q -p app -- stress-smoke`; `cargo run -q -p app -- http-stress --profile baseline --warmup-seconds 1 --measure-seconds 5 --concurrency 8 --shard-count 1 --ingress-capacity 256 --ring-size 256`; same with `--shard-count 8`. [VERIFIED: local commands]
- Crate metadata: `cargo info disruptor --locked`, `cargo info tokio --locked`, `cargo info reqwest --locked`, `cargo info sqlx --locked`, `cargo info hdrhistogram --locked`, `cargo info sysinfo --locked`. [VERIFIED: local commands]

### Secondary (MEDIUM confidence)

- Tokio `mpsc` docs: https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html [CITED]
- Tokio `MissedTickBehavior` docs: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html [CITED]
- Reqwest `Client` docs: https://docs.rs/reqwest/latest/reqwest/struct.Client.html [CITED]
- Sysinfo docs: https://docs.rs/sysinfo/latest/sysinfo/ [CITED]
- Disruptor crate docs: https://docs.rs/disruptor/latest/disruptor/ [CITED]
- SQLx pool docs: https://docs.rs/sqlx/latest/sqlx/struct.Pool.html [CITED]

### Tertiary (LOW confidence)

- None. [VERIFIED: research log]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH - Phase 13.1 can and should reuse the existing workspace stack; the bottlenecks are in topology and measurement semantics, not missing libraries. [VERIFIED: Cargo.toml] [VERIFIED: repo code review]
- Architecture: HIGH - identical 1-shard and 8-shard live throughput, plus the single-receiver engine design, make the shard-serialization diagnosis strong. [VERIFIED: crates/es-runtime/src/engine.rs] [VERIFIED: crates/app/src/serve.rs] [VERIFIED: local measurements]
- Pitfalls: HIGH - the false hot-key profile, 1 ms offered-load cap, and zero-valued diagnostics are directly visible in code and reproduced in local output. [VERIFIED: crates/app/src/http_stress.rs] [VERIFIED: local measurements]

**Research date:** 2026-04-26 [VERIFIED: local system date]
**Valid until:** 2026-05-26 for repo-internal architecture findings; rerun local measurements sooner if the runtime or stress harness changes materially. [ASSUMED]

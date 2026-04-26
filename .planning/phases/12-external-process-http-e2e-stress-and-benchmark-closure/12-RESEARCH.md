# Phase 12: External-Process HTTP E2E, Stress, and Benchmark Closure - Research

**Researched:** 2026-04-25
**Domain:** Runnable HTTP service verification, external-process load generation, and benchmark/report separation for the Disruptor ES template
**Confidence:** HIGH

## User Constraints

### Locked Phase Scope
- Phase 12 exists to replace the misleading in-process “full E2E” path with canonical external-process HTTP workloads that drive the real `app serve` process. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- The phase must cover `API-01`, `API-03`, `TEST-03`, `TEST-04`, and `OBS-02`, and it should preserve the distinction between ring-only, single-process integrated, and external-process measurements. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/REQUIREMENTS.md]
- The user explicitly wants real serving-path measurements, not internal `CommandEnvelope` shortcuts mislabeled as end-to-end. [VERIFIED: .planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-CONTEXT.md]
- Phase 12 depends on Phase 11’s official runnable service path and should reuse that substrate instead of inventing a second HTTP composition. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md] [VERIFIED: crates/app/src/serve.rs]

### Project Constraints
- The event store remains the source of truth and the HTTP adapter must stay gateway-only; external-process coverage should still hit `adapter_http::router(HttpState)` rather than bypassing into runtime internals. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: docs/hot-path-rules.md] [VERIFIED: crates/app/src/serve.rs] [VERIFIED: crates/adapter-http/src/commerce.rs]
- The repository currently has only one external-process proof path: `crates/app/tests/serve_smoke.rs`, which boots the real binary, waits for `/healthz`, and submits one real HTTP request. [VERIFIED: crates/app/tests/serve_smoke.rs]
- The current stress harness lives in-process inside `crates/app/src/stress.rs`; its `FullE2eInProcess` label is now the main naming defect Phase 12 must close. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- Existing benchmark lanes already cover `ring_only`, `domain_only`, `adapter_only`, `storage_only`, and `projector_outbox`; Phase 12 should add an external-process HTTP lane rather than collapsing these categories together. [VERIFIED: Cargo.toml] [VERIFIED: docs/stress-results.md]

### Deferred Ideas
- Final milestone debt closure, reopened prior artifacts, and archive sign-off belong to Phase 14. [VERIFIED: .planning/ROADMAP.md]
- Phase 12 should not turn into a generalized deployment/ops automation effort; the focus is executable-service workloads, evidence, and measurement clarity. [VERIFIED: .planning/phases/12-external-process-http-e2e-stress-and-benchmark-closure/12-CONTEXT.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| API-01 | Thin HTTP adapter exposes command endpoints that decode requests, attach metadata, send through bounded ingress, and await command replies. [VERIFIED: .planning/REQUIREMENTS.md] | The best way to re-prove this requirement at Phase 12 scope is with external-process tests that launch `app serve` and submit actual HTTP requests over the network boundary rather than using in-memory `router().oneshot(...)` alone. [VERIFIED: crates/app/tests/serve_smoke.rs] [VERIFIED: crates/adapter-http/tests/commerce_api.rs] |
| API-03 | API responses include stream revision, global position, correlation ID, and typed success/error payloads. [VERIFIED: .planning/REQUIREMENTS.md] | External-process E2E tests can assert the same response-contract fields currently covered in `commerce_api.rs`, but now through the real binary/process boundary. [VERIFIED: crates/adapter-http/tests/commerce_api.rs] [VERIFIED: crates/app/tests/serve_smoke.rs] |
| TEST-03 | Benchmark harnesses separately measure ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded dependency scenarios. [VERIFIED: .planning/REQUIREMENTS.md] | The repository already has every lane except an honest external-process HTTP full-E2E lane. Phase 12 should add that lane and keep it visibly separate from the existing in-process stress and Criterion microbenchmarks. [VERIFIED: Cargo.toml] [VERIFIED: benches/ring_only.rs] [VERIFIED: benches/domain_only.rs] [VERIFIED: benches/adapter_only.rs] [VERIFIED: benches/storage_only.rs] [VERIFIED: benches/projector_outbox.rs] [VERIFIED: docs/stress-results.md] |
| TEST-04 | A single-service integrated stress test runs the production-shaped composition in one service process and reports throughput, p50/p95/p99 latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization under realistic traffic. [VERIFIED: .planning/REQUIREMENTS.md] | Phase 7/11 already proved the in-process integrated and runnable-service parts, but Phase 12 must add the real service-process HTTP workload that measures those fields against `app serve` instead of an internal shortcut. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: crates/app/tests/serve_smoke.rs] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| OBS-02 | Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, OCC conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency. [VERIFIED: .planning/REQUIREMENTS.md] | The current stress report already emits throughput, latency percentiles, queue depth, append latency, projection lag, outbox lag, reject rate, CPU, and core count. Phase 12 should preserve these report fields for the new external-process lane and document how they differ from in-process measurements. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: docs/stress-results.md] |

</phase_requirements>

## Summary

Phase 12 should stay split into two plans, matching the roadmap:

1. **Plan 12-01 — External-process harness and canonical request scenarios**
   Extract a reusable external-process app-process harness from `serve_smoke`, then add canonical end-to-end request scenarios that assert success, error/overload behavior, and durable response metadata through the real HTTP service path.
2. **Plan 12-02 — Naming cleanup, HTTP stress/benchmark lane, and reporting docs**
   Remove the archive-facing ambiguity around `FullE2eInProcess`, add an explicit external-process HTTP stress/benchmark lane, and update docs/report labels so future evidence cannot confuse in-process and external-process numbers.

This split matches the work already visible in the repository:
- external-process process-control primitives already exist, but only in one smoke test [VERIFIED: crates/app/tests/serve_smoke.rs]
- response-contract checks already exist, but only in in-memory adapter tests [VERIFIED: crates/adapter-http/tests/commerce_api.rs]
- stress metrics/report fields already exist, but only for the in-process harness [VERIFIED: crates/app/src/stress.rs]
- benchmark-layer separation already exists, but the external-process HTTP lane is still missing [VERIFIED: Cargo.toml] [VERIFIED: docs/stress-results.md]

**Primary recommendation:** reuse and generalize the existing `serve_smoke` process harness instead of creating a second app-launch path. The cheapest correct design is: one reusable process-test support layer, one external-process E2E test surface, and one explicit external-process HTTP stress/benchmark lane.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Launch real service process | `crates/app/tests` support harness | `crates/app/src/serve.rs` | The binary already owns service composition; tests should spawn it rather than recompose the app in-process. |
| Canonical external-process response-contract verification | `crates/app/tests` | `crates/adapter-http/tests` | Adapter tests remain the fast contract/unit layer; Phase 12 adds process-boundary proof above them. |
| In-process integrated stress | `crates/app/src/stress.rs` | `docs/stress-results.md` | Keep this lane because it is still diagnostically useful. |
| External-process HTTP stress lane | `crates/app` runtime helper and/or workspace bench | `crates/app/tests` smoke coverage | Needs reusable client/process orchestration and machine-readable reporting. |
| Benchmark-lane separation | workspace `Cargo.toml` benches + docs | `.planning` artifacts | Source-of-truth labels must keep microbenchmarks, in-process stress, and external-process HTTP measurements distinct. |
| Service-path documentation | `docs/template-guide.md`, `docs/stress-results.md` | `.planning` artifacts | Operators need one authoritative explanation of how to run and compare these workloads. |

## Standard Stack

### Core

| Library / Crate | Version | Purpose | Why Standard |
|-----------------|---------|---------|--------------|
| `app` crate | workspace `0.1.0` | owns service composition, CLI entrypoint, and existing stress harness | Natural owner for any reusable external-process HTTP workload helpers. [VERIFIED: crates/app/Cargo.toml] [VERIFIED: crates/app/src/main.rs] [VERIFIED: crates/app/src/serve.rs] |
| `adapter-http` | workspace `0.1.0` | request decode and JSON response mapping | External-process E2E should continue to prove this surface instead of bypassing it. [VERIFIED: crates/adapter-http/src/lib.rs] [VERIFIED: crates/adapter-http/src/commerce.rs] |
| PostgreSQL via Testcontainers | workspace dependencies | realistic external-process service runs in tests and benches | Existing smoke path already uses this and should remain the realism baseline. [VERIFIED: crates/app/tests/serve_smoke.rs] [VERIFIED: Cargo.toml] |
| Criterion benches | `criterion 0.7.0` | benchmark lane registration | Existing benchmark structure already lives at workspace root. [VERIFIED: Cargo.toml] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio` | `1.52.0` | async task orchestration and test runtime | Reuse for client concurrency, timeouts, and process wait/retry loops. [VERIFIED: Cargo.toml] |
| raw `TcpStream` or a single explicit HTTP client dependency | existing stdlib / optional new dependency | external-process request generation | Current smoke test proves raw TCP is viable; a dedicated client is acceptable if it simplifies repeated HTTP runs without obscuring the real boundary. [VERIFIED: crates/app/tests/serve_smoke.rs] |
| `hdrhistogram` | `7.5.4` | percentile reporting | Already used by the in-process stress harness; keep report-field semantics aligned. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/stress.rs] |
| `sysinfo` | `0.36.1` | CPU/core reporting | Existing stress report already depends on it for utilization summaries. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/src/stress.rs] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Reusing `serve_smoke` process harness | Write a second bespoke process-launch helper inside each new test/bench | Duplicates startup, readiness, child-log, and shutdown logic; higher flake risk. |
| External-process E2E tests through `app serve` | More in-memory `router().oneshot(...)` tests | Faster, but does not prove the real process boundary, listener bind, or wire-format behavior. |
| Add an explicit external-process bench lane | Reuse current `FullE2eInProcess` label and treat it as “good enough” | This is the exact audit defect Phase 12 exists to close. |
| Keep current scenario naming | Rename/demote the in-process path | The current name keeps archive readers confused about what was actually measured. |

## Architecture Patterns

### System Architecture Diagram

```text
benchmark/test driver
  |
  +--> spawn `app serve` child process
  |       |
  |       +--> wait for `/healthz`
  |       +--> submit HTTP requests over TCP/client library
  |       +--> collect response metadata + latency
  |       +--> stop child cleanly and capture logs on failure
  |
  +--> produce external-process report
          |
          +--> keep labels separate from:
                  - ring_only
                  - domain_only
                  - adapter_only
                  - storage_only
                  - projector_outbox
                  - in-process integrated stress
```

### Pattern 1: Reusable child-process HTTP harness

**What:** Factor `free_listen_addr`, binary resolution, child spawn, readiness waiting, request helpers, and log capture out of `serve_smoke` into reusable support code.

**When to use:** Any integration test or benchmark that must prove behavior through the official `app serve` process.

**Example source:** `crates/app/tests/serve_smoke.rs`

### Pattern 2: Layered response-contract proving

**What:** Keep `adapter-http` contract tests as the fast inner layer, then mirror the most important success/error metadata assertions in external-process tests.

**When to use:** When API-01/API-03 need both local contract confidence and real process-boundary proof.

**Example source set:**
- `crates/adapter-http/tests/commerce_api.rs`
- `crates/app/tests/serve_smoke.rs`

### Pattern 3: Separate workload names by boundary

**What:** Names must state whether the workload is in-process or external-process. “Full E2E” without the boundary in the name is no longer acceptable.

**When to use:** Always for stress scenarios, docs, benchmark lanes, and `.planning` evidence references.

**Current defect evidence:** `StressScenario::FullE2eInProcess` plus `scenario_name(...) -> "full-e2e"`. [VERIFIED: crates/app/src/stress.rs]

### Pattern 4: Benchmark/report layering remains explicit

**What:** Add one external-process HTTP lane while keeping the existing microbenchmarks and in-process stress reports intact.

**When to use:** Whenever a benchmark artifact or documentation page could blur one measurement layer into another.

**Current evidence:** `docs/stress-results.md` already warns not to compare unlike layers, but the repository still lacks the actual external-process lane it references. [VERIFIED: docs/stress-results.md]

### Anti-Patterns to Avoid
- Treating `FullE2eInProcess` as acceptable archive-facing terminology after Phase 12. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- Re-implementing HTTP decode/response logic in the benchmark harness instead of driving `app serve`. [VERIFIED: .planning/REQUIREMENTS.md]
- Replacing existing in-process stress with the external-process lane; Phase 12 should add clarity, not erase the diagnostic layer. [VERIFIED: docs/stress-results.md]
- Publishing external-process benchmark numbers without the required stress fields already used by the repo’s stress reports. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: crates/app/src/stress.rs]

## Don’t Hand-Roll

| Problem | Don’t Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| app-process startup logic in every test | copy-pasted child-spawn/readiness code | one shared `tests/support` process harness | Fewer flakes, better failure logs, easier reuse. |
| “full E2E” label reuse | keep current scenario name and add a footnote | explicit in-process vs external-process names | Prevents future audit confusion. |
| benchmark lane overloading | stuff HTTP numbers into existing benches/docs | dedicated external-process bench/report lane | Preserves TEST-03 separation. |
| proof from adapter-only tests | rely only on `router().oneshot(...)` | external-process tests against `app serve` | API-01/API-03 need real boundary proof here. |

## Common Pitfalls

### Pitfall 1: confusing readiness smoke with E2E coverage
**What goes wrong:** `serve_smoke` proves boot and one happy-path request, but phase evidence falsely treats that as complete E2E/stress closure.
**How to avoid:** Reuse the smoke harness, but add separate external-process request-scenario tests and explicit stress/benchmark commands.

### Pitfall 2: leaving `FullE2eInProcess` in docs or JSON labels
**What goes wrong:** Even after adding the new lane, archive evidence still points to the misleading old name.
**How to avoid:** Remove or demote the label everywhere: enum variant names, scenario strings, docs, and `.planning` references.

### Pitfall 3: benchmarking through the wrong layer
**What goes wrong:** Reports mix Criterion microbenchmarks, in-process stress, and external-process HTTP numbers into one “throughput” narrative.
**How to avoid:** Keep a dedicated external-process HTTP report/bench lane and document how it differs from the existing layers.

### Pitfall 4: process-test flakiness from duplicated harness code
**What goes wrong:** Timeouts, orphaned child processes, and weak failure logs make CI debugging miserable.
**How to avoid:** Centralize port reservation, readiness polling, child shutdown, and log capture in reusable test support.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The existing `serve_smoke` harness is the correct foundation for all external-process Phase 12 work. | harness design | Low; it already launches the real binary successfully. |
| A2 | The repo should keep both in-process and external-process workload lanes after Phase 12. | workload layering | Low; roadmap and docs already imply both lanes are valuable when clearly labeled. |
| A3 | A dedicated external-process Criterion bench or equivalent bench-like command is acceptable for satisfying TEST-03 without replacing existing microbench files. | benchmark plan | Medium; if rejected, Phase 12 would need a different explicit benchmark entrypoint. |

## Open Questions (RESOLVED)

1. **Should Phase 12 replace the in-process stress harness?**
   - Resolved: no. It should rename/demote the misleading path and add the external-process lane beside it.
2. **Should external-process contract checks live in `adapter-http` tests?**
   - Resolved: no. They should live above the binary boundary in `crates/app/tests`, while adapter tests remain the fast inner contract layer.
3. **Should docs wait until after implementation?**
   - Resolved: no. The naming/reporting cleanup is part of the acceptance criteria, not optional polish.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler / Cargo | all Phase 12 implementation and verification | ✓ | workspace baseline | none |
| PostgreSQL Testcontainers harness | realistic external-process app runs | ✓ | `testcontainers 0.25.0`, `testcontainers-modules 0.13.0` | document Docker blockers if local runtime is unavailable |
| `app serve` binary path | external-process tests/benches | ✓ | built from workspace | current `CARGO_BIN_EXE_app` + fallback path already exists in `serve_smoke` |
| Existing stress metrics/report plumbing | external-process report parity | ✓ | repo-local | reuse current stress-report field names and semantics |

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust integration tests plus workspace benches |
| Config file | workspace `Cargo.toml` and `crates/app/Cargo.toml` |
| Quick run command | `cargo test -p app serve_smoke -- --nocapture && cargo test -p app external_process_http -- --nocapture` |
| Full suite command | `cargo test --workspace && cargo bench --bench external_process_http -- --sample-size 10` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| API-01 | real HTTP command submission flows through the binary/service path | external-process integration | `cargo test -p app external_process_http_success_path -- --nocapture` | Missing |
| API-03 | success/error replies include durable metadata through the real process boundary | external-process integration | `cargo test -p app external_process_http_metadata_contract -- --nocapture` | Missing |
| TEST-03 | external-process benchmark lane exists and is explicitly labeled | workspace bench | `cargo bench --bench external_process_http -- --sample-size 10` | Missing |
| TEST-04 | external-process HTTP stress run reports throughput/latency/depth/lag fields | targeted stress smoke / CLI or equivalent | `cargo test -p app external_process_http_stress_smoke -- --nocapture` | Missing |
| OBS-02 | external-process report preserves required metrics fields | targeted stress smoke + docs grep | `rg -n "throughput_per_second|p95|projection_lag|outbox_lag|reject_rate|cpu_utilization_percent" crates/app/src docs/stress-results.md` | Partial |

### Sampling Rate
- **After harness extraction:** rerun `serve_smoke` before building new scenarios.
- **After canonical E2E scenarios:** run targeted external-process contract tests before stress/bench work.
- **After naming/report cleanup:** rerun the in-process stress smoke and the new external-process stress smoke so labels and metrics stay aligned.
- **Before phase sign-off:** run targeted app tests plus the external-process bench entrypoint; then run `cargo test --workspace` if environment/time allow.

### Wave 0 Gaps
- [ ] Add reusable external-process service-process test support under `crates/app/tests/support/`.
- [ ] Add canonical external-process HTTP E2E tests beyond the single `serve_smoke` happy path.
- [ ] Replace or demote `FullE2eInProcess` naming in code and docs.
- [ ] Add an explicit external-process HTTP stress/benchmark lane with report-field parity.
- [ ] Update documentation so benchmark/report consumers cannot confuse in-process and external-process numbers.

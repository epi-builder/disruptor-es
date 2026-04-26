---
phase: 13-live-external-process-http-steady-state-stress-testing
verified: 2026-04-26T06:50:27Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 13: Live External-Process HTTP Steady-State Stress Testing Verification Report

**Phase Goal:** Live External-Process HTTP Steady-State Stress Testing. Verify the implementation measures a real spawned `app serve` process over warmup plus measured steady-state windows, excludes startup/warmup from reported counters, exposes bounded CLI profiles, keeps Criterion secondary, and documents interpretation.
**Verified:** 2026-04-26T06:50:27Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Developer can run a documented external-process HTTP stress command that keeps one `app serve` process alive across warmup and measurement windows. | ✓ VERIFIED | [`run_external_process_http_stress`](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:286) validates config, spawns one child via [`ExternalProcessHarness::spawn`](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:228), waits for readiness, then runs warmup and measured windows against the same harness; CLI entrypoint is wired in [crates/app/src/main.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/main.rs:223). |
| 2 | The measured interval excludes PostgreSQL container startup, service process boot, migration, readiness probing, and benchmark harness compilation. | ✓ VERIFIED | Postgres/container setup, binary build, child spawn, and health probing happen before measurement starts in [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:229), [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:640), [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:671), and [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:742); measured state is reset after warmup and before the sampler plus latency histogram start in [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:301). |
| 3 | Reports include sustained throughput, p50/p95/p99/max latency, success/error/reject counts, reject rate, append latency, ingress/shard depth, projection lag, outbox lag, CPU/core count, run duration, concurrency, and environment metadata. | ✓ VERIFIED | Phase 13 populates all required fields when building `StressReport` in [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:348), the report struct includes those fields in [crates/app/src/stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/stress.rs:128), and JSON output exposes them in [crates/app/src/main.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/main.rs:7). |
| 4 | The stress lane supports at least smoke, baseline, burst, and hot-key style profiles without conflating them with Criterion microbenchmarks. | ✓ VERIFIED | The four bounded presets and validation live in [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:63); CLI flags expose profile and bounded overrides in [crates/app/src/main.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/main.rs:5); the Criterion bench explicitly reuses the smoke profile and states `app http-stress` is authoritative in [benches/external_process_http.rs](/Users/epikem/dev/projects/disruptor-es/benches/external_process_http.rs:13). |
| 5 | Documentation explains how to interpret steady-state live HTTP results separately from Phase 12 external-process smoke benchmarks and in-process integrated stress. | ✓ VERIFIED | [docs/stress-results.md](/Users/epikem/dev/projects/disruptor-es/docs/stress-results.md:32) and [docs/template-guide.md](/Users/epikem/dev/projects/disruptor-es/docs/template-guide.md:99) distinguish Phase 13 steady-state JSON from Criterion and in-process stress, list the supported commands, and explain excluded startup/warmup costs plus deadline semantics. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/app/src/http_stress.rs` | Duration-window external HTTP stress runner with safe profile validation | ✓ VERIFIED | Presets, exact bounds, single-child harness, warmup reset, measured window, deadline policy, sampler, and smoke/tests are implemented. |
| `crates/app/src/http_stress.rs` | Measured-window report fields and resource sampling | ✓ VERIFIED | Report population and sampler logic are present and measured-window scoped. |
| `crates/app/src/http_stress.rs` | Measured-window deadline policy and run metadata | ✓ VERIFIED | `run_duration_seconds`, `concurrency`, `deadline_policy`, and `drain_timeout_seconds` are set from the measured run. |
| `crates/app/src/main.rs` | CLI entrypoint for configurable live HTTP stress | ✓ VERIFIED | `http-stress` parses the bounded flags and prints JSON. |
| `benches/external_process_http.rs` | Criterion lane explicitly demoted to non-authoritative smoke/baseline use | ✓ VERIFIED | Bench wraps the smoke profile only and labels Criterion as secondary. |
| `docs/stress-results.md` | Steady-state interpretation guidance | ✓ VERIFIED | Separates steady-state live HTTP from Phase 12 and Criterion evidence. |
| `docs/template-guide.md` | Operator run commands for live HTTP stress profiles | ✓ VERIFIED | Documents exact Phase 13 commands and interpretation boundaries. |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `crates/app/src/http_stress.rs` | `app serve` child process | `ExternalProcessHarness::spawn` and `wait_for_health` | ✓ VERIFIED | `Command::new(binary).arg("serve")` plus readiness probing before warmup/measurement. |
| `crates/app/src/http_stress.rs` | steady-state report | warmup reset followed by measured window accumulation | ✓ VERIFIED | Warmup runs first, metrics baseline is captured, measured state is reset, then measured counters/histogram are accumulated. |
| `crates/app/src/http_stress.rs` | Prometheus metrics | bounded interval sampler | ✓ VERIFIED | `spawn_metric_sampler` uses `tokio::time::interval` over the measured deadline only. |
| `crates/app/src/main.rs` | `crates/app/src/http_stress.rs` | CLI parsing and config construction | ✓ VERIFIED | `parse_http_stress_args` builds validated `HttpStressConfig`, then `main` calls `run_external_process_http_stress`. |
| `benches/external_process_http.rs` | `crates/app/src/http_stress.rs` | Criterion wrapper uses smoke-only steady-state config | ✓ VERIFIED | Bench calls `run_external_process_http_stress(HttpStressConfig::from_profile(HttpStressProfile::Smoke))`. |
| `docs/stress-results.md` | `docs/template-guide.md` | shared operator language for steady-state vs Criterion vs in-process lanes | ✓ VERIFIED | Both docs use the same distinctions and commands. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| --- | --- | --- | --- | --- |
| `crates/app/src/http_stress.rs` | `counters`, `latency`, `measured.metrics`, `cpu_usage_samples` | Live HTTP requests to the spawned `app serve` process plus measured-window `/metrics` scrapes and `sysinfo` CPU refreshes | Yes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| Profile presets and bounds exist in code | `cargo test -p app http_stress_profile_presets_cover_phase13_profiles -- --nocapture` | 1 test passed | ✓ PASS |
| Report omits sensitive env fields | `cargo test -p app stress_report_omits_sensitive_environment_fields -- --nocapture` | 1 test passed | ✓ PASS |
| Baseline steady-state external-process run works | `cargo run -p app -- http-stress --profile baseline --warmup-seconds 5 --measure-seconds 30 --concurrency 8` | Orchestrator evidence: 2414 submitted, 2414 succeeded, 0 failed/rejected, `throughput_per_second=80.20627059002098`, `p95_micros=104255`, `run_duration_seconds=30.097397401` | ✓ PASS |
| Burst and hot-key profiles work as bounded steady-state lanes | `cargo run -p app -- http-stress --profile burst` and `cargo run -p app -- http-stress --profile hot-key` | Orchestrator evidence: burst `concurrency=32`, hot-key `concurrency=16`, both completed with 0 failed/rejected and steady-state summaries | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| `TEST-03` | `13-02-PLAN.md` | Benchmark harnesses separately measure ring-only, domain-only, adapter-only, storage-only, single-service integrated, full E2E, projector/outbox, hot-key, burst, and degraded dependency scenarios. | ✓ SATISFIED | Phase 13 adds the documented live external-process HTTP lane with smoke/baseline/burst/hot-key profiles while keeping Criterion secondary in [benches/external_process_http.rs](/Users/epikem/dev/projects/disruptor-es/benches/external_process_http.rs:16) and [docs/stress-results.md](/Users/epikem/dev/projects/disruptor-es/docs/stress-results.md:34). |
| `TEST-04` | `13-01-PLAN.md`, `13-02-PLAN.md` | A single-service integrated stress test runs the production-shaped composition in one service process and reports throughput, p50/p95/p99 latency, queue depths, append latency, projection lag, outbox lag, reject rate, and CPU/core utilization under realistic traffic. | ✓ SATISFIED | The spawned `app serve` process is measured over an isolated steady-state window and returns all required report fields in [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:348) and [crates/app/src/main.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/main.rs:7). |
| `OBS-02` | `13-01-PLAN.md`, `13-02-PLAN.md` | Metrics expose ingress depth, shard queue depth, ring wait, decision latency, append latency, OCC conflicts, dedupe hits, projection lag, outbox lag, and p95/p99 command latency. | ✓ SATISFIED | The measured-window sampler scrapes ingress depth, shard depth, append latency delta, projection lag, and outbox lag from Prometheus during the measured interval only in [crates/app/src/http_stress.rs](/Users/epikem/dev/projects/disruptor-es/crates/app/src/http_stress.rs:526). |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| --- | --- | --- | --- | --- |
| - | - | None in reviewed phase files | - | No blocking stubs, placeholders, empty implementations, or secret-reporting fields found. |

### Gaps Summary

No goal-blocking gaps found. The implementation satisfies the roadmap success criteria and both plan contracts: it measures one spawned `app serve` process across explicit readiness, warmup, and measured windows; resets measured state between warmup and measurement; exposes bounded profile-driven CLI controls with no arbitrary target override; keeps Criterion as a secondary smoke wrapper; and documents how to interpret Phase 13 steady-state evidence separately from Phase 12 and in-process stress.

---

_Verified: 2026-04-26T06:50:27Z_
_Verifier: Claude (gsd-verifier)_

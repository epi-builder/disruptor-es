# Phase 12: External-Process HTTP E2E, Stress, and Benchmark Closure - Pattern Map

**Mapped:** 2026-04-25
**Files analyzed:** 11 expected new/modified files across tests, app runtime helpers, benches, docs, and planning artifacts
**Analogs found:** 11 / 11

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `Cargo.toml` | workspace bench registration / shared dependency exposure | benchmark + build config | current bench declarations | exact |
| `crates/app/Cargo.toml` | app-level test/client dependency wiring | build/test config | current app manifest | exact |
| `crates/app/tests/support/mod.rs` | shared integration-test support module | process harness glue | existing single-file `serve_smoke.rs` helpers | role-match |
| `crates/app/tests/support/http_process.rs` | spawn/wait/request/log external-process helpers | test process control | `crates/app/tests/serve_smoke.rs` | role-match |
| `crates/app/tests/serve_smoke.rs` | narrow readiness smoke | external-process integration | current file | exact |
| `crates/app/tests/external_process_http.rs` | canonical external-process E2E scenarios | external-process integration | `crates/adapter-http/tests/commerce_api.rs` + `serve_smoke.rs` | role-match |
| `crates/app/src/stress.rs` | in-process stress lane naming/report alignment | in-process metrics | current file | exact |
| `crates/app/src/http_stress.rs` | reusable external-process HTTP stress runner/reporting | external-process metrics | `crates/app/src/stress.rs` + `serve_smoke.rs` | role-match |
| `crates/app/src/main.rs` | thin CLI dispatch for external-process stress entrypoint if added | operator entrypoint | current `serve` / `stress-smoke` dispatch | exact |
| `benches/external_process_http.rs` | explicit external-process benchmark lane | benchmark | existing `ring_only.rs` / `storage_only.rs` bench structure | role-match |
| `docs/stress-results.md`, `docs/template-guide.md` | reporting and operator guidance | docs | current files | exact |

## Pattern Assignments

### Pattern 1: Extract reusable external-process harness from `serve_smoke`
**Apply to:** `crates/app/tests/support/mod.rs`, `crates/app/tests/support/http_process.rs`, `crates/app/tests/serve_smoke.rs`

Use the current `serve_smoke.rs` helper set as the source pattern:
- ephemeral listen-port reservation
- binary path resolution via `CARGO_BIN_EXE_app` fallback
- child spawn with env-driven config
- `/healthz` readiness polling
- failure-time child log capture
- explicit child shutdown

The new support layer should be a straight generalization of this logic, not a second independent startup path.

### Pattern 2: Mirror adapter contract assertions at the real process boundary
**Apply to:** `crates/app/tests/external_process_http.rs`

Use `crates/adapter-http/tests/commerce_api.rs` as the contract source and `serve_smoke.rs` as the transport/process source. The external-process tests should reassert the most important real-wire truths:
- success responses include reply + stream metadata
- overload/conflict/error mapping still serializes correctly
- retry/idempotency behavior remains visible through HTTP responses when practical

### Pattern 3: Keep CLI shell thin, move workload logic into library modules
**Apply to:** `crates/app/src/main.rs`, `crates/app/src/http_stress.rs`

Follow the same architecture used for `serve` and `stress-smoke`: `main.rs` should dispatch only, while reusable workload logic lives in a library module. If Phase 12 adds a new executable entrypoint (for example `http-stress`), the implementation belongs in `crates/app/src/http_stress.rs`, not inline in the binary shell.

### Pattern 4: Preserve report schema parity across stress lanes
**Apply to:** `crates/app/src/http_stress.rs`, `crates/app/src/stress.rs`, `docs/stress-results.md`

Use the current `StressReport` field set as the baseline schema:
- throughput
- latency percentiles
- queue depth
- append latency
- projection lag
- outbox lag
- reject rate
- CPU/core metrics

The external-process lane can add boundary-specific fields if useful, but it must not silently drop the existing required fields from archive-facing reports.

### Pattern 5: Register a dedicated benchmark lane instead of overloading existing ones
**Apply to:** `Cargo.toml`, `benches/external_process_http.rs`

Follow the Criterion bench registration pattern already used by:
- `benches/ring_only.rs`
- `benches/domain_only.rs`
- `benches/adapter_only.rs`
- `benches/storage_only.rs`
- `benches/projector_outbox.rs`

The Phase 12 lane should have its own bench target and its own explanatory header so external-process numbers cannot be mistaken for component benchmarks.

### Pattern 6: Documentation updates land in existing canonical docs
**Apply to:** `docs/stress-results.md`, `docs/template-guide.md`

Do not create a one-off Phase 12 operator note. Update the two documents that already define:
- how `app serve` is run
- how stress layers are interpreted
- which measurements should and should not be compared

## Anti-Patterns to Avoid

- Leaving `FullE2eInProcess` in code/docs while adding a new external-process lane beside it.
- Re-embedding child-process startup helpers separately in every new test and bench file.
- Treating `serve_smoke` as the canonical stress benchmark instead of a narrow readiness smoke.
- Dumping external-process throughput into `docs/stress-results.md` without preserving the “Do Not Compare” section’s layer distinctions.
- Adding a thick CLI parser or deployment-focused config system just to run Phase 12 workloads.

## Expected Implementation Shape

### Test Support Shape
```text
crates/app/tests/
  support/
    mod.rs
    http_process.rs   <- spawn app, wait for health, send request, capture logs
  serve_smoke.rs      <- slim smoke test reusing support helpers
  external_process_http.rs <- canonical E2E scenarios reusing support helpers
```

### Runtime / Bench Shape
```text
crates/app/src/
  stress.rs       <- keep in-process lane, rename misleading scenario
  http_stress.rs  <- external-process HTTP stress runner/report
  main.rs         <- thin dispatch only

benches/
  external_process_http.rs <- Criterion lane for external-process HTTP baseline
```

### Documentation Shape
```text
docs/template-guide.md   <- how to run app serve + external-process harness

docs/stress-results.md   <- how to interpret external-process vs in-process vs microbench numbers
```

---
phase: 07
slug: adapters-observability-stress-and-template-guidance
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-18
---

# Phase 07 - Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness with Tokio async tests, Testcontainers PostgreSQL, and Criterion 0.7.0 for benches |
| **Config file** | Root `Cargo.toml`; no separate nextest config exists |
| **Quick run command** | `cargo test --workspace --no-run` plus targeted package tests |
| **Full suite command** | `cargo test --workspace -- --nocapture` plus applicable `cargo bench --bench <name>` smoke runs |
| **Estimated runtime** | ~180 seconds for full tests; benchmark smoke runtime varies by bench |

---

## Sampling Rate

- **After every task commit:** Run the relevant package tests plus `cargo test --workspace --no-run`.
- **After every plan wave:** Run `cargo test --workspace -- --nocapture`; run quick smoke benches if benchmark code changed.
- **Before `$gsd-verify-work`:** Full workspace tests must be green, PostgreSQL integration tests must pass with Docker available, benchmark/stress artifacts must be generated or smoke-validated, and docs grep checks must pass.
- **Max feedback latency:** 240 seconds for targeted checks; heavy stress/bench runs may be explicit phase-gate commands.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | API-01 / API-03 | T-07-01 / T-07-02 / T-07-03 | HTTP command DTOs validate tenant/idempotency metadata and submit only through bounded gateway replies. | unit/integration | `cargo test -p adapter-http commerce_api -- --nocapture`<br>`cargo test -p adapter-http response_contract -- --nocapture` | No - Wave 0 | pending |
| 07-01-02 | 01 | 1 | API-02 | T-07-04 | Adapter dependencies and imports do not expose aggregate caches, projector mutation APIs, outbox stores, or direct event-store writes. | dependency-boundary | `cargo test -p adapter-http dependency_boundaries -- --nocapture` | No - Wave 0 | pending |
| 07-02-01 | 02 | 2 | OBS-01 / OBS-02 | T-07-05 / T-07-06 | Trace fields carry command identity while metric labels stay bounded and non-cardinality-explosive. | unit/integration | `cargo test -p app observability_traces -- --nocapture`<br>`cargo test -p app observability_metrics -- --nocapture` | No - Wave 0 | pending |
| 07-03-01 | 03 | 3 | TEST-02 | T-07-07 / T-07-08 | PostgreSQL-backed tests verify append, conflicts, dedupe, snapshots, projector checkpoints, and outbox dispatch through real storage semantics. | integration | `cargo test -p es-store-postgres -- --nocapture` | Yes | pending |
| 07-04-01 | 04 | 4 | TEST-03 | T-07-09 | Benchmark harnesses remain layer-separated and do not report ring-only results as service throughput. | bench smoke | `cargo bench --bench ring_only -- --warm-up-time 1 --measurement-time 3` | No - Wave 0 | pending |
| 07-05-01 | 05 | 5 | TEST-04 | T-07-10 / T-07-11 | Single-service stress uses bounded ingress, records reject rate, and reports required latency/lag/CPU metrics without making projection/outbox part of command success. | integration/stress | `cargo test -p app single_service_stress_smoke -- --nocapture` | No - Wave 0 | pending |
| 07-06-01 | 06 | 6 | API-04 / DOC-01 | T-07-12 | Documentation states gateway boundaries, hot-path rules, and forbidden direct-publish/direct-state-mutation patterns. | doc validation | `rg "event store is the source of truth|CommandGateway|single-owner|outbox|ring-only" docs` | No - Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `crates/adapter-http/src/commerce.rs` - command DTOs, handlers, metadata/envelope mapping for API-01/API-03.
- [ ] `crates/adapter-http/src/error.rs` - typed `ApiError`/`IntoResponse` mapping for API-03.
- [ ] `crates/adapter-http/tests/commerce_api.rs` - fake-gateway adapter tests for API-01/API-03.
- [ ] `crates/adapter-http/tests/dependency_boundaries.rs` - verifies API-02 imports/boundaries.
- [ ] `crates/app/src/observability.rs` - subscriber/exporter setup and metric descriptions for OBS-01/OBS-02.
- [ ] `crates/app/src/stress.rs` - single-service integrated stress core for TEST-04.
- [ ] `benches/ring_only.rs`, `benches/domain_only.rs`, `benches/adapter_only.rs`, `benches/storage_only.rs`, `benches/projector_outbox.rs` - TEST-03 artifacts.
- [ ] `docs/hot-path-rules.md`, `docs/template-guide.md`, `docs/stress-results.md` - DOC-01 and API-04 guidance.

---

## Manual-Only Verifications

All Phase 07 behaviors should have automated validation or smoke commands. Real external telemetry backends and production load generation are out of scope for this phase; local exporter initialization and stress summaries are sufficient.

---

## Validation Sign-Off

- [ ] All tasks have automated verify commands or Wave 0 dependencies.
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify.
- [ ] Wave 0 covers all missing references.
- [ ] No watch-mode flags.
- [ ] Feedback latency target is documented.
- [ ] `nyquist_compliant: true` set in frontmatter after implementation validation passes.

**Approval:** pending

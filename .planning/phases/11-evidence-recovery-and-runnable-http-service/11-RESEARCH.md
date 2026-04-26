# Phase 11: Evidence Recovery and Runnable HTTP Service - Research

**Researched:** 2026-04-21
**Domain:** Rust event-sourced service archive hygiene, runnable HTTP composition, and milestone evidence recovery
**Confidence:** HIGH

## User Constraints

### Locked Phase Scope
- Phase 11 restores the missing verification/evidence chain and adds the official runnable HTTP service path; it is not the phase for final external-process HTTP benchmark closure or final archive sign-off. [VERIFIED: .planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md] [VERIFIED: .planning/ROADMAP.md]
- Phase 11 depends on Phase 10 and must close the milestone blockers recorded in `.planning/v1.0-MILESTONE-AUDIT.md`. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- Phase 11 is currently mapped to `API-02`, `API-04`, `OBS-01`, `DOC-01`, and `TEST-04` in the roadmap, but the audit also exposes a stale traceability contradiction around `TEST-03` and several already-verified Phase 7 requirements. [VERIFIED: .planning/ROADMAP.md] [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- The serve entrypoint should stay small and product-facing: bind/config wiring, startup, shutdown, and smoke verification are in scope; production deployment automation is not. [VERIFIED: .planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md]

### Project Constraints
- The event store remains the source of truth; adapters must submit through `CommandGateway` and must not mutate aggregate, projection, or outbox state directly. [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: docs/hot-path-rules.md] [VERIFIED: crates/adapter-http/src/commerce.rs]
- Use Rust-first composition in the `app` crate; avoid adding a second app framework or moving business logic into the HTTP adapter. [VERIFIED: AGENTS.md] [VERIFIED: crates/app/src/lib.rs] [VERIFIED: crates/adapter-http/src/lib.rs]
- Node/Python package preferences (`pnpm`, `uv`) do not materially affect this phase unless docs/scripts are added later. [VERIFIED: AGENTS.md]
- No project-local `.claude` or `.agents` skill bundle exists in this repo. [VERIFIED: search/files checks during orchestration]

### Deferred Ideas
- Phase 12 owns external-process HTTP E2E, stress, and benchmark closure against the real service process. [VERIFIED: .planning/ROADMAP.md]
- Phase 14 owns final milestone debt closure, reopened earlier artifacts if needed, and archive sign-off. [VERIFIED: .planning/ROADMAP.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| API-02 | Adapter code does not mutate aggregate state, projector state, or outbox state directly. [VERIFIED: .planning/REQUIREMENTS.md] | Existing `adapter-http` handlers already own only runtime gateways via `HttpState` and build `CommandEnvelope` + await replies; the serve path should compose these handlers, not bypass them. [VERIFIED: crates/adapter-http/src/commerce.rs] |
| API-04 | Documentation explains how WebSocket or gRPC gateways should connect without sharing hot business state. [VERIFIED: .planning/REQUIREMENTS.md] | Phase 7 docs already satisfy the conceptual guidance, but archive source-of-truth files still leave it pending; evidence reconciliation must update the authoritative docs. [VERIFIED: docs/template-guide.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |
| OBS-01 | Runtime emits structured traces with command ID, correlation ID, causation ID, tenant ID, stream ID, shard ID, and global position when available. [VERIFIED: .planning/REQUIREMENTS.md] | Adapter, runtime, and store instrumentation already exist; Phase 11 should preserve those spans through the new serve path and reconcile stale requirement tracking. [VERIFIED: crates/adapter-http/src/commerce.rs] [VERIFIED: crates/app/src/observability.rs] |
| DOC-01 | Documentation states hot-path rules, forbidden patterns, service-boundary guidance, and how to create a new domain service from the template. [VERIFIED: .planning/REQUIREMENTS.md] | Existing docs already cover these topics; Phase 11 needs to update the source-of-truth documents and add runnable-HTTP usage docs, not rewrite the architecture guidance from scratch. [VERIFIED: docs/hot-path-rules.md] [VERIFIED: docs/template-guide.md] |
| TEST-04 | A single-service integrated stress test runs the production-shaped composition in one service process and reports throughput/latency/depth/lag metrics under realistic traffic. [VERIFIED: .planning/REQUIREMENTS.md] | Phase 7 already created the in-process integrated stress harness, but the audit correctly notes that the repo still lacks an official runnable HTTP service entrypoint for later real-process smoke/E2E usage. [VERIFIED: crates/app/src/stress.rs] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md] |

</phase_requirements>

## Summary

Phase 11 is split cleanly into two workstreams:

1. **Evidence recovery** — repair the archive evidence chain by creating the missing Phase 10 verification artifact and reconciling authoritative planning documents (`REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md`, and `v1.0-MILESTONE-AUDIT.md`) with already-verified Phase 7/10 evidence. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
2. **Runnable HTTP service** — add the official `app serve` composition path that boots the real HTTP router, runtime gateways, migrations, observability, and graceful shutdown as a stable executable service surface for later external-process phases. [VERIFIED: .planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md] [VERIFIED: crates/app/src/main.rs] [VERIFIED: crates/adapter-http/src/lib.rs]

The current repository already contains most of the substrate needed for both workstreams:
- Phase 10 has a complete plan, summary, and validation artifact but lacks `10-VERIFICATION.md`. [VERIFIED: .planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-PLAN.md] [VERIFIED: .planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-SUMMARY.md] [VERIFIED: .planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VALIDATION.md]
- Phase 7 already verified API boundary, observability, documentation, and in-process stress requirements, but those outcomes are still stale in `REQUIREMENTS.md` and the milestone audit. [VERIFIED: .planning/phases/07-adapters-observability-stress-and-template-guidance/07-VERIFICATION.md] [VERIFIED: .planning/REQUIREMENTS.md] [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]
- `adapter-http` already exposes production-shaped command routes via gateway-only `HttpState`, while `app` lacks the runtime composition and listener bootstrap needed to serve them. [VERIFIED: crates/adapter-http/src/commerce.rs] [VERIFIED: crates/app/src/main.rs]

**Primary recommendation:** keep Phase 11 as two plans:
- `11-01-PLAN.md` for evidence recovery and traceability reconciliation
- `11-02-PLAN.md` for `app serve`, smoke coverage, and runnable-service docs

That matches the roadmap, limits blast radius, and prevents the archive-evidence work from being blocked on code-level service composition changes.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Phase 10 evidence recovery | `.planning/` artifacts | targeted regression tests | The missing artifact is documentation/evidence, but it must cite real commands and current behavior. |
| Source-of-truth traceability reconciliation | `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md`, milestone audit | prior verification + summary docs | The contradiction is between docs, not missing code evidence. |
| Runnable HTTP composition | `app` crate | `adapter-http` crate | `app` owns composition/bootstrap; `adapter-http` owns request decoding and gateway-only handlers. |
| HTTP request ingress | `adapter-http` crate | `es-runtime` gateway | Adapter should continue to create envelopes and await replies instead of owning runtime/store state. |
| Runtime/service bootstrap | `app::serve`-style module | `main.rs` thin CLI shell | `main.rs` should remain argument dispatch only; long-lived composition belongs in reusable library code. |
| Observability and metrics | `app::observability` | runtime/store spans/metrics | The serve path must initialize existing observability, not fork a new stack. |

## Standard Stack

### Core

| Library / Crate | Version | Purpose | Why Standard |
|-----------------|---------|---------|--------------|
| Rust workspace | `rust-version = "1.85"`, edition 2024 | compilation baseline | This phase should stay on the existing workspace floor. [VERIFIED: Cargo.toml] |
| `app` crate | workspace `0.1.0` | runtime composition, CLI entrypoint, observability | Owns stress harness already; natural home for serve path. [VERIFIED: crates/app/Cargo.toml] [VERIFIED: crates/app/src/main.rs] |
| `adapter-http` crate | workspace `0.1.0` | Axum router + DTO decode + gateway submission | Already owns the HTTP boundary and must stay state-thin. [VERIFIED: crates/adapter-http/src/lib.rs] [VERIFIED: crates/adapter-http/src/commerce.rs] |
| `es-runtime` | workspace `0.1.0` | `CommandGateway`, `CommandEngine`, duplicate replay | Existing command ingress/runtime boundary; serve path should reuse it. [VERIFIED: crates/adapter-http/src/commerce.rs] |
| `es-store-postgres` | workspace `0.1.0` | durable store + migrations | Serve path needs a real DB-backed composition, not mocks. [VERIFIED: crates/app/Cargo.toml] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `axum` | `0.8.9` | router/service serving | Already present in workspace and adapter crate. [VERIFIED: Cargo.toml] |
| `tokio` | `1.52.0` | async runtime, listener, ctrl-c | Existing workspace runtime; serve path will need `net` and `signal` features enabled. [VERIFIED: Cargo.toml] |
| `sqlx` | `0.8.6` | `PgPool` + migrations | Use for runtime DB connect/migrate before booting the service. [VERIFIED: Cargo.toml] |
| `tracing` + OTLP/prometheus stack | workspace dependencies | logs/metrics bootstrap | Already used in the app crate; reuse rather than inventing another observability path. [VERIFIED: crates/app/Cargo.toml] [VERIFIED: crates/app/src/observability.rs] |
| `testcontainers` | `0.25.0` | smoke/integration process boot tests | Use for realistic `app serve` smoke tests if service-process coverage is added. [VERIFIED: Cargo.toml] [VERIFIED: crates/app/Cargo.toml] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Two-plan split | Single monolithic Phase 11 plan | Harder to execute/review; evidence docs and runtime composition would be coupled unnecessarily. |
| Thin `main.rs` + `app::serve` module | Put all bootstrap in `main.rs` | Makes the binary harder to test and reuse; inconsistent with existing composition style. |
| Env-first config for `serve` | Add `clap` or another CLI parser now | Extra dependency/surface area for a narrow phase; manual subcommand parsing is enough here. |
| `adapter-http` health route | External smoke without a simple health endpoint | Later process tests become flakier and startup detection is harder. |

## Architecture Patterns

### System Architecture Diagram

```text
CLI: cargo run -p app -- serve
  |
  v
app::serve bootstrap
  |
  +--> init observability
  +--> connect PgPool + run migrations
  +--> build Postgres-backed CommandEngine<Order|Product|User>
  +--> clone gateways into adapter_http::HttpState
  +--> build axum router (commerce routes + health route)
  +--> bind TcpListener
  +--> serve until ctrl-c / shutdown signal

HTTP request
  |
  v
adapter-http DTO decode
  |
  v
CommandEnvelope::<Aggregate>::new + CommandGateway::try_submit
  |
  v
runtime/store append + reply
  |
  v
HTTP JSON success/error response with durable metadata
```

### Pattern 1: Evidence Repair From Existing Truth Sources

**What:** Create `10-VERIFICATION.md` from existing Phase 10 plan/summary/validation artifacts and current targeted regression commands, then reconcile the planning docs to match that evidence.

**When to use:** Use when the code is already implemented and verified but the archive/source-of-truth chain is incomplete.

**Example source set:**
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-PLAN.md`
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-SUMMARY.md`
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VALIDATION.md`
- `.planning/phases/07-adapters-observability-stress-and-template-guidance/07-VERIFICATION.md`

### Pattern 2: Thin Binary, Reusable Composition Module

**What:** Keep `main.rs` as command dispatch only and put service boot logic into a reusable `serve` module in the `app` library.

**When to use:** Use whenever app composition needs tests, smoke harnesses, or future alternate binaries.

**Example shape:**
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("stress-smoke") => app::stress::run_smoke_cli().await,
        Some("serve") => app::serve::run_from_env().await,
        _ => {
            eprintln!("usage: app serve | app stress-smoke");
            Ok(())
        }
    }
}
```

### Pattern 3: Gateway-Only HTTP Adapter Preservation

**What:** The serve path should compose `adapter_http::router(state)` with gateway-only `HttpState`; it should not bypass the adapter by calling runtime/domain logic directly.

**When to use:** Always, because API-02 is explicitly about preventing adapters from owning aggregate/projector/outbox state.

**Example current evidence:** `adapter-http` handlers build `CommandEnvelope`, call `try_submit`, await the oneshot reply, and serialize durable append metadata back out. [VERIFIED: crates/adapter-http/src/commerce.rs]

### Anti-Patterns to Avoid
- **Papering over stale traceability with sidecar notes instead of updating the authoritative files** — this preserves contradiction. [VERIFIED: .planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md]
- **Implementing `app serve` by bypassing `adapter-http` and calling command engines directly from the binary** — violates the adapter boundary and weakens smoke/E2E realism. [VERIFIED: .planning/REQUIREMENTS.md]
- **Expanding Phase 11 into full external-process benchmark closure** — that belongs to Phase 12. [VERIFIED: .planning/ROADMAP.md]
- **Treating in-process `stress-smoke` as proof that a real HTTP service path exists** — the milestone audit already shows that is insufficient. [VERIFIED: .planning/v1.0-MILESTONE-AUDIT.md]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP business logic in the binary | ad-hoc route handling in `main.rs` | `adapter-http` router + `HttpState` | Keeps API boundary and tests aligned. |
| New runtime/queue stack for serve | custom command submission pipeline | existing `CommandEngine` + `CommandGateway` | Reuses the verified runtime path. |
| Archive evidence rewrite from memory | hand-wavy verification prose | current summaries/validation docs + rerun targeted commands | Evidence chain must remain grounded. |
| New documentation site for serve usage | isolated sidecar note | update existing `docs/template-guide.md` and relevant phase docs | Prevents further source-of-truth drift. |

## Common Pitfalls

### Pitfall 1: TEST-03 / TEST-04 traceability contradiction
**What goes wrong:** Phase 7 verification marks these satisfied, while `REQUIREMENTS.md` and the milestone audit still tie later HTTP work to pending requirement ownership.
**How to avoid:** Make `11-01-PLAN` explicitly resolve the editorial rule: Phase 7 satisfied the original benchmark/in-process stress requirements, while Phases 11-12 add archive/runnable-service/external-process closure. Update every authoritative doc consistently.

### Pitfall 2: `adapter-http` remains dev-only from the app crate
**What goes wrong:** `app serve` cannot compile cleanly because `adapter-http` is only a dev-dependency.
**How to avoid:** Move it to normal dependencies in `crates/app/Cargo.toml` as part of the serve plan.

### Pitfall 3: missing serde readiness in the user aggregate path
**What goes wrong:** a shared runtime codec for the serve path can fail because `UserEvent` / `UserReply` are not serde-derived like the order/product equivalents.
**How to avoid:** include `crates/example-commerce/src/user.rs` in scope for the serve plan and add explicit serialization coverage.

### Pitfall 4: no health route or startup contract
**What goes wrong:** later external-process smoke tests become brittle because there is no stable readiness probe.
**How to avoid:** add a minimal `/healthz` or equivalent route through the official router and document it.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 11 should preserve the roadmap’s two-plan split. | overall planning | Low; the roadmap already declares two plans. |
| A2 | `TEST-03` and `TEST-04` should remain evidenced by Phase 7 while Phase 11/12 close runnable-service and external-process audit debt. | evidence recovery | Medium; if rejected, REQUIREMENTS/ROADMAP/audit wording will need broader remapping. |
| A3 | Minimal env-based serve config is acceptable for this repo. | serve design | Low; can be upgraded later without changing the service boundary. |

## Open Questions (RESOLVED)

1. **Should evidence repair and serve implementation be split?**
   - Resolved: yes. The roadmap already declares two plans and the workstreams have different artifact shapes.
2. **Should Phase 11 claim full external HTTP E2E closure?**
   - Resolved: no. Phase 11 creates the runnable service path and smoke proof; Phase 12 uses that path for external-process workloads.
3. **Should `app serve` introduce a heavy CLI/config layer now?**
   - Resolved: no. Thin env-based config + simple subcommand parsing is enough for this phase.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | compile/tests | ✓ | workspace baseline | none needed |
| Cargo | builds/tests | ✓ | workspace baseline | none needed |
| PostgreSQL via SQLx/Testcontainers | realistic serve smoke | available in workspace dependencies | `sqlx 0.8.6`, `testcontainers 0.25.0` | targeted compile-only or no-run checks if local Docker is unavailable |
| Axum/Tokio | HTTP service boot | ✓ | workspace dependencies | enable missing Tokio features instead of adding a new runtime |

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness plus `tokio::test` integration/smoke tests |
| Config file | workspace `Cargo.toml` and crate-local `Cargo.toml` files |
| Quick run command | `cargo test -p app --no-run && cargo test -p adapter-http --no-run` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| API-02 | Official runnable HTTP service still routes only through `HttpState` gateways | adapter/app integration | `cargo test -p adapter-http -- --nocapture` and serve smoke test | Partial |
| API-04 | Source-of-truth docs map gateway/service-boundary guidance correctly | doc/evidence verification | `rg -n "API-04|CommandGateway|WebSocket and gRPC gateways" .planning docs` | Partial |
| OBS-01 | Structured traces remain initialized and exercised through serve composition | app smoke / composition | `cargo test -p app -- --nocapture` | Partial |
| DOC-01 | Template docs describe `app serve`, health/smoke usage, and existing hot-path rules remain authoritative | doc verification | `rg -n "app serve|healthz|CommandGateway|event store is the source of truth" docs` | Partial |
| TEST-04 | Official service process can start and be reached through the documented HTTP path | external-process smoke | `cargo test -p app serve_smoke -- --nocapture` or equivalent targeted test | Missing |

### Sampling Rate
- **After each evidence/doc task:** run the relevant `rg` and file-existence checks.
- **After each serve/code task:** run targeted crate tests (`adapter-http`, `app`) before moving on.
- **Before phase sign-off:** run `cargo test --workspace`, or explicitly document environment blockers and the passing targeted commands.

### Wave 0 Gaps
- [ ] Create `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VERIFICATION.md`.
- [ ] Add explicit Phase 11 validation coverage for evidence/doc reconciliation and runnable service smoke.
- [ ] Add official `app serve` smoke coverage and its documented invocation path.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V4 Access Control | yes | Preserve tenant-scoped command metadata and do not bypass gateway/runtime boundaries. |
| V5 Input Validation | yes | Continue adapter DTO validation + typed domain constructors through the official serve path. |
| V7 Error Handling / Logging | yes | Map runtime/domain/store errors to typed HTTP responses and preserve structured traces. |
| V14 Configuration | yes | Keep serve config minimal, explicit, and env-driven; do not hide dangerous defaults. |

### Known Threat Patterns for this Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Archive source-of-truth contradicts verified evidence | Repudiation / Tampering | Reconcile authoritative docs against current summaries, verification, and rerun commands. |
| Binary bypasses HTTP adapter and mutates runtime directly | Tampering | Compose only through `adapter-http` router and gateways. |
| Serve path starts without observability or health signaling | Denial of Service / Repudiation | Initialize observability and expose a minimal health endpoint for smoke checks. |
| Shared runtime codec misses one aggregate’s serde support | Reliability / DoS | Include explicit user aggregate serialization work and targeted tests. |

## Sources

### Primary (HIGH confidence)
- `.planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md`
- `.planning/ROADMAP.md`
- `.planning/STATE.md`
- `.planning/REQUIREMENTS.md`
- `.planning/v1.0-MILESTONE-AUDIT.md`
- `.planning/phases/07-adapters-observability-stress-and-template-guidance/07-VERIFICATION.md`
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-PLAN.md`
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-SUMMARY.md`
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VALIDATION.md`
- `crates/app/src/main.rs`
- `crates/app/Cargo.toml`
- `crates/app/src/lib.rs`
- `crates/app/src/stress.rs`
- `crates/app/src/observability.rs`
- `crates/adapter-http/src/lib.rs`
- `crates/adapter-http/src/commerce.rs`
- `crates/example-commerce/src/user.rs`
- `docs/template-guide.md`
- `docs/hot-path-rules.md`
- `docs/stress-results.md`

### Secondary (MEDIUM confidence)
- Prior phase plan/verification conventions inferred from Phase 07, 09, and 10 artifacts.

## Metadata

**Confidence breakdown:**
- Evidence repair scope: HIGH — blockers and contradictions are explicitly documented.
- Runnable service architecture: HIGH — current code clearly shows the missing entrypoint and existing adapter/runtime composition boundaries.
- Main risk area: MEDIUM/HIGH — the `TEST-03` / `TEST-04` wording contradiction must be resolved consistently across all authoritative docs.

**Research date:** 2026-04-21
**Valid until:** 2026-05-21, or until the Phase 11 roadmap, app composition, or milestone audit changes materially.

---
phase: 11-evidence-recovery-and-runnable-http-service
plan: 02
subsystem: app+adapter-http+docs
completed: 2026-04-21
requirements-completed: [API-02, OBS-01, TEST-04, DOC-01]
artifacts-created:
  - crates/app/src/serve.rs
  - crates/app/tests/serve_smoke.rs
artifacts-updated:
  - Cargo.toml
  - crates/app/Cargo.toml
  - crates/app/src/lib.rs
  - crates/app/src/main.rs
  - crates/adapter-http/src/lib.rs
  - crates/adapter-http/tests/commerce_api.rs
  - crates/example-commerce/src/user.rs
  - docs/template-guide.md
  - docs/stress-results.md
verification:
  - cargo test -p adapter-http -- --nocapture
  - cargo test -p app --no-run
  - cargo test -p app serve_smoke -- --nocapture
---

# Phase 11 Plan 02 Summary

## Outcome

Added the official runnable HTTP service path.

## What changed

- Added `app::serve` with env-driven configuration, PostgreSQL connect+migrate bootstrap, Postgres-backed command engines, and `adapter_http::router(HttpState)` composition.
- Extended the binary shell so `cargo run -p app -- serve` starts the real HTTP router while `stress-smoke` remains available.
- Added `/healthz` to the official router and adapter coverage for the readiness endpoint.
- Added an external-process smoke test that boots `app serve`, waits for readiness, and sends a real HTTP order command.
- Updated docs so `app serve` is the canonical service process and `app stress-smoke` is clearly documented as an in-process harness.

## Verification

- `cargo test -p adapter-http -- --nocapture` ✅
- `cargo test -p app --no-run` ✅
- `cargo test -p app serve_smoke -- --nocapture` ✅

## Handoff

Phase 11 runnable-service work is complete. Phase 12 should now reuse `app serve` as the canonical process for external-process HTTP E2E, stress, and benchmark closure.

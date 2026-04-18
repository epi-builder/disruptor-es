---
phase: 07-adapters-observability-stress-and-template-guidance
plan: 01
subsystem: api
tags: [rust, axum, tower, command-gateway, event-sourcing]

requires:
  - phase: 03-local-command-runtime-and-disruptor-execution
    provides: bounded CommandGateway ingress and durable CommandOutcome replies
  - phase: 04-commerce-fixture-domain
    provides: typed commerce aggregates, commands, IDs, and replies
provides:
  - Thin Axum commerce command adapter over bounded runtime gateways
  - Typed JSON success and error contracts carrying durable append metadata
  - Adapter dependency and source-boundary regression tests for API-02
affects: [adapter-http, es-runtime, phase-07-observability, phase-07-stress]

tech-stack:
  added: [axum, tower, tower-http, metrics, tracing-subscriber, hdrhistogram, criterion, sysinfo, metrics-exporter-prometheus, tracing-opentelemetry, opentelemetry, opentelemetry_sdk, opentelemetry-otlp]
  patterns: [thin HTTP command adapter, CommandGateway DTO mapping, JSON ApiError IntoResponse, file-path boundary tests]

key-files:
  created:
    - crates/adapter-http/src/commerce.rs
    - crates/adapter-http/src/error.rs
    - crates/adapter-http/tests/commerce_api.rs
    - crates/adapter-http/tests/dependency_boundaries.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/adapter-http/Cargo.toml
    - crates/adapter-http/src/lib.rs
    - crates/es-runtime/src/lib.rs

key-decisions:
  - "Keep adapter-http dependency-safe: no direct es-store-postgres, es-projection, or es-outbox dependencies."
  - "Expose es_runtime::CommittedAppend and es_runtime::Aggregate as runtime boundary re-exports for tests and adapter-safe outcome construction."
  - "Represent HTTP replies as adapter DTOs instead of requiring commerce domain reply types to implement Serialize."

patterns-established:
  - "HTTP handlers construct CommandMetadata, CommandEnvelope, and one-shot replies, then call CommandGateway::try_submit without owning aggregate state."
  - "ApiError maps RuntimeError variants to stable JSON error codes and HTTP statuses."
  - "Adapter boundary tests scan manifests and source files for forbidden direct storage/projection/outbox/hot-state markers."

requirements-completed: [API-01, API-02, API-03]

duration: 12min 12s
completed: 2026-04-18
---

# Phase 07 Plan 01: HTTP Adapter Command Boundary Summary

**Axum commerce command routes now submit through bounded runtime gateways and return durable-position JSON responses without direct storage, projection, or outbox dependencies.**

## Performance

- **Duration:** 12min 12s
- **Started:** 2026-04-18T14:20:15Z
- **Completed:** 2026-04-18T14:32:27Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Added the Phase 7 workspace dependency catalog and wired `adapter-http` only to adapter-safe dependencies.
- Implemented commerce command routes for orders, products, and users using `CommandEnvelope::<Order|Product|User>::new`, one-shot replies, and `CommandGateway::try_submit`.
- Added typed JSON success responses with correlation ID, stream ID, stream revision, first/last revisions, global positions, event IDs, and adapter reply DTOs.
- Added JSON-only `ApiError` mapping for overload, unavailable, conflict, domain, invalid request, and internal errors.
- Added contract and boundary tests proving command submission, response shape, error mapping, and forbidden dependency/source patterns.

## Task Commits

Each task was committed atomically:

1. **Task 07-01-01: Add Phase 7 workspace and adapter dependencies** - `f849401` (chore)
2. **Task 07-01-02 RED: Add failing HTTP commerce contract tests** - `c668b03` (test)
3. **Task 07-01-02 GREEN: Implement commerce command routes and typed response mapping** - `4b6b569` (feat)
4. **Task 07-01-03: Prove adapter boundary rules** - `ae97518` (test)

## Files Created/Modified

- `Cargo.toml` - Added Phase 7 workspace dependencies.
- `Cargo.lock` - Locked dependency graph for adapter and observability dependencies.
- `crates/adapter-http/Cargo.toml` - Added only adapter-safe direct dependencies.
- `crates/adapter-http/src/lib.rs` - Exported router, state, commerce routes, and API error types.
- `crates/adapter-http/src/commerce.rs` - Added Axum routes, request DTOs, metadata construction, command submission, and success DTOs.
- `crates/adapter-http/src/error.rs` - Added typed API error mapping and JSON `IntoResponse` implementation.
- `crates/adapter-http/tests/commerce_api.rs` - Added HTTP command and response contract tests.
- `crates/adapter-http/tests/dependency_boundaries.rs` - Added dependency and source-boundary checks.
- `crates/es-runtime/src/lib.rs` - Re-exported `Aggregate` and `CommittedAppend` at the runtime boundary.

## Decisions Made

- Adapter HTTP state contains only `CommandGateway<Order>`, `CommandGateway<Product>`, and `CommandGateway<User>`.
- Adapter responses use DTO reply enums so domain reply types remain free of HTTP serialization concerns.
- Runtime re-exports expose outcome construction types for tests without adding direct storage dependencies to `adapter-http`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Re-exported runtime outcome support types**
- **Found during:** Task 07-01-02 (commerce API TDD GREEN)
- **Issue:** Adapter tests needed to complete a `CommandReply` with durable append metadata, but `CommittedAppend` was only visible as a public field type through `CommandOutcome`; adding `es-store-postgres` to `adapter-http` would violate API-02.
- **Fix:** Re-exported `CommittedAppend` and `Aggregate` from `es-runtime`, keeping adapter tests on the runtime boundary.
- **Files modified:** `crates/es-runtime/src/lib.rs`
- **Verification:** `cargo test -p adapter-http commerce_api -- --nocapture`; `cargo test -p adapter-http response_contract -- --nocapture`; `cargo test -p adapter-http dependency_boundaries -- --nocapture`
- **Committed in:** `4b6b569`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The deviation preserved the planned adapter dependency boundary and did not add direct storage/projection/outbox access to `adapter-http`.

## Issues Encountered

- Cargo package/build locks were held briefly by parallel Phase 07 work; commands waited and completed normally.
- The initial overload test shape could hang by awaiting an accepted command without completing its reply; it was corrected before the GREEN commit by filling the bounded gateway directly and then asserting the HTTP 429 response.

## TDD Gate Compliance

- RED commit present: `c668b03`
- GREEN commit present after RED: `4b6b569`
- REFACTOR commit: not needed

## Known Stubs

None.

## Threat Flags

None. The new HTTP DTO and adapter-to-gateway trust boundaries are covered by the plan threat model.

## Verification

- `cargo check -p adapter-http` - passed
- `cargo test -p adapter-http commerce_api -- --nocapture` - passed
- `cargo test -p adapter-http response_contract -- --nocapture` - passed
- `cargo test -p adapter-http dependency_boundaries -- --nocapture` - passed
- `cargo test -p adapter-http -- --nocapture` - passed
- `cargo test --workspace --no-run` - passed

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The HTTP command adapter is ready for Phase 07 observability and stress work. Future adapters can reuse the same thin ingress pattern: decode DTOs, construct metadata/envelopes, submit through bounded gateways, and keep committed events as the durable source of truth.

## Self-Check: PASSED

- Verified all key created/modified files exist.
- Verified task commits exist: `f849401`, `c668b03`, `4b6b569`, `ae97518`.
- Verified plan-level tests passed.

---
*Phase: 07-adapters-observability-stress-and-template-guidance*
*Completed: 2026-04-18*

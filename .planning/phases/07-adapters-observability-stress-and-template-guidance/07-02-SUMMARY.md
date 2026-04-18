---
phase: 07-adapters-observability-stress-and-template-guidance
plan: 02
subsystem: observability
tags: [rust, tracing, metrics, prometheus, opentelemetry, cqrs, outbox]

requires:
  - phase: 07-adapters-observability-stress-and-template-guidance
    provides: "Plan 07-01 HTTP command adapter and runtime gateway boundaries"
provides:
  - App-level observability bootstrap with bounded Phase 7 metric catalog
  - Trace spans carrying command, correlation, causation, tenant, stream, shard, and global-position fields where available
  - Runtime, append, projection, and outbox metrics using bounded labels
affects: [phase-07-stress, app, adapter-http, es-runtime, es-store-postgres, es-outbox]

tech-stack:
  added: [metrics, metrics-exporter-prometheus, tracing-subscriber, tracing-opentelemetry, opentelemetry, opentelemetry_sdk, opentelemetry-otlp]
  patterns: [app-owned telemetry bootstrap, metrics facade in lower crates, trace-fields-for-identity, bounded metric labels]

key-files:
  created:
    - crates/app/src/observability.rs
  modified:
    - Cargo.lock
    - crates/app/Cargo.toml
    - crates/app/src/lib.rs
    - crates/adapter-http/Cargo.toml
    - crates/adapter-http/src/commerce.rs
    - crates/es-runtime/Cargo.toml
    - crates/es-runtime/src/gateway.rs
    - crates/es-runtime/src/engine.rs
    - crates/es-runtime/src/shard.rs
    - crates/es-store-postgres/Cargo.toml
    - crates/es-store-postgres/src/event_store.rs
    - crates/es-store-postgres/src/projection.rs
    - crates/es-outbox/Cargo.toml
    - crates/es-outbox/src/dispatcher.rs

key-decisions:
  - "Keep telemetry exporter setup in the app crate while lower crates emit through tracing and metrics facades."
  - "Use high-cardinality command identity only as trace fields, never as metric labels."
  - "Use bounded metric labels: aggregate, outcome, reason, shard, projector, and topic."

patterns-established:
  - "Command spans are created at adapter, gateway, engine, shard, and append boundaries with durable global position recorded after append replies."
  - "Runtime/storage/outbox metrics use the metrics facade and bounded labels so exporters can be configured at composition time."

requirements-completed: [OBS-01, OBS-02]

duration: 12min 50s
completed: 2026-04-18
---

# Phase 07 Plan 02: Observability Boundary Instrumentation Summary

**App telemetry bootstrap and bounded runtime/storage/outbox metrics now expose command causality, durable positions, queue depth, latency, conflicts, dedupe, projection lag, and outbox lag.**

## Performance

- **Duration:** 12min 50s
- **Started:** 2026-04-18T14:37:55Z
- **Completed:** 2026-04-18T14:50:45Z
- **Tasks:** 2
- **Files modified:** 15

## Accomplishments

- Added `ObservabilityConfig`, `init_observability`, `PHASE7_METRIC_NAMES`, and `FORBIDDEN_METRIC_LABELS` in `crates/app/src/observability.rs`.
- Added RED/GREEN TDD commits proving the Phase 7 metric catalog and forbidden high-cardinality label list.
- Instrumented HTTP submission, gateway admission, engine routing, shard processing, append, projection catch-up, and outbox dispatch boundaries with structured spans and metrics.
- Preserved bounded metric labels while recording command IDs, tenant IDs, stream IDs, shard IDs, and global positions as trace fields only.

## Task Commits

Each task was committed atomically:

1. **Task 07-02-01 RED: Add failing observability catalog tests** - `84669d4` (test)
2. **Task 07-02-01 GREEN: Implement observability bootstrap and metric catalog** - `ef2dc3d` (feat)
3. **Task 07-02-02: Instrument runtime, storage, projection, and outbox boundaries** - `6dba485` (feat)

**Plan metadata:** this docs commit

## Files Created/Modified

- `Cargo.lock` - Locked app observability dependencies added during Task 07-02-01.
- `crates/app/Cargo.toml` - Added app-level telemetry/exporter dependencies.
- `crates/app/src/lib.rs` - Exported the observability module.
- `crates/app/src/observability.rs` - Added observability config, initialization, metric catalog, forbidden-label catalog, and tests.
- `crates/adapter-http/Cargo.toml` - Added metrics facade for adapter command latency emission.
- `crates/adapter-http/src/commerce.rs` - Added `http.command` span fields and command latency recording after replies.
- `crates/es-runtime/Cargo.toml` - Added metrics facade dependency.
- `crates/es-runtime/src/gateway.rs` - Added gateway submission span, ingress depth, command accepted/rejected counters.
- `crates/es-runtime/src/engine.rs` - Added `command_engine.process_one` routing span.
- `crates/es-runtime/src/shard.rs` - Added shard handoff span, shard queue depth, ring wait, decision latency, and command latency metrics.
- `crates/es-store-postgres/Cargo.toml` - Added metrics and tracing dependencies.
- `crates/es-store-postgres/src/event_store.rs` - Added append span, append latency, OCC conflict, and dedupe hit metrics.
- `crates/es-store-postgres/src/projection.rs` - Added projection catch-up span and projection lag/catch-up metrics.
- `crates/es-outbox/Cargo.toml` - Added metrics and tracing dependencies.
- `crates/es-outbox/src/dispatcher.rs` - Added outbox dispatch span, outbox lag, and dispatch outcome counters.

## Decisions Made

- App composition owns exporter/subscriber initialization; reusable crates only emit facades.
- Identity fields are emitted as trace fields because they are high-cardinality and operationally useful for causality.
- Metric labels remain bounded to avoid cardinality explosions under load.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope changes.

## TDD Gate Compliance

- RED commit present: `84669d4`
- GREEN commit present after RED: `ef2dc3d`
- REFACTOR commit: not needed

## Verification

- `cargo test -p app observability -- --nocapture` - PASS
- `rg 'ObservabilityConfig|init_observability|PHASE7_METRIC_NAMES|FORBIDDEN_METRIC_LABELS|es_ingress_depth|es_command_latency_seconds|tenant_id' crates/app/src/observability.rs` - PASS
- `rg 'pub mod observability' crates/app/src/lib.rs` - PASS
- `rg 'command_id|correlation_id|causation_id|tenant_id|stream_id|shard_id|global_position' crates/adapter-http/src/commerce.rs crates/es-runtime/src/gateway.rs crates/es-runtime/src/engine.rs crates/es-runtime/src/shard.rs crates/es-store-postgres/src/event_store.rs` - PASS
- `rg 'http.command|command_gateway.try_submit|command_engine.process_one|info_span!|debug_span!|record\("global_position"' crates/adapter-http/src/commerce.rs crates/es-runtime/src crates/es-store-postgres/src/event_store.rs` - PASS
- `rg 'es_ingress_depth|es_shard_queue_depth|es_ring_wait_seconds|es_decision_latency_seconds|es_append_latency_seconds|es_occ_conflicts_total|es_dedupe_hits_total|es_projection_lag|es_outbox_lag|es_command_latency_seconds' crates/es-runtime/src crates/es-store-postgres/src crates/es-outbox/src` - PASS
- `! rg '=> .*tenant_id|=> .*command_id|=> .*correlation_id|=> .*causation_id|=> .*stream_id|=> .*event_id|=> .*idempotency_key' crates/es-runtime/src crates/es-store-postgres/src crates/es-outbox/src` - PASS
- `! rg '=> .*tenant_id|=> .*command_id|=> .*correlation_id|=> .*causation_id|=> .*stream_id|=> .*global_position' crates/adapter-http/src/commerce.rs crates/es-runtime/src crates/es-store-postgres/src crates/es-outbox/src` - PASS
- `cargo fmt --check` - PASS
- `cargo test --workspace --no-run` - PASS

## Known Stubs

None. Stub scan found the pre-existing `ShardHandoffToken::placeholder` sentinel in `crates/es-runtime/src/shard.rs`; it is an intentional disruptor path placeholder token, not a user-visible or unwired data stub.

## Threat Flags

None - the telemetry backend and metric-cardinality trust boundaries were covered by the plan threat model.

## Issues Encountered

- Cargo briefly waited on package and build directory locks while parallel Phase 07 work was compiling. The waits completed normally.
- Parallel benchmark-plan files were present in the working tree while verification ran. They were not staged or modified by this plan.

## User Setup Required

None - no external service configuration required. OTLP and Prometheus endpoints are optional runtime configuration through `ObservabilityConfig`.

## Next Phase Readiness

Phase 07 stress and benchmark plans can now scrape or collect bounded metrics and correlate command traces across adapter, gateway, shard, append, projection, and outbox boundaries without exposing high-cardinality identifiers as metric labels.

## Self-Check: PASSED

- Verified key files exist: `crates/app/src/observability.rs`, app manifests/module export, adapter/runtime/storage/outbox instrumentation files.
- Verified task commits exist: `84669d4`, `ef2dc3d`, `6dba485`.
- Verified plan-level tests and workspace no-run compilation passed.

---
*Phase: 07-adapters-observability-stress-and-template-guidance*
*Completed: 2026-04-18*

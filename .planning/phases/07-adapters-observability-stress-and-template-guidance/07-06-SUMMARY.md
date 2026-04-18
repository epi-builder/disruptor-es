---
phase: 07-adapters-observability-stress-and-template-guidance
plan: 06
subsystem: documentation
tags: [rust, event-sourcing, command-gateway, outbox, stress, documentation]

requires:
  - phase: 07-adapters-observability-stress-and-template-guidance
    provides: "Phase 07 HTTP adapter, observability, integration tests, benchmark harnesses, and stress runner from Plans 07-01 through 07-05"
provides:
  - Hot-path source-of-truth, single-owner, gateway, outbox, and forbidden-pattern rules
  - Template guide for adding domains, HTTP/WebSocket/gRPC gateways, queries, outbox, and process managers
  - Stress-results guide separating ring-only, layer benchmarks, and single-service integrated stress reports
affects: [API-04, DOC-01, phase-07-verification, future-domain-services]

tech-stack:
  added: []
  patterns: [grep-verifiable architecture docs, thin gateway guidance, benchmark interpretation guidance]

key-files:
  created:
    - docs/hot-path-rules.md
    - docs/template-guide.md
    - docs/stress-results.md
  modified: []

key-decisions:
  - "Document all adapters as thin CommandGateway clients plus query APIs, with no shared hot aggregate state."
  - "State that ring-only microbenchmarks measure local disruptor handoff cost and must not be compared to integrated service throughput."
  - "Keep command success anchored to durable append replies; projection and outbox lag are interpreted after command success."

patterns-established:
  - "Architecture guidance uses exact grep-verifiable source-of-truth, outbox, gateway, and forbidden-pattern wording."
  - "Template extension docs describe HTTP, WebSocket, and gRPC gateways with the same bounded CommandGateway submission pattern."
  - "Stress documentation requires named report fields and scenario labels before comparing benchmark output."

requirements-completed: [API-04, DOC-01]

duration: 2min 45s
completed: 2026-04-18
---

# Phase 07 Plan 06: Template Guidance Summary

**Hot-path, gateway-extension, and stress-interpretation documentation now makes the template rules grep-verifiable for future domain and adapter authors.**

## Performance

- **Duration:** 2min 45s
- **Started:** 2026-04-18T15:10:31Z
- **Completed:** 2026-04-18T15:13:16Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `docs/hot-path-rules.md` with source-of-truth, single-owner, gateway, outbox, forbidden-pattern, and required-check guidance.
- Added `docs/template-guide.md` with new-domain, HTTP, WebSocket, gRPC, projection query, outbox, process-manager, and verification command guidance.
- Added `docs/stress-results.md` with ring-only, layer benchmark, integrated stress, report-field, lag, and comparison rules.

## Task Commits

Each task was committed atomically:

1. **Task 07-06-01: Write hot-path and service-boundary rules** - `734f131` (docs)
2. **Task 07-06-02: Write template and gateway extension guide** - `f43e08d` (docs)
3. **Task 07-06-03: Write stress interpretation guide** - `9af30fb` (docs)

**Plan metadata:** this docs commit

## Files Created/Modified

- `docs/hot-path-rules.md` - Documents event-store source of truth, non-durable disruptor sequences, single-owner hot state, gateway boundaries, outbox publication, forbidden patterns, and required checks.
- `docs/template-guide.md` - Documents how to create a new aggregate/domain and how HTTP, WebSocket, and gRPC gateways should connect through `CommandGateway` plus read-model query APIs.
- `docs/stress-results.md` - Documents how to interpret ring-only microbenchmarks, layer benchmarks, single-service integrated stress reports, required report fields, projection lag, and outbox lag.
- `.planning/phases/07-adapters-observability-stress-and-template-guidance/07-06-SUMMARY.md` - Execution summary for this plan.

## Decisions Made

- Kept all future gateway guidance transport-neutral: adapters decode DTOs, construct metadata/envelopes, submit through bounded `CommandGateway`, and await durable replies.
- Treated projection and outbox lag as post-command-success signals in docs, preserving append commit as the command success point.
- Required stress report field names to match the Phase 07 stress runner output so docs and JSON reports stay aligned.

## Deviations from Plan

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope changes.

## Issues Encountered

None.

## Known Stubs

None. Stub scan found no TODO/FIXME/placeholder or hardcoded empty data markers in the created docs.

## Threat Flags

None - this plan only added documentation for the documentation-to-future-implementers trust boundary covered by T-07-17 through T-07-19.

## Verification

- `rg '^# Hot Path Rules|^## Source Of Truth|^## Single Owner Execution|^## Gateway Boundaries|^## Outbox Publication|^## Forbidden Patterns|^## Required Checks' docs/hot-path-rules.md` - PASS
- `rg "The event store is the source of truth|Disruptor sequences are never durable positions|Adapters must submit through CommandGateway|External publication must flow through durable outbox rows|Arc<Mutex<HashMap|direct broker publish|using ring sequence as global position|projector catch-up as command success|dynamic SQL" docs/hot-path-rules.md` - PASS
- `rg '^# Template Guide|^## Create A New Domain|^## Add A Command Gateway|^## HTTP Gateway|^## WebSocket Gateway|^## gRPC Gateway|^## Projection Queries|^## Outbox And Process Managers|^## Verification Commands' docs/template-guide.md` - PASS
- `rg "WebSocket and gRPC gateways should be thin ingress clients of CommandGateway|Aggregate|StreamId|PartitionKey|ExpectedRevision|cargo test --workspace --no-run|cargo run -p app -- stress-smoke" docs/template-guide.md` - PASS
- `rg '^# Stress Results|^## Ring-Only Benchmarks|^## Layer Benchmarks|^## Single-Service Integrated Stress|^## Required Report Fields|^## Reading Projection And Outbox Lag|^## Do Not Compare' docs/stress-results.md` - PASS
- `rg "Ring-only microbenchmarks measure local disruptor handoff cost, not service throughput|Single-service integrated stress includes adapter DTO work|throughput_per_second|p95_micros|projection_lag|outbox_lag|cpu_utilization_percent|core_count" docs/stress-results.md` - PASS
- `rg "event store is the source of truth|CommandGateway|single-owner|outbox|ring-only" docs` - PASS
- `rg -n "not available|coming soon|placeholder|TODO|FIXME|=\[\]|=\{\}|=null|=\"\"" docs/hot-path-rules.md docs/template-guide.md docs/stress-results.md || true` - PASS, no matches

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 07 now has docs that future implementers can use to add new domains, HTTP/WebSocket/gRPC gateways, projection queries, outbox workflows, and stress runs without violating event-store source-of-truth, single-owner execution, or outbox publication boundaries.

## Self-Check: PASSED

- Verified key files exist: `docs/hot-path-rules.md`, `docs/template-guide.md`, `docs/stress-results.md`, and this summary.
- Verified task commits exist in git history: `734f131`, `f43e08d`, `9af30fb`.
- Verified plan-level grep checks passed.
- Verified `.planning/STATE.md` and `.planning/ROADMAP.md` were not modified by this executor.

---
*Phase: 07-adapters-observability-stress-and-template-guidance*
*Completed: 2026-04-18*

# Phase 12: External-Process HTTP E2E, Stress, and Benchmark Closure - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>

## Phase Boundary

Phase 12 turns the newly runnable HTTP service into the canonical end-to-end and stress-measurement path. The emphasis is not merely HTTP semantics in-process, but the real serving process overhead and request lifecycle seen by external clients.

This phase does not replace lower-level microbenchmarks or single-process component tests; it establishes the separate external-process baseline they should be compared against.

</domain>

<decisions>

## Implementation Decisions

- **D-01:** Canonical full-E2E and stress workloads must launch the real service process and issue HTTP requests from outside that process.
- **D-02:** In-process `CommandEnvelope` shortcuts are not representative enough to remain the archive-facing “full E2E” path.
- **D-03:** If legacy in-process scenarios remain useful for debugging, they should be renamed so they are not confused with external-process E2E coverage.
- **D-04:** Stress and benchmark reporting must clearly distinguish ring-only, single-process integrated, and external-process HTTP measurements.
- **D-05:** This phase exists because the user explicitly wants execution-workload performance measured through the real serving path, not an ambiguous internal shortcut.

</decisions>

<canonical_refs>

## Canonical References

- `.planning/ROADMAP.md` - Updated Phase 12 scope and acceptance criteria.
- `.planning/v1.0-MILESTONE-AUDIT.md` - Audit evidence describing the `FullE2eInProcess` gap.
- `.planning/phases/11-evidence-recovery-and-runnable-http-service/11-CONTEXT.md` - Runnable-service assumptions Phase 12 depends on.
- `crates/app/src/stress.rs` - Existing stress scenarios that currently bypass HTTP.
- `crates/app/src/main.rs` - The runnable service process introduced in Phase 11.

</canonical_refs>

<code_context>

## Existing Code Insights

- `FullE2eInProcess` currently constructs `CommandEnvelope` directly and submits to the gateway, bypassing HTTP decode, router wiring, and error mapping.
- The repository already separates several benchmark layers; Phase 12 needs to preserve that separation while adding an external-process HTTP lane.
- The stress harness should produce data that later archive docs can point to as the executable-service baseline.

</code_context>

<specifics>

## Specific Ideas

- Prefer a test/harness shape that can start the app on an ephemeral port, wait for readiness, run scenarios, then cleanly shut it down.
- Make request fixtures and reported metrics stable enough to compare external-process runs over time.
- Remove naming ambiguity aggressively so future readers know which workloads include the real HTTP path.

</specifics>

<deferred>

## Deferred Ideas

- Final milestone validation closure and archive sign-off belong to Phase 14.

</deferred>

---

*Phase: 12-external-process-http-e2e-stress-and-benchmark-closure*
*Context gathered: 2026-04-21*

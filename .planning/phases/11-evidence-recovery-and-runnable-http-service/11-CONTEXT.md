# Phase 11: Evidence Recovery and Runnable HTTP Service - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>

## Phase Boundary

Phase 11 converts the remaining archive-evidence and runnable-service gaps into concrete closure work. It restores the verification chain expected by the milestone, reconciles stale source-of-truth planning artifacts, and adds the official `app serve` entrypoint so the HTTP adapter can be launched and smoke-tested as a real service path.

This phase does not yet claim final external-process HTTP stress/benchmark closure or final milestone sign-off. It lays down the evidence and runnable-service foundation that those later phases depend on.

</domain>

<decisions>

## Implementation Decisions

### Evidence Chain Recovery

- **D-01:** Phase 10 must gain a formal `10-VERIFICATION.md` artifact so the milestone evidence chain matches the completed pattern used by Phases 01-09.
- **D-02:** `REQUIREMENTS.md`, `ROADMAP.md`, `STATE.md`, and the milestone audit must stop disagreeing about already-verified Phase 7 and Phase 10 outcomes.
- **D-03:** Stale traceability is treated as an archive blocker, not a cosmetic documentation issue.

### Runnable HTTP Service Path

- **D-04:** The repository needs an official `app serve` entrypoint that starts the real HTTP router and is callable for smoke tests and later external-process workloads.
- **D-05:** Route-only HTTP composition is not sufficient for v1 archive because E2E, stress, and benchmark work need a stable executable serving path.
- **D-06:** The serve entrypoint should stay small and product-facing: bind/config wiring, startup, shutdown, and smoke verification are in scope; production deployment automation is not.

### Milestone Policy

- **D-07:** Milestone-critical gaps must be closed in code/docs/evidence, not carried as accepted debt.
- **D-08:** If evidence recovery or runnable serve work exposes earlier phase defects, the earlier phase artifacts should be reopened and repaired instead of papered over.

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

- `.planning/ROADMAP.md` - Updated Phase 11-13 structure and closure policy.
- `.planning/STATE.md` - Current focus and milestone progress after roadmap expansion.
- `.planning/REQUIREMENTS.md` - Stale traceability that must be reconciled with verified evidence.
- `.planning/v1.0-MILESTONE-AUDIT.md` - Source list of evidence-chain, runnable HTTP, and archive blockers.
- `crates/app/src/main.rs` - Current binary entrypoint that still needs `serve` support.
- `crates/adapter-http/src/commerce.rs` - Existing router surface that needs real app composition.

</canonical_refs>

<code_context>

## Existing Code Insights

### Known Gaps

- The app binary currently exposes `stress-smoke` but not a runnable HTTP server path.
- The HTTP routes already exist in the adapter crate, which means the main missing piece is composition and executable startup wiring.
- Phase 7 and Phase 10 evidence exists, but archive source-of-truth documents still disagree about what is complete.

### Integration Points

- The future external-process HTTP harness in Phase 12 will depend on whatever `app serve` interface is created here.
- Requirements and milestone audit updates must cite the real verification artifacts created or repaired in this phase.

</code_context>

<specifics>

## Specific Ideas

- Keep `serve` easy to invoke from tests and scripts so external-process harnesses can treat it as the canonical service process.
- Document exact smoke commands and expected health/probe behavior as soon as the server path exists.
- When reconciling traceability, update the source-of-truth files instead of adding yet another sidecar note that drifts later.

</specifics>

<deferred>

## Deferred Ideas

- External-process HTTP stress orchestration and benchmark baselines belong to Phase 12.
- Final Nyquist closure and archive sign-off belong to Phase 14.

</deferred>

---

*Phase: 11-evidence-recovery-and-runnable-http-service*
*Context gathered: 2026-04-21*

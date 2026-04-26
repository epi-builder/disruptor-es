---
phase: 10-duplicate-safe-process-manager-follow-up-keys
plan: 01
subsystem: app
tags: [rust, event-sourcing, process-manager, idempotency, replay]

requires:
  - phase: 09-tenant-scoped-runtime-aggregate-cache
    provides: tenant-scoped runtime replay/cache ordering
provides:
  - Line-aware reserve/release follow-up idempotency keys
  - Duplicate same-product reserve/release regression coverage
  - Exact-key replay-aware process-manager duplicate retry coverage
affects: [app, process-manager, phase-10, phase-11]

tech-stack:
  added: []
  patterns:
    - Deterministic follow-up identity = manager + source event + action + line index + product id
    - Compensation release reuses the original successful reserve line identity
    - Replay-aware test store keeps one replay record per exact idempotency key

key-files:
  created:
    - .planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-01-SUMMARY.md
  modified:
    - crates/app/src/commerce_process_manager.rs
    - .planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VALIDATION.md
    - .planning/ROADMAP.md
    - .planning/REQUIREMENTS.md
    - .planning/STATE.md

key-decisions:
  - "Reserve/release keys now include zero-based line index so duplicate same-product lines never collide."
  - "Replay-aware test stores index replay records by exact idempotency key instead of keeping only the last replay record."
  - "Phase 10 stays app-local; no runtime, store SQL, schema, or domain payload changes were required."

patterns-established:
  - "Line-aware follow-up key: pm:{manager}:{source_event_id}:{action}:{line_index}:{product_id}"
  - "Duplicate retry replay: same process event reuses runtime/store replay per exact follow-up key"

requirements-completed: [STORE-03, RUNTIME-05, DOM-04, DOM-05, INT-04]

duration: session-executed
completed: 2026-04-20
---

# Phase 10 Plan 01: Duplicate-Safe Process Manager Follow-Up Keys Summary

**Commerce process-manager follow-up commands now use line-aware deterministic idempotency keys, so duplicate same-product order lines replay correctly without collapsing into the wrong reserve/release record.**

## Accomplishments

- Added `follow_up_line_key(...)` and switched reserve/release follow-up keys to `pm:{manager}:{source_event_id}:{action}:{line_index}:{product_id}`.
- Preserved confirm/reject key shapes so order-level follow-up semantics stayed stable.
- Stored `(line_index, product_id, quantity)` for successful reserves so compensation releases target the original successful line identity.
- Added app-level regressions proving duplicate same-product lines emit `reserve:0` and `reserve:1` keys and that failed duplicate-line compensation emits `release:0`.
- Upgraded the replay-aware test store to persist one `CommandReplayRecord` per exact idempotency key and proved duplicate-line retries replay two original reserve outcomes without appending fresh commands.

## Files Created/Modified

- `crates/app/src/commerce_process_manager.rs` - line-aware key construction, duplicate-line regressions, replay-aware exact-key store map, and duplicate-line replay test updates.
- `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VALIDATION.md` - marked validation complete.
- `.planning/ROADMAP.md` - marked Phase 10 and plan 10-01 complete.
- `.planning/REQUIREMENTS.md` - marked STORE-03, DOM-04, and INT-04 complete and updated traceability.
- `.planning/STATE.md` - advanced project state to Phase 11 readiness.

## Verification

- `cargo test -p app duplicate_product_lines_emit_distinct_reserve_keys -- --nocapture` ✅
- `cargo test -p app duplicate_product_line_failure_releases_distinct_prior_lines -- --nocapture` ✅
- `cargo test -p app process_manager_uses_deterministic_idempotency_keys -- --nocapture` ✅
- `cargo test -p app process_manager_replayed_followups_return_original_outcomes -- --nocapture` ✅
- `cargo test -p app commerce_process_manager -- --nocapture` ✅
- `cargo test -p es-runtime runtime_duplicate -- --nocapture` ✅

## Decisions Made

- Used source-event-local line ordinals instead of coalescing repeated product lines, which keeps command counts and reserve/release sequencing deterministic.
- Kept the fix inside the app process manager and tests because runtime/store replay infrastructure already accepted opaque tenant-scoped idempotency keys.
- Used `BTreeMap` for replay-aware test storage so replay position assertions remain deterministic.

## Deviations from Plan

None. The implementation stayed inside `crates/app/src/commerce_process_manager.rs` and matched the planned verification path.

## Next Phase Readiness

Phase 10 is complete. The remaining roadmap work is Phase 11 archive hygiene, HTTP E2E debt, observability/docs traceability cleanup, and final v1 wrap-up.

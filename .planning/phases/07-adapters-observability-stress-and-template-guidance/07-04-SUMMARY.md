---
phase: 07-adapters-observability-stress-and-template-guidance
plan: 04
subsystem: testing
tags: [rust, criterion, benchmarks, postgres, testcontainers, cqrs, outbox]

requires:
  - phase: 07-adapters-observability-stress-and-template-guidance
    provides: HTTP command adapter boundaries and runtime gateway APIs from Plan 07-01
provides:
  - Layer-separated Criterion benchmark harnesses for TEST-03
  - Ring-only, domain-only, adapter-only, storage-only, and projector/outbox scenario names
  - Self-contained PostgreSQL 18 Testcontainers harness for projector/outbox benchmarks
affects: [TEST-03, phase-07-stress, benchmark-guidance]

tech-stack:
  added: []
  patterns: [root benchmark package, Criterion layer microbenches, Testcontainers bench harness]

key-files:
  created:
    - benches/ring_only.rs
    - benches/domain_only.rs
    - benches/adapter_only.rs
    - benches/storage_only.rs
    - benches/projector_outbox.rs
    - migrations/20260417000000_event_store.sql
    - migrations/20260418000000_projection_read_models.sql
    - migrations/20260418010000_outbox.sql
  modified:
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "Root benches are owned by a minimal root package so `cargo bench --bench <name>` resolves the plan's root `benches/*.rs` artifacts."
  - "Storage-only benchmarks require explicit DATABASE_URL and do not auto-discover or fall back to in-memory storage."
  - "Projector/outbox benchmarks use their own PostgreSQL 18 Testcontainers harness and never depend on developer database configuration."

patterns-established:
  - "Ring-only benchmark files explicitly state they measure DisruptorPath publication/polling only, not service throughput."
  - "Layer benchmark files keep scenario names and imports aligned with the layer being measured."
  - "Root benchmark package uses root migrations for `sqlx::migrate!(\"./migrations\")` smoke compilation."

requirements-completed: [TEST-03]

duration: 15min 03s
completed: 2026-04-18
---

# Phase 07 Plan 04: Layer-Separated Benchmark Harnesses Summary

**Criterion benchmark artifacts now isolate ring, domain, adapter, storage, and projector/outbox paths so TEST-03 numbers cannot be confused with full service throughput.**

## Performance

- **Duration:** 15min 03s
- **Started:** 2026-04-18T14:38:06Z
- **Completed:** 2026-04-18T14:53:09Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments

- Added root Criterion bench targets for `ring_only`, `domain_only`, `adapter_only`, `storage_only`, and `projector_outbox`.
- Added ring-only benches using `es_runtime::DisruptorPath` only, with comments warning that these are not service throughput numbers.
- Added domain-only commerce aggregate `decide`/`apply` benches and adapter-only DTO/envelope/gateway admission benches.
- Added storage-only PostgreSQL append, OCC conflict, dedupe, and global read benches that require explicit `DATABASE_URL`.
- Added projector/outbox benches with a bench-local PostgreSQL 18 Testcontainers harness, `PostgresProjectionStore::catch_up`, and `dispatch_once` over `PostgresOutboxStore` plus `InMemoryPublisher`.

## Task Commits

Each task was committed atomically:

1. **Task 07-04-01: Add isolated microbenchmark harnesses** - `7ef2548` (feat)
2. **Task 07-04-02: Add storage and projector/outbox benchmark harnesses** - `04c1a01` (feat)

**Plan metadata:** this docs commit

## Files Created/Modified

- `Cargo.toml` - Added a minimal root benchmark package, dev dependencies, and bench targets.
- `Cargo.lock` - Locked Criterion and root benchmark package dependencies in Task 1.
- `benches/ring_only.rs` - DisruptorPath publication/polling benchmarks only.
- `benches/domain_only.rs` - In-memory commerce aggregate decision/replay benchmarks only.
- `benches/adapter_only.rs` - HTTP-shaped DTO decode, `CommandEnvelope`, and bounded `CommandGateway::try_submit` benchmarks only.
- `benches/storage_only.rs` - PostgreSQL event-store append, conflict, dedupe, and global read benchmarks requiring explicit database configuration.
- `benches/projector_outbox.rs` - PostgreSQL 18 container-backed projector catch-up and outbox claim/publish benchmarks.
- `migrations/20260417000000_event_store.sql` - Root benchmark package event-store schema migration.
- `migrations/20260418000000_projection_read_models.sql` - Root benchmark package projection schema migration.
- `migrations/20260418010000_outbox.sql` - Root benchmark package outbox schema migration.

## Decisions Made

- Added a root package because a virtual workspace manifest cannot own root `benches/*.rs` targets for `cargo bench --bench ...`.
- Kept storage-only and projector/outbox database setup intentionally different: storage-only uses explicit user-supplied database configuration; projector/outbox uses a disposable container.
- Duplicated the current storage crate migrations at the root benchmark package so the plan-required `sqlx::migrate!("./migrations")` path compiles for root benches.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a root benchmark package**
- **Found during:** Task 07-04-01
- **Issue:** The workspace root was a virtual manifest, so root `benches/*.rs` files were not discoverable by `cargo bench --bench <name>`.
- **Fix:** Added a minimal non-published root package, dev dependencies, and explicit bench targets.
- **Files modified:** `Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo bench --bench ring_only -- --warm-up-time 1 --measurement-time 3`; `cargo bench --bench adapter_only -- --warm-up-time 1 --measurement-time 3`
- **Committed in:** `7ef2548`

**2. [Rule 3 - Blocking] Added root migrations for root bench compilation**
- **Found during:** Task 07-04-02
- **Issue:** The plan required `sqlx::migrate!("./migrations")`, but that path did not exist for the new root benchmark package.
- **Fix:** Added root migration files mirroring the current event-store, projection, and outbox schema used by the storage crate tests.
- **Files modified:** `migrations/20260417000000_event_store.sql`, `migrations/20260418000000_projection_read_models.sql`, `migrations/20260418010000_outbox.sql`
- **Verification:** `cargo bench --bench storage_only --no-run`; `cargo bench --bench projector_outbox -- --warm-up-time 1 --measurement-time 3`
- **Committed in:** `04c1a01`

**3. [Rule 1 - Bug] Dropped Testcontainers handles inside the Tokio runtime**
- **Found during:** Task 07-04-02
- **Issue:** `projector_outbox` initially panicked after `projector_catch_up` because the async Testcontainers drop path ran outside a Tokio reactor.
- **Fix:** Dropped each harness inside `runtime.block_on(async move { drop(harness); })`.
- **Files modified:** `benches/projector_outbox.rs`
- **Verification:** `cargo bench --bench projector_outbox -- --warm-up-time 1 --measurement-time 3`
- **Committed in:** `04c1a01`

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 bug)
**Impact on plan:** All deviations were required for the benchmark artifacts to compile and run through the requested commands. Layer boundaries and scenario intent were preserved.

## Issues Encountered

- `cargo fmt --check` reported formatting in `crates/es-runtime/src/shard.rs`, which is concurrent Phase 07 work outside this plan. It was not modified by 07-04.
- Cargo build locks were held briefly by concurrent Phase 07 activity; benchmark commands waited and completed.
- `Cargo.lock` remained dirty from concurrent workspace dependency changes after the 07-04 commits. It was not included in Task 2 because Task 2 added no new dependencies.

## Known Stubs

None found.

## Threat Flags

None. The benchmark database and benchmark-report interpretation boundaries were covered by the plan threat model.

## Verification

- `rg 'criterion_group!|ring_only_publish_poll|ring_only_hot_key_publish_poll' benches/ring_only.rs` - PASS
- `rg 'domain_only_product_decide_apply|domain_only_order_lifecycle_decide_apply' benches/domain_only.rs` - PASS
- `rg 'adapter_only_decode_envelope_submit|adapter_only_burst_overload|CommandGateway|CommandEnvelope' benches/adapter_only.rs` - PASS
- `! rg 'PostgresEventStore|CommandEngine::process_one|dispatch_once' benches/ring_only.rs benches/domain_only.rs benches/adapter_only.rs` - PASS
- `rg 'storage_only_append|storage_only_occ_conflict|storage_only_dedupe|storage_only_global_read|DATABASE_URL' benches/storage_only.rs` - PASS
- `rg 'projector_catch_up|outbox_claim_publish|PostgresProjectionStore|PostgresOutboxStore|dispatch_once|InMemoryPublisher' benches/projector_outbox.rs` - PASS
- `rg 'testcontainers|testcontainers_modules::postgres::Postgres|with_tag\("18"\)|sqlx::migrate!\("./migrations"\)' benches/projector_outbox.rs` - PASS
- `! rg 'DATABASE_URL|requires DATABASE_URL' benches/projector_outbox.rs` - PASS
- `cargo bench --bench ring_only -- --warm-up-time 1 --measurement-time 3` - PASS
- `cargo bench --bench domain_only -- --warm-up-time 1 --measurement-time 3` - PASS
- `cargo bench --bench adapter_only -- --warm-up-time 1 --measurement-time 3` - PASS
- `cargo bench --bench storage_only --no-run` - PASS
- `cargo bench --bench projector_outbox -- --warm-up-time 1 --measurement-time 3` - PASS

## User Setup Required

None for the required smoke checks. Running `storage_only` measurements, not just `--no-run`, requires a developer-provided PostgreSQL `DATABASE_URL`.

## Next Phase Readiness

TEST-03 now has layer-separated benchmark artifacts for ring-only, domain-only, adapter-only, storage-only, and projector/outbox paths. Plan 07-05 can build integrated, hot-key, burst, and degraded-dependency stress coverage without reusing these microbenchmark numbers as service throughput.

## Self-Check: PASSED

- Verified key created files exist: `benches/ring_only.rs`, `benches/domain_only.rs`, `benches/adapter_only.rs`, `benches/storage_only.rs`, `benches/projector_outbox.rs`, and root migration files.
- Verified task commits exist in git history: `7ef2548`, `04c1a01`.
- Verified required smoke bench and grep commands passed.
- Verified `.planning/STATE.md` and `.planning/ROADMAP.md` were not modified by this executor.

---
*Phase: 07-adapters-observability-stress-and-template-guidance*
*Completed: 2026-04-18*

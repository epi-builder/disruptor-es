# Phase 03 Source Coverage Audit

SOURCE | ID | Feature/Requirement | Plan | Status | Notes
--- | --- | --- | --- | --- | ---
GOAL | - | Requests enter a bounded local command engine, route by aggregate/partition key to a single shard owner, execute through an in-process disruptor path, and reply only after event-store commit. | 01, 02, 03, 04 | COVERED | Contracts in 01, bounded routing/gateway in 02, disruptor/shard ownership in 03, commit-gated replies in 04.
REQ | RUNTIME-01 | Adapter requests enter bounded ingress with explicit overload behavior. | 02, 04 | COVERED | Gateway `try_send` overload in 02; runtime flow regression in 04.
REQ | RUNTIME-02 | Partition routing sends all commands for the same aggregate key to the same local shard owner. | 02 | COVERED | Fixed-seed tenant-aware `PartitionRouter` and golden tests.
REQ | RUNTIME-03 | Shard runtime owns processor-local aggregate cache and dedupe cache without global mutable business-state locks. | 03, 04 | COVERED | Shard-owned `HashMap` cache and forbidden-pattern greps.
REQ | RUNTIME-04 | Shard runtime integrates the `disruptor` crate as the local command execution/fan-out mechanism. | 03 | COVERED | `DisruptorPath` compile proof and `try_publish` full-ring handling.
REQ | RUNTIME-05 | Command replies are sent only after durable event-store append commit succeeds. | 01, 04 | COVERED | Outcome contract includes `CommittedAppend`; shard processing replies after append.
REQ | RUNTIME-06 | Optimistic concurrency conflicts are surfaced as typed retryable or conflict errors without corrupting shard-local cache. | 01, 04 | COVERED | Store conflict mapping in 01; cache-preserving conflict tests in 04.
RESEARCH | bounded-ingress | Use bounded Tokio mpsc plus `try_send`; map full/closed to typed runtime errors. | 02 | COVERED | `CommandGateway::try_submit` plan prohibits `.send().await`.
RESEARCH | stable-routing | Use fixed-seed stable hash over tenant + partition key modulo shard count. | 02 | COVERED | `ROUTING_HASH_SEED = 0x4553_5255_4e54494d` and golden tests.
RESEARCH | shard-local-cache | Use plain shard-local `HashMap`; do not introduce `moka` without bounded eviction requirement. | 01, 03 | COVERED | Dependency task forbids `moka`; cache task uses `HashMap`.
RESEARCH | disruptor-crate | Use `disruptor = "4.0.0"` instead of older literal `disruptor-rs` crate. | 01, 03 | COVERED | Workspace dependency and `DisruptorPath` plan.
RESEARCH | try-publish | Use `Producer::try_publish`; map full ring to typed overload instead of spinning. | 03 | COVERED | `DisruptorPath::try_publish` task and full-ring tests.
RESEARCH | reply-after-commit | Call event-store append before success reply; include committed positions. | 01, 04 | COVERED | `CommandOutcome` and `process_next_handoff`.
RESEARCH | conflict-cache-safety | Preserve cache on `StoreError::StreamConflict`; return typed conflict. | 01, 04 | COVERED | `RuntimeError::from_store_error` and conflict tests.
RESEARCH | async-bridge | Include early compile/feasibility proof for disruptor sync processor to async storage bridge. | 03 | COVERED | Plan 03 keeps storage out of disruptor handler and greps for blocking runtime calls.
RESEARCH | no-global-locks | Avoid global `Arc<Mutex<_>>` business-state maps. | 03, 04 | COVERED | Production source greps in plans 03 and 04.
RESEARCH | runtime-store-trait | Use a small runtime-facing storage trait only if it materially simplifies tests. | 01 | COVERED | Plan 01 creates append/rehydration-only `RuntimeEventStore` for fake-store tests.
RESEARCH | validation | Create Wave 0 artifacts/tests consistent with `03-VALIDATION.md`. | 01, 02, 03, 04 | COVERED | Each plan updates validation status and creates named test files.
CONTEXT | none | No Phase 03 CONTEXT.md exists. | - | COVERED | Planning used requirements, research, patterns, roadmap, and state only.

No unplanned source items found.

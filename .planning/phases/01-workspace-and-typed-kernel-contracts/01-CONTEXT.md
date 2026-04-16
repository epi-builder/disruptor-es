# Phase 01: Workspace and Typed Kernel Contracts - Context

**Gathered:** 2026-04-16
**Status:** Ready for planning

<domain>

## Phase Boundary

Phase 01 creates the Rust 2024 workspace and typed event-sourcing kernel contracts that later phases will build on. The phase delivers crate boundaries, reusable core types, a synchronous deterministic aggregate contract, a small example-domain proof point, and verification that lower-level kernel crates stay free of runtime, storage, broker, network, and adapter dependencies.

This phase does not implement durable event storage, disruptor runtime execution, CQRS projection, outbox dispatch, HTTP/gRPC adapters, or production domain workflows. Those concerns may appear as crate shells or compile-time boundaries only.

</domain>

<decisions>

## Implementation Decisions

### Toolchain and Workspace Policy

- **D-01:** Add `rust-toolchain.toml` and pin the workspace to Rust 1.85 or newer so Rust 2024 support is reproducible locally and in downstream automation.
- **D-02:** Set `edition = "2024"` and `rust-version = "1.85"` through workspace package inheritance in the root `Cargo.toml`.
- **D-03:** Use Cargo workspace `resolver = "3"`, workspace dependency inheritance, and workspace lints so dependency and lint policy is centralized.

### Crate Boundary Strictness

- **D-04:** Create the full Phase 1 crate shell: `es-core`, `es-kernel`, `es-runtime`, `es-store-postgres`, `es-projection`, `es-outbox`, `example-commerce`, adapter crates, and app composition.
- **D-05:** `es-core` and `es-kernel` must not depend on Tokio, SQLx, Axum, Tonic, broker clients, PostgreSQL-specific types, network types, or adapter crates.
- **D-06:** Runtime, storage, projection, outbox, adapter, and app crates may exist as placeholders in Phase 1, but their purpose is boundary visibility and future integration points, not implementation of later-phase behavior.

### Core ID and Metadata Model

- **D-07:** Start with opaque core newtypes, not highly structured aggregate-specific key types: `StreamId(String)`, `PartitionKey(String)`, `TenantId(String)`, and `StreamRevision(u64)`.
- **D-08:** Add `ExpectedRevision` in Phase 1 with at least `Any`, `NoStream`, and `Exact(StreamRevision)`.
- **D-09:** Command metadata is explicit and required at the kernel boundary: `command_id`, `correlation_id`, `causation_id`, `tenant_id`, and request timestamp. `tenant_id` is a required `TenantId(String)`, not optional.
- **D-10:** Use UUID-backed identifiers for command, event, correlation, and causation IDs where appropriate, while keeping stream and partition identity storage-agnostic.

### Aggregate Trait Shape

- **D-11:** Use one associated-type `Aggregate` trait in `es-kernel` with typed `State`, `Command`, `Event`, `Reply`, and `Error`.
- **D-12:** The trait includes `stream_id`, `partition_key`, `expected_revision`, `decide`, and `apply` in Phase 1. Do not split routing/revision into separate traits yet.
- **D-13:** `decide` and `apply` are synchronous, deterministic, side-effect-free functions. They must not be `async`, call databases, call network services, perform broker publication, read clocks directly, generate randomness, or mutate shared global business state.
- **D-14:** `decide` returns a typed decision result, not raw events only: `Decision { events: Vec<Event>, reply: Reply }`. This preserves the Phase 1 requirement for typed replies while keeping durable commit semantics for later phases.
- **D-15:** Avoid trait-object or dynamic JSON/reflection kernels in Phase 1. Do not use `Box<dyn Aggregate>`, `serde_json::Value` as command/event core representation, or plugin-style runtime dispatch for the hot domain contract.

### Contract Verification Level

- **D-16:** Phase 1 verification should be strong, not merely a successful workspace build.
- **D-17:** Include compile/build checks, focused unit tests, dependency-boundary checks, replay determinism tests, and property-style tests where the example aggregate makes that practical.
- **D-18:** Snapshot fixtures are acceptable for stable contract examples, but unstable IDs/timestamps must be controlled or redacted.
- **D-19:** Verification should prove that `es-core` and `es-kernel` remain pure Rust domain/kernel crates and do not acquire runtime/storage/adapter dependencies.

### the agent's Discretion

- The planner may choose exact module names and file organization inside each crate as long as the crate boundaries and dependency rules above are preserved.
- The planner may choose whether adapter crates are named `adapter-http`/`adapter-grpc` or use an `es-adapter-*` prefix, provided the naming is consistent and clear.
- The planner may choose the smallest example aggregate needed in Phase 1 to exercise the kernel trait, as long as it does not pull Phase 4 commerce behavior forward.

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Scope and Requirements

- `.planning/PROJECT.md` - Project vision, source-of-truth boundary, hot-path rules, and out-of-scope constraints.
- `.planning/REQUIREMENTS.md` - CORE-01 through CORE-04 acceptance requirements for this phase.
- `.planning/ROADMAP.md` - Phase 01 goal, dependencies, and success criteria.
- `.planning/STATE.md` - Current phase status and carried-forward project decisions.

### Phase Research

- `.planning/phases/01-workspace-and-typed-kernel-contracts/01-RESEARCH.md` - Prescriptive research for workspace topology, standard stack, architecture patterns, pitfalls, and code examples.

</canonical_refs>

<code_context>

## Existing Code Insights

### Reusable Assets

- No implementation code exists yet. The repository currently contains planning artifacts and `AGENTS.md`.

### Established Patterns

- GSD planning artifacts are the current source of project direction.
- Project instructions require using GSD workflow entry points before repo edits.
- Node tooling should prefer `pnpm`; Python tooling should prefer `uv`. Phase 1 is Rust-first, so these are only relevant for auxiliary tooling.

### Integration Points

- New implementation should start at the workspace root with `Cargo.toml`, `rust-toolchain.toml`, and crate directories under `crates/`.
- Phase 1 planning should use `01-RESEARCH.md` as the implementation guide and keep future storage/runtime/adapter behavior limited to boundaries.

</code_context>

<specifics>

## Specific Ideas

- The core mental model is `Command -> decide(state) -> Decision { events, reply }` and `Events -> apply(state)`.
- The kernel must remain pure, deterministic, replayable, and free of side effects.
- Explicitly avoid early generic flexibility, early plugin systems, trait-object dispatch, dynamic JSON hot-path representations, async decision logic, repository access in kernel code, database access in kernel code, and shared mutable business state such as `Arc<Mutex<HashMap<...>>>`.
- Preserve the three project anchors: Disruptor is the in-process execution engine, the event store is the source of truth, and single-owner shard routing is the concurrency model.

</specifics>

<deferred>

## Deferred Ideas

None - discussion stayed within phase scope.

</deferred>

---

*Phase: 01-workspace-and-typed-kernel-contracts*
*Context gathered: 2026-04-16*

# Phase 01: Workspace and Typed Kernel Contracts - Pattern Map

**Mapped:** 2026-04-16
**Files analyzed:** 23
**Analogs found:** 23 / 23 planning/research analogs, 0 / 23 local implementation analogs

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `rust-toolchain.toml` | config | build/toolchain | `01-CONTEXT.md` lines 20-24 | research-only |
| `Cargo.toml` | config | build/workspace | `01-RESEARCH.md` lines 328-351 | research-only |
| `crates/es-core/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 353-369 | role-match |
| `crates/es-core/src/lib.rs` | model | transform | `01-RESEARCH.md` lines 195-229 | exact research pattern |
| `crates/es-kernel/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 353-369 | exact research pattern |
| `crates/es-kernel/src/lib.rs` | service/trait | transform | `01-RESEARCH.md` lines 231-264 | exact research pattern |
| `crates/es-runtime/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/es-runtime/src/lib.rs` | service | event-driven | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `crates/es-store-postgres/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/es-store-postgres/src/lib.rs` | service | CRUD | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `crates/es-projection/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/es-projection/src/lib.rs` | service | event-driven | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `crates/es-outbox/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/es-outbox/src/lib.rs` | service | event-driven/pub-sub | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `crates/example-commerce/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 353-369 | role-match |
| `crates/example-commerce/src/lib.rs` | model/test fixture | transform | `01-RESEARCH.md` lines 371-429 | exact research pattern |
| `crates/adapter-http/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/adapter-http/src/lib.rs` | controller/adapter | request-response | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `crates/adapter-grpc/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/adapter-grpc/src/lib.rs` | controller/adapter | request-response | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `crates/app/Cargo.toml` | config | build/crate | `01-RESEARCH.md` lines 123-160 | role-match |
| `crates/app/src/main.rs` | app/composition | request-response/bootstrap | `01-RESEARCH.md` lines 101-121 | boundary-only |
| `tests/dependency_boundaries.rs` | test | validation/batch | `01-RESEARCH.md` lines 319-324 and 430-436 | exact research pattern |

## Pattern Assignments

### `rust-toolchain.toml` (config, build/toolchain)

**Analog:** `01-CONTEXT.md`

**Toolchain policy pattern** (lines 20-24):

```text
- **D-01:** Add `rust-toolchain.toml` and pin the workspace to Rust 1.85 or newer so Rust 2024 support is reproducible locally and in downstream automation.
- **D-02:** Set `edition = "2024"` and `rust-version = "1.85"` through workspace package inheritance in the root `Cargo.toml`.
- **D-03:** Use Cargo workspace `resolver = "3"`, workspace dependency inheritance, and workspace lints so dependency and lint policy is centralized.
```

**Apply:** Use a minimal toolchain file that selects Rust 1.85+ for the workspace. The exact channel can be `1.85` or a newer stable toolchain, but it must satisfy Rust 2024.

---

### `Cargo.toml` (config, build/workspace)

**Analog:** `01-RESEARCH.md`

**Root workspace manifest pattern** (lines 328-351):

```toml
# Source: Cargo Book workspace docs and verified crate metadata.
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
thiserror = "2.0.18"
uuid = { version = "1.23.0", features = ["serde", "v7"] }
time = { version = "0.3.47", features = ["serde", "formatting", "parsing"] }

[workspace.lints.rust]
unsafe_code = "forbid"
```

**Boundary intent** (lines 164-168):

```text
Put edition, MSRV, license, common dependencies, and lints in the root workspace manifest; make each member opt into package metadata and dependencies explicitly.
```

**Apply:** Treat `[workspace.dependencies]` as a version catalog. Do not make core/kernel inherit runtime, storage, HTTP, gRPC, or broker dependencies.

---

### Crate `Cargo.toml` files (config, build/crate)

**Applies to:** `crates/es-core/Cargo.toml`, `crates/es-kernel/Cargo.toml`, `crates/es-runtime/Cargo.toml`, `crates/es-store-postgres/Cargo.toml`, `crates/es-projection/Cargo.toml`, `crates/es-outbox/Cargo.toml`, `crates/example-commerce/Cargo.toml`, `crates/adapter-http/Cargo.toml`, `crates/adapter-grpc/Cargo.toml`, `crates/app/Cargo.toml`

**Analog:** `01-RESEARCH.md`

**Member manifest pattern** (lines 353-369):

```toml
# Source: Cargo workspace dependency inheritance docs.
[package]
name = "es-kernel"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
es-core = { path = "../es-core" }

[lints]
workspace = true
```

**Apply:** Change `name` and dependencies per crate. All crates should inherit package fields and lints. Only add dependencies actually used by that crate.

**Dependency direction:**

- `es-core`: allow `serde`, `uuid`, `time`; no project crate dependencies.
- `es-kernel`: depend on `es-core`; optionally `thiserror` only if kernel-owned errors exist.
- `example-commerce`: depend on `es-core`, `es-kernel`, and `thiserror`.
- Boundary placeholder crates: keep dependencies empty unless a compile-visible type requires `es-core` or `es-kernel`.
- Adapter/app crates: may exist as placeholders in Phase 1; do not implement real Axum/Tonic behavior yet unless the plan explicitly scopes it as compile-only.

---

### `crates/es-core/src/lib.rs` (model, transform)

**Analog:** `01-RESEARCH.md`

**Core newtype pattern** (lines 195-229):

```rust
// Source: project requirement CORE-03; serde/uuid/time crates verified through cargo metadata.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StreamId(String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PartitionKey(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct StreamRevision(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedRevision {
    Any,
    NoStream,
    Exact(StreamRevision),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CommandMetadata {
    pub command_id: uuid::Uuid,
    pub correlation_id: uuid::Uuid,
    pub causation_id: Option<uuid::Uuid>,
    pub tenant_id: Option<uuid::Uuid>,
    pub requested_at: time::OffsetDateTime,
}
```

**Required override from context** (lines 34-37):

```text
- **D-07:** Start with opaque core newtypes, not highly structured aggregate-specific key types: `StreamId(String)`, `PartitionKey(String)`, `TenantId(String)`, and `StreamRevision(u64)`.
- **D-08:** Add `ExpectedRevision` in Phase 1 with at least `Any`, `NoStream`, and `Exact(StreamRevision)`.
- **D-09:** Command metadata is explicit and required at the kernel boundary: `command_id`, `correlation_id`, `causation_id`, `tenant_id`, and request timestamp. `tenant_id` is a required `TenantId(String)`, not optional.
- **D-10:** Use UUID-backed identifiers for command, event, correlation, and causation IDs where appropriate, while keeping stream and partition identity storage-agnostic.
```

**Apply:** Copy the newtype shape, but add `TenantId(String)` and make `CommandMetadata.tenant_id: TenantId`, not `Option<uuid::Uuid>`. Add event metadata if needed by CORE-03 with the same explicit typed style.

**Validation pattern:** Prefer constructors such as `StreamId::new(value) -> Result<Self, CoreError>` or `TryFrom<String>` if enforcing non-empty IDs. Keep validation local to core types.

---

### `crates/es-kernel/src/lib.rs` (service/trait, transform)

**Analog:** `01-RESEARCH.md`

**Aggregate trait pattern** (lines 231-264):

```rust
// Source: project requirement CORE-02/CORE-04.
pub trait Aggregate {
    type State: Default + Clone + PartialEq;
    type Command;
    type Event: Clone;
    type Reply;
    type Error;

    fn stream_id(command: &Self::Command) -> StreamId;
    fn partition_key(command: &Self::Command) -> PartitionKey;
    fn expected_revision(command: &Self::Command) -> ExpectedRevision;

    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error>;

    fn apply(state: &mut Self::State, event: &Self::Event);
}

pub struct Decision<E, R> {
    pub events: Vec<E>,
    pub reply: R,
}
```

**Locked trait decisions** (lines 41-45):

```text
- **D-11:** Use one associated-type `Aggregate` trait in `es-kernel` with typed `State`, `Command`, `Event`, `Reply`, and `Error`.
- **D-12:** The trait includes `stream_id`, `partition_key`, `expected_revision`, `decide`, and `apply` in Phase 1. Do not split routing/revision into separate traits yet.
- **D-13:** `decide` and `apply` are synchronous, deterministic, side-effect-free functions.
- **D-14:** `decide` returns a typed decision result, not raw events only: `Decision { events: Vec<Event>, reply: Reply }`.
- **D-15:** Avoid trait-object or dynamic JSON/reflection kernels in Phase 1.
```

**Apply:** Do not use `async_trait`, `Box<dyn Aggregate>`, `serde_json::Value`, database handles, clocks, random generation, or network types in `es-kernel`.

---

### Boundary crate `src/lib.rs` files (placeholder services/adapters)

**Applies to:** `crates/es-runtime/src/lib.rs`, `crates/es-store-postgres/src/lib.rs`, `crates/es-projection/src/lib.rs`, `crates/es-outbox/src/lib.rs`, `crates/adapter-http/src/lib.rs`, `crates/adapter-grpc/src/lib.rs`

**Analog:** `01-RESEARCH.md`

**Responsibility map pattern** (lines 101-121):

```text
Later phase boundaries, present as crates only:
  es-runtime -> will own command routing/shards/disruptor
  es-store-postgres -> will own durable append
  es-projection -> will own projectors
  es-outbox -> will own dispatcher/publisher contracts
  adapter-http / adapter-grpc -> will decode requests only
  app -> will compose runtime/storage/adapters
```

**Boundary strictness** (context lines 28-30):

```text
- **D-04:** Create the full Phase 1 crate shell: `es-core`, `es-kernel`, `es-runtime`, `es-store-postgres`, `es-projection`, `es-outbox`, `example-commerce`, adapter crates, and app composition.
- **D-05:** `es-core` and `es-kernel` must not depend on Tokio, SQLx, Axum, Tonic, broker clients, PostgreSQL-specific types, network types, or adapter crates.
- **D-06:** Runtime, storage, projection, outbox, adapter, and app crates may exist as placeholders in Phase 1, but their purpose is boundary visibility and future integration points, not implementation of later-phase behavior.
```

**Apply:** Use minimal documented placeholder modules/types. Avoid implementing storage, runtime, projection, outbox, HTTP, or gRPC behavior in Phase 1. If `missing_docs = "warn"` is enabled, add crate-level docs explaining the future responsibility.

---

### `crates/example-commerce/src/lib.rs` (model/test fixture, transform)

**Analog:** `01-RESEARCH.md`

**Example aggregate pattern** (lines 371-429):

```rust
// Source: project CORE-02/CORE-04 requirements.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct CounterState {
    pub value: i64,
}

pub enum CounterCommand {
    Increment { stream_id: StreamId, by: i64 },
}

#[derive(Clone, PartialEq, Eq)]
pub enum CounterEvent {
    Incremented { by: i64 },
}

pub enum CounterReply {
    Accepted,
}

#[derive(Debug, thiserror::Error)]
pub enum CounterError {
    #[error("increment must be positive")]
    NonPositiveIncrement,
}

pub struct Counter;

impl Aggregate for Counter {
    type State = CounterState;
    type Command = CounterCommand;
    type Event = CounterEvent;
    type Reply = CounterReply;
    type Error = CounterError;

    fn decide(
        state: &Self::State,
        command: Self::Command,
        metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        let _ = (state, metadata);
        match command {
            CounterCommand::Increment { by, .. } if by <= 0 => Err(CounterError::NonPositiveIncrement),
            CounterCommand::Increment { by, .. } => Ok(Decision {
                events: vec![CounterEvent::Incremented { by }],
                reply: CounterReply::Accepted,
            }),
        }
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        match event {
            CounterEvent::Incremented { by } => state.value += by,
        }
    }
}
```

**Scope guard** (context lines 56-58):

```text
- The planner may choose exact module names and file organization inside each crate as long as the crate boundaries and dependency rules above are preserved.
- The planner may choose whether adapter crates are named `adapter-http`/`adapter-grpc` or use an `es-adapter-*` prefix, provided the naming is consistent and clear.
- The planner may choose the smallest example aggregate needed in Phase 1 to exercise the kernel trait, as long as it does not pull Phase 4 commerce behavior forward.
```

**Apply:** Build the smallest commerce-flavored aggregate fixture needed to prove the contract. Keep state explicit and replayable. Use typed commands/events/replies/errors and `thiserror` for the error enum.

---

### `crates/app/src/main.rs` (app/composition, bootstrap)

**Analog:** `01-RESEARCH.md`

**Composition boundary pattern** (lines 112-118):

```text
  es-runtime -> will own command routing/shards/disruptor
  es-store-postgres -> will own durable append
  es-projection -> will own projectors
  es-outbox -> will own dispatcher/publisher contracts
  adapter-http / adapter-grpc -> will decode requests only
  app -> will compose runtime/storage/adapters
```

**Apply:** Keep `main` compile-only in Phase 1. A minimal `fn main() {}` with crate docs/comments is acceptable unless the planner chooses a simple placeholder banner. Do not wire runtime/storage/adapters yet.

---

### `tests/dependency_boundaries.rs` (test, validation/batch)

**Analog:** `01-RESEARCH.md`

**Verification gap pattern** (lines 319-324):

```text
**What goes wrong:** Phase 1 compiles but does not prove replay determinism or crate boundary rules.
**How to avoid:** Include a minimal example aggregate plus tests for `decide`, `apply`, replay equivalence, and `cargo tree` dependency absence.
**Warning signs:** No test calls `Aggregate::decide` and no test/CI command checks dependency boundaries.
```

**Boundary command pattern** (lines 430-436):

```bash
# Source: Cargo package selection/workspace commands docs.
cargo tree -p es-kernel
cargo tree -p es-core
cargo check --workspace
cargo test --workspace
```

**Apply:** Create automated or script-backed tests/checks that fail if `es-core` or `es-kernel` acquire forbidden dependencies. At minimum, document and verify:

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo tree -p es-core`
- `cargo tree -p es-kernel`

## Shared Patterns

### Dependency Boundary

**Source:** `01-CONTEXT.md` lines 28-30 and `01-RESEARCH.md` lines 291-296
**Apply to:** `es-core`, `es-kernel`, placeholder boundary crates, dependency boundary tests

```text
`es-core` and `es-kernel` must not depend on Tokio, SQLx, Axum, Tonic, broker clients, PostgreSQL-specific types, network types, or adapter crates.
```

```text
Keep root `[workspace.dependencies]` as a version catalog only; each member must opt into dependencies explicitly.
```

### Deterministic Kernel

**Source:** `01-CONTEXT.md` lines 41-45 and `01-RESEARCH.md` lines 305-310
**Apply to:** `es-kernel`, `example-commerce`, tests

```text
`decide` and `apply` are synchronous, deterministic, side-effect-free functions.
```

```text
Generate IDs/timestamps at command envelope boundaries and pass them through metadata; `decide` should use only command, state, and metadata inputs.
```

### Typed Data Contracts

**Source:** `01-CONTEXT.md` lines 34-37 and `01-RESEARCH.md` lines 195-229
**Apply to:** `es-core`, `es-kernel`, `example-commerce`

```text
Start with opaque core newtypes, not highly structured aggregate-specific key types: `StreamId(String)`, `PartitionKey(String)`, `TenantId(String)`, and `StreamRevision(u64)`.
```

Use derives for clone/debug/equality/hash/serde where appropriate. Keep hot-path domain commands/events typed, not JSON-erased.

### Test Strength

**Source:** `01-CONTEXT.md` lines 49-52
**Apply to:** workspace checks, crate tests, boundary checks

```text
Include compile/build checks, focused unit tests, dependency-boundary checks, replay determinism tests, and property-style tests where the example aggregate makes that practical.
```

### Project Constraints

**Source:** `AGENTS.md` lines 14-21
**Apply to:** all implementation planning

```text
- Prefer `pnpm` for Node tooling and `uv` for Python tooling.
- Rust-first service implementation.
- Event store is the source of truth; disruptor rings must never be durable state.
- Same aggregate or ordered partition key must map to the same shard owner.
- Hot business state should be single-owner and processor-local where practical.
- External publication must flow through outbox rows committed in the same transaction as domain events.
```

## No Local Implementation Analog Found

The repository currently contains planning artifacts and `AGENTS.md` only; `rg --files` found no existing `Cargo.toml`, `rust-toolchain.toml`, or Rust source files. The planner should use the research/context excerpts above as source patterns.

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| All 23 target files | mixed | mixed | Greenfield implementation; no source code exists yet. |

## Metadata

**Analog search scope:** repository root, `.planning/phases/01-workspace-and-typed-kernel-contracts`, `.planning/research`, project root `AGENTS.md`
**Files scanned:** 6 primary files (`AGENTS.md`, `01-CONTEXT.md`, `01-RESEARCH.md`, plus repository file listings and planning references)
**Project skills:** none found under `.claude/skills/` or `.agents/skills/`
**Pattern extraction date:** 2026-04-16

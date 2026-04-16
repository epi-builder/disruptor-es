# Phase 1: Workspace and Typed Kernel Contracts - Research

**Researched:** 2026-04-16 [VERIFIED: local date/context]
**Domain:** Rust 2024 workspace topology and typed event-sourcing kernel contracts [VERIFIED: .planning/ROADMAP.md]
**Confidence:** HIGH for workspace/kernel boundaries; MEDIUM for exact final trait ergonomics until implementation tests exercise example domains [VERIFIED: .planning/REQUIREMENTS.md]

## User Constraints

No phase `CONTEXT.md` exists yet, so there are no additional locked decisions, discretion notes, or deferred ideas to copy for this phase. [VERIFIED: `node gsd-tools.cjs init phase-op 1` returned `has_context: false`]

## Summary

Phase 1 should establish a Rust 2024 virtual workspace with crate boundaries that make dependency direction visible before runtime behavior exists. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] The root workspace should use `resolver = "3"`, `[workspace.package]` inheritance for `edition = "2024"` and `rust-version = "1.85"`, `[workspace.dependencies]` for shared crate versions, and `[workspace.lints]` to propagate lint policy to member crates. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html]

The deterministic domain surface belongs in `es-core` and `es-kernel`; runtime, storage, projection, outbox, adapters, and app composition should exist as separate crates but must not be dependencies of core/kernel crates in this phase. [VERIFIED: .planning/REQUIREMENTS.md] The aggregate kernel should be synchronous and typed: command in, current aggregate state in, zero-or-more events plus reply out, with stream IDs, partition keys, expected revisions, command metadata, and event metadata represented by reusable core types. [VERIFIED: .planning/REQUIREMENTS.md]

**Primary recommendation:** Build a strict Rust 2024 workspace and typed synchronous aggregate contract first; add runtime/storage/adapters only as boundary crates with dependency rules, not as active concerns. [VERIFIED: .planning/ROADMAP.md]

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-01 | Developer can create and build a Rust 2024 workspace with separate crates for core types, domain kernel, runtime, storage, projection, outbox, example domain, adapters, and app composition. [VERIFIED: .planning/REQUIREMENTS.md] | Use a virtual Cargo workspace with `crates/*`, `resolver = "3"`, shared workspace package metadata, and explicit member crates. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| CORE-02 | Developer can define typed commands, events, aggregate state, replies, and errors through a generic aggregate kernel trait. [VERIFIED: .planning/REQUIREMENTS.md] | Use associated types on a synchronous `Aggregate` trait and keep domain errors typed with `thiserror`. [VERIFIED: cargo info thiserror] |
| CORE-03 | Developer can derive stream IDs, partition keys, expected revisions, command metadata, and event metadata through reusable core types. [VERIFIED: .planning/REQUIREMENTS.md] | Put newtype wrappers and metadata structs in `es-core`, backed by `uuid`, `time`, and `serde` where serialization is needed. [VERIFIED: cargo search/cargo info uuid, time, serde] |
| CORE-04 | Domain decision logic is synchronous, deterministic, typed, and free of adapter, database, broker, and network dependencies. [VERIFIED: .planning/REQUIREMENTS.md] | Make `es-kernel` depend only on `es-core` and small typed-support crates; enforce boundaries with `cargo tree -p es-kernel` checks and no `tokio`, `sqlx`, `axum`, `tonic`, or broker dependencies. [VERIFIED: .planning/research/STACK.md] |

</phase_requirements>

## Project Constraints

- Prefer `pnpm` for Node tooling and `uv` for Python tooling. [VERIFIED: user-provided AGENTS.md instructions]
- Implement the service Rust-first around `disruptor-rs`/`disruptor`, with committed events as source of truth. [VERIFIED: user-provided AGENTS.md project doc]
- Do not treat disruptor rings as durable state. [VERIFIED: user-provided AGENTS.md project doc]
- Route the same aggregate or ordered partition key to the same shard owner in later runtime phases. [VERIFIED: user-provided AGENTS.md project doc]
- Keep hot business state single-owner and processor-local where practical; do not hide it behind shared mutable adapter state. [VERIFIED: user-provided AGENTS.md project doc]
- External publication must flow through durable outbox rows committed with domain events in later storage/outbox phases. [VERIFIED: user-provided AGENTS.md project doc]
- Keep adapter, command engine, projection, and outbox concerns separable. [VERIFIED: user-provided AGENTS.md project doc]
- Performance tests must separate ring-only, domain-only, adapter-only, full E2E, soak, and chaos scenarios in later phases. [VERIFIED: user-provided AGENTS.md project doc]
- No `CLAUDE.md` file exists in the repo, so no additional CLAUDE-specific directives were found. [VERIFIED: `test -f CLAUDE.md`]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Workspace topology | Build system / repository | Developer tooling | Cargo owns workspace membership, package inheritance, dependency resolution, lockfile, and lint configuration. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| Core IDs and metadata | Domain core crate | Storage/adapters later serialize them | IDs, revisions, and metadata are shared vocabulary, but Phase 1 should keep them storage-agnostic. [VERIFIED: CORE-03 in .planning/REQUIREMENTS.md] |
| Aggregate kernel trait | Domain kernel crate | Example domain | The trait defines deterministic business decision contracts and should not know about async runtimes, databases, brokers, or protocols. [VERIFIED: CORE-02 and CORE-04 in .planning/REQUIREMENTS.md] |
| Example typed aggregate | Example domain crate | Kernel test suite | The fixture proves trait ergonomics and replay determinism without introducing runtime or storage dependencies. [VERIFIED: .planning/ROADMAP.md] |
| Runtime/storage/adapter boundaries | Separate placeholder crates | Later phases | Phase 1 must expose crate boundaries so later phases can add implementation without back-propagating dependencies into core/kernel. [VERIFIED: CORE-01 and CORE-04 in .planning/REQUIREMENTS.md] |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust | Edition 2024, `rust-version = "1.85"` | Language and MSRV baseline | Rust 1.85.0 stabilized the 2024 edition, and Cargo manifests support `edition = "2024"` plus `rust-version`. [CITED: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/] [CITED: https://doc.rust-lang.org/cargo/reference/manifest.html] |
| Cargo workspaces | Cargo with resolver 3 | Multi-crate workspace, dependency inheritance, lint inheritance | Cargo supports virtual workspaces, explicit members, `resolver`, `[workspace.package]`, `[workspace.dependencies]`, and `[workspace.lints]`. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| `serde` | 1.0.228 | Derive serialization for metadata/contracts where boundary encoding is needed | Serde derive generates `Serialize` and `Deserialize` for Rust structs/enums and is the ecosystem standard for Rust data boundaries. [VERIFIED: cargo info serde] [CITED: https://serde.rs/derive.html] |
| `uuid` | 1.23.0 | Command IDs, event IDs, correlation IDs, causation IDs, tenant IDs where UUID-backed | Current crate metadata exposes `v7` and `serde` feature flags; 1.23.0 requires Rust 1.85, aligning with this phase MSRV. [VERIFIED: cargo info uuid@1.23.0] |
| `time` | 0.3.47 | Event/command metadata timestamps | `cargo search` reports 0.3.47 as latest, and the local Cargo 1.82 failed to parse it because it uses edition 2024, confirming the toolchain gap. [VERIFIED: cargo search time; cargo info time@0.3.47 failure on Cargo 1.82] |
| `thiserror` | 2.0.18 | Typed domain/example error enums | `thiserror` provides `derive(Error)` and keeps errors explicit without using `anyhow` in public kernel/domain APIs. [VERIFIED: cargo info thiserror] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde_json` | 1.0.149 | JSON fixture/snapshot helpers for tests and docs | Use in tests or examples that need inspectable metadata/event fixture output; do not make hot-path domain logic operate on `serde_json::Value`. [VERIFIED: cargo info serde_json] [VERIFIED: .planning/research/PITFALLS.md] |
| `proptest` | 1.11.0 | Property tests for replay determinism and routing/type invariants | Use for generated command/event sequences once an example aggregate exists; 1.11.0 requires Rust 1.85. [VERIFIED: cargo info proptest@1.11.0] |
| `insta` | 1.47.2 | Snapshot tests for contract fixtures and generated docs/examples | Use with redaction for unstable IDs/timestamps. [VERIFIED: cargo info insta] [ASSUMED] |
| `cargo-deny` | 0.19.4 | Dependency policy checks for licenses/advisories/duplicates | Add config early for a reusable template; local Cargo 1.82 cannot parse latest due edition 2024, so install after toolchain upgrade. [VERIFIED: cargo search cargo-deny; cargo info cargo-deny@0.19.4 failure on Cargo 1.82] |
| `cargo-nextest` | 0.9.133 | Faster workspace test runner | Use as an optional developer tool after Rust toolchain upgrade; local Cargo 1.82 cannot parse latest due edition 2024. [VERIFIED: cargo search cargo-nextest; cargo info cargo-nextest@0.9.133 failure on Cargo 1.82] |
| `cargo-llvm-cov` | 0.8.5 | Coverage for kernel/domain tests | Latest metadata requires Rust 1.87, so do not make it a Phase 1 blocker unless the toolchain is upgraded beyond 1.85. [VERIFIED: cargo info cargo-llvm-cov@0.8.5] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Cargo workspace with typed crates | Single crate with modules | A single crate hides dependency boundaries and makes CORE-04 harder to verify mechanically. [VERIFIED: CORE-01/CORE-04 in .planning/REQUIREMENTS.md] |
| Associated-type `Aggregate` trait | Trait objects over erased command/event payloads | Erasure weakens compile-time command/event compatibility and invites JSON/reflection logic in the hot path. [VERIFIED: .planning/research/PITFALLS.md] |
| Newtype wrappers for IDs/revisions | Raw `String`, `Uuid`, or `i64` everywhere | Raw primitives make stream IDs, partition keys, revisions, and metadata easy to mix up across boundaries. [ASSUMED] |
| `thiserror` in domain/examples | `anyhow` in domain public API | `anyhow` is useful at app/bootstrap edges, but public kernel/domain contracts should expose typed errors. [VERIFIED: .planning/research/STACK.md] |

**Installation:**

```bash
cargo add serde@1.0.228 --features derive
cargo add uuid@1.23.0 --features serde,v7
cargo add time@0.3.47 --features serde,formatting,parsing
cargo add thiserror@2.0.18
cargo add --dev serde_json@1.0.149 proptest@1.11.0 insta@1.47.2
```

**Version verification:** Versions above were checked with `cargo search` and `cargo info` on 2026-04-16; local Cargo 1.82 could not parse some edition-2024 crates, so the implementation must upgrade Rust before locking final dependencies. [VERIFIED: local command output]

## Architecture Patterns

### System Architecture Diagram

```text
Developer-defined domain types
  -> es-core IDs / metadata / revision types
  -> es-kernel Aggregate trait
       -> decide(state, command, metadata) -> events + reply
       -> apply(state, event) -> new state
  -> example-commerce aggregate fixture
       -> property/snapshot tests

Later phase boundaries, present as crates only:
  es-runtime -> will own command routing/shards/disruptor
  es-store-postgres -> will own durable append
  es-projection -> will own projectors
  es-outbox -> will own dispatcher/publisher contracts
  adapter-http / adapter-grpc -> will decode requests only
  app -> will compose runtime/storage/adapters
```

This diagram reflects Phase 1 responsibility only; no runtime, database, broker, or adapter dependency should point back into `es-core` or `es-kernel`. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md]

### Recommended Project Structure

```text
Cargo.toml
crates/
  es-core/
    Cargo.toml
    src/lib.rs
  es-kernel/
    Cargo.toml
    src/lib.rs
  es-runtime/
    Cargo.toml
    src/lib.rs
  es-store-postgres/
    Cargo.toml
    src/lib.rs
  es-projection/
    Cargo.toml
    src/lib.rs
  es-outbox/
    Cargo.toml
    src/lib.rs
  example-commerce/
    Cargo.toml
    src/lib.rs
  adapter-http/
    Cargo.toml
    src/lib.rs
  adapter-grpc/
    Cargo.toml
    src/lib.rs
  app/
    Cargo.toml
    src/main.rs
tests/
  dependency_boundaries.rs
```

Use a virtual root manifest because this template has no root package and all runnable code should live in explicit member crates. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html]

### Pattern 1: Workspace Inheritance and Boundary Visibility

**What:** Put edition, MSRV, license, common dependencies, and lints in the root workspace manifest; make each member opt into package metadata and dependencies explicitly. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html]

**When to use:** Use immediately in Phase 1 so future phases cannot accidentally normalize adapter/storage dependencies inside the deterministic domain crates. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md]

**Example:**

```toml
# Source: Cargo Book workspace docs
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
uuid = { version = "1.23.0", features = ["serde", "v7"] }
time = { version = "0.3.47", features = ["serde", "formatting", "parsing"] }
thiserror = "2.0.18"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"
```

### Pattern 2: Typed Core Newtypes

**What:** Define stream IDs, partition keys, revisions, expected revisions, command metadata, and event metadata as first-class types instead of aliases over primitives. [VERIFIED: CORE-03 in .planning/REQUIREMENTS.md]

**When to use:** Use for every cross-crate contract in Phase 1; adapters and storage can add encoding later without changing domain semantics. [VERIFIED: .planning/ROADMAP.md]

**Example:**

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

### Pattern 3: Synchronous Aggregate Kernel

**What:** Use associated types for state, command, event, reply, and error; keep `decide` and `apply` synchronous and free of `async`, `tokio`, database handles, and network types. [VERIFIED: CORE-02/CORE-04 in .planning/REQUIREMENTS.md]

**When to use:** Use for all domain aggregates and for the example commerce fixture. [VERIFIED: .planning/ROADMAP.md]

**Example:**

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

### Anti-Patterns to Avoid

- **Core/kernel depending on runtime/storage/adapters:** This violates CORE-04 and prevents deterministic domain tests from staying cheap. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md]
- **`async fn decide`:** Async domain decisions invite hidden I/O and runtime coupling; keep async at adapter/storage/runtime boundaries in later phases. [VERIFIED: .planning/research/STACK.md]
- **`serde_json::Value` as command/event domain model:** JSON/reflection in the hot path undermines typed Rust contracts and compiler-checked replay. [VERIFIED: .planning/research/PITFALLS.md]
- **Global `Arc<Mutex<_>>` business state in examples:** Even a Phase 1 fixture should model state as explicit aggregate state, not shared runtime state. [VERIFIED: .planning/research/PITFALLS.md]
- **Workspace members with implicit dependency sprawl:** Every crate should declare only dependencies it uses, inherited from `[workspace.dependencies]` where appropriate. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rust workspace orchestration | Custom scripts for crate discovery/build order | Cargo workspaces | Cargo already provides members, default members, shared lockfile, shared target dir, resolver, and workspace inheritance. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| Serialization derives | Manual serializers for every contract type | `serde` derives | Serde derives implementations for structs/enums and avoids repetitive encoding boilerplate. [CITED: https://serde.rs/derive.html] |
| Domain error boilerplate | Manual `Display`/`Error` implementations for every enum | `thiserror` | `thiserror` provides `derive(Error)` for typed error enums. [VERIFIED: cargo info thiserror] |
| UUID generation/parsing | Custom ID generation | `uuid` with `v7` and `serde` features | The crate provides UUID versions/features and ecosystem serialization support. [VERIFIED: cargo info uuid@1.23.0] |
| Property-test engine | Custom random command-sequence runner | `proptest` | The crate provides property-based generation and shrinking. [VERIFIED: cargo info proptest@1.11.0] |
| Snapshot approval tooling | Ad hoc golden-file comparison | `insta` | The crate is a Rust snapshot testing library with redaction support. [VERIFIED: cargo info insta] |
| Dependency policy checks | One-off grep over `Cargo.lock` | `cargo-deny` | The crate is a Cargo plugin for managing dependency graphs; install after Rust toolchain upgrade. [VERIFIED: cargo search cargo-deny] |

**Key insight:** Hand-rolling the kernel contract is appropriate because it is the product architecture; hand-rolling workspace management, derives, IDs, test shrinking, or dependency policy is not. [VERIFIED: .planning/research/STACK.md]

## Common Pitfalls

### Pitfall 1: Letting Future Crates Leak Backward

**What goes wrong:** `es-kernel` gains `tokio`, `sqlx`, `axum`, `tonic`, or broker dependencies because later phases need them. [VERIFIED: .planning/research/STACK.md]
**Why it happens:** Workspace-wide dependencies feel convenient and future crates are present from Phase 1. [ASSUMED]
**How to avoid:** Keep root `[workspace.dependencies]` as a version catalog only; each member must opt into dependencies explicitly. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html]
**Warning signs:** `cargo tree -p es-kernel` shows async runtime, storage, HTTP, gRPC, or broker crates. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md]

### Pitfall 2: Trait Too Generic to Be Useful

**What goes wrong:** A generic kernel erases command/event/reply/error types into dynamic payloads and loses compiler help. [VERIFIED: .planning/research/PITFALLS.md]
**Why it happens:** Infrastructure genericity is mistaken for domain genericity. [ASSUMED]
**How to avoid:** Use associated types and concrete domain enums for each aggregate; keep only the runner contracts generic. [VERIFIED: CORE-02 in .planning/REQUIREMENTS.md]
**Warning signs:** Domain events are `serde_json::Value`, `Box<dyn Any>`, or stringly typed event names inside `decide`. [VERIFIED: .planning/research/PITFALLS.md]

### Pitfall 3: Non-Deterministic Decision Logic

**What goes wrong:** `decide` reads clocks, random IDs, environment variables, databases, network services, or global state. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md]
**Why it happens:** Metadata creation and business decisions are not separated. [ASSUMED]
**How to avoid:** Generate IDs/timestamps at command envelope boundaries and pass them through metadata; `decide` should use only command, state, and metadata inputs. [VERIFIED: CORE-03/CORE-04 in .planning/REQUIREMENTS.md]
**Warning signs:** `std::time::SystemTime::now`, `uuid::Uuid::now_v7`, `tokio`, `sqlx`, or HTTP clients appear inside domain crates. [ASSUMED]

### Pitfall 4: Rust 2024 Toolchain Mismatch

**What goes wrong:** The workspace declares edition 2024 or depends on edition-2024 crates, but the local toolchain cannot parse them. [VERIFIED: cargo info time@0.3.47/cargo-deny@0.19.4/cargo-nextest@0.9.133 failures on Cargo 1.82]
**Why it happens:** Rust 2024 stabilized in Rust 1.85.0, while this machine currently has Rust/Cargo 1.82.0-nightly. [CITED: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/] [VERIFIED: `rustc --version`]
**How to avoid:** Make toolchain upgrade Wave 0 before build tasks; add `rust-toolchain.toml` only if the project wants a pinned toolchain. [ASSUMED]
**Warning signs:** Cargo reports `feature edition2024 is required`. [VERIFIED: local cargo info failures]

### Pitfall 5: Tests Only Prove Compilation

**What goes wrong:** Phase 1 compiles but does not prove replay determinism or crate boundary rules. [VERIFIED: CORE-02/CORE-04 in .planning/REQUIREMENTS.md]
**Why it happens:** Kernel contracts look abstract until an example aggregate uses them. [ASSUMED]
**How to avoid:** Include a minimal example aggregate plus tests for `decide`, `apply`, replay equivalence, and `cargo tree` dependency absence. [VERIFIED: .planning/ROADMAP.md]
**Warning signs:** No test calls `Aggregate::decide` and no test/CI command checks dependency boundaries. [ASSUMED]

## Code Examples

### Root Workspace Manifest

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

### Kernel Crate Manifest

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

### Minimal Aggregate Fixture

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

### Boundary Verification Command

```bash
# Source: Cargo package selection/workspace commands docs.
cargo tree -p es-kernel
cargo tree -p es-core
cargo check --workspace
cargo test --workspace
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Rust 2021 workspace defaults | Rust 2024 with resolver 3 and Rust-version-aware resolution | Rust 1.85.0 stabilized Rust 2024 on 2025-02-20. [CITED: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/] | Phase 1 should target Rust 1.85+ and not pretend Cargo 1.82 can build the workspace. [VERIFIED: local toolchain check] |
| Repeating metadata/dependency versions in each crate | Root `[workspace.package]` and `[workspace.dependencies]` inheritance | Cargo supports workspace package/dependency inheritance. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] | Keeps crate manifests small while still requiring each crate to opt into dependencies. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| Crate-local lint config only | `[workspace.lints]` plus member `[lints] workspace = true` | Cargo documents workspace lint inheritance. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] | Lets the template enforce unsafe and documentation policy consistently. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| End-to-end examples only | Typed kernel plus property/snapshot contract tests | `proptest` and `insta` current metadata confirms available Rust test tooling. [VERIFIED: cargo info proptest@1.11.0; cargo info insta] | Phase 1 can prove contract determinism before storage/runtime exists. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md] |

**Deprecated/outdated:**

- Building Phase 1 on local Rust/Cargo 1.82 is outdated for this project because Rust 2024 requires 1.85.0 and several latest support tools use edition 2024 manifests. [CITED: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/] [VERIFIED: local cargo failures]
- Using one crate with feature flags for core/runtime/storage/adapters is the wrong default for this template because CORE-01 requires separate crates and CORE-04 requires inspectable lower-level boundaries. [VERIFIED: .planning/REQUIREMENTS.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Newtype wrappers are better than raw primitives for IDs/revisions. | Standard Stack alternatives | If wrong, implementation may add extra boilerplate without enough safety benefit. |
| A2 | `insta` redaction should be used for unstable IDs/timestamps. | Supporting stack | If wrong, snapshot tests may be too complex or brittle. |
| A3 | Workspace dependency sprawl usually happens from convenience. | Common Pitfalls | If wrong, the prevention still helps but may miss a stronger root cause. |
| A4 | Metadata generation should happen at command envelope boundaries, not inside `decide`. | Common Pitfalls | If wrong, deterministic replay may need a different source-of-time/ID strategy. |
| A5 | A pinned `rust-toolchain.toml` should be added only if the project wants pinning. | Common Pitfalls | If wrong, planners may under-specify toolchain setup. |
| A6 | Boundary tests should include `cargo tree` absence checks. | Common Pitfalls | If wrong, planner may choose a different dependency-boundary enforcement mechanism. |

## Open Questions

1. **Should Phase 1 pin a toolchain with `rust-toolchain.toml`?**
   - What we know: Rust 2024 requires Rust 1.85.0, and the local toolchain is Rust/Cargo 1.82.0-nightly. [CITED: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/] [VERIFIED: `rustc --version`]
   - What's unclear: Whether the project prefers a pinned channel/version or a documented minimum only. [ASSUMED]
   - Recommendation: Add a Wave 0 task to upgrade or pin Rust 1.85+ before creating the workspace. [ASSUMED]
2. **Should `StreamId`/`PartitionKey` be string-backed or structured enum-backed in Phase 1?**
   - What we know: CORE-03 requires reusable stream ID and partition key types. [VERIFIED: .planning/REQUIREMENTS.md]
   - What's unclear: Whether the template wants opaque strings for flexibility or structured fields for stricter routing. [ASSUMED]
   - Recommendation: Start with opaque validated newtypes and add constructors from aggregate type/id so storage/runtime can evolve without breaking domain code. [ASSUMED]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | Rust 2024 workspace | No for target MSRV | `rustc 1.82.0-nightly` locally; target is 1.85+ | Upgrade via `rustup update stable` or install/pin Rust 1.85+. [VERIFIED: `rustc --version`; cited Rust 1.85 blog] |
| Cargo | Workspace build/test | No for target MSRV | `cargo 1.82.0-nightly` locally; target is 1.85+ | Upgrade with Rust toolchain. [VERIFIED: `cargo --version`; local edition2024 parse failures] |
| crates.io access | Version verification | Yes | Able to run `cargo search` and `cargo info` | None needed. [VERIFIED: cargo command output] |
| `cargo-nextest` | Optional faster tests | Not found | `cargo-nextest` latest 0.9.133 from search | Use `cargo test --workspace` until installed after toolchain upgrade. [VERIFIED: command lookup; cargo search cargo-nextest] |
| `cargo-deny` | Optional dependency policy | Not found | `cargo-deny` latest 0.19.4 from search | Add config now or defer CLI install until toolchain upgrade. [VERIFIED: command lookup; cargo search cargo-deny] |
| `cargo-llvm-cov` | Optional coverage | Not found | 0.8.5 requires Rust 1.87 | Do not gate Phase 1 on coverage CLI; use tests first. [VERIFIED: cargo info cargo-llvm-cov@0.8.5] |

**Missing dependencies with no fallback:**

- Rust/Cargo 1.85+ is required to build a Rust 2024 workspace and latest Phase 1 crate versions. [CITED: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/] [VERIFIED: local Cargo 1.82 failures]

**Missing dependencies with fallback:**

- `cargo-nextest`, `cargo-deny`, and `cargo-llvm-cov` can be deferred or replaced by `cargo test --workspace` and manual review in Phase 1. [VERIFIED: local command lookup]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness via `cargo test`; optional `proptest` 1.11.0 and `insta` 1.47.2 for richer contract tests. [VERIFIED: cargo info proptest@1.11.0; cargo info insta] |
| Config file | None yet; Wave 0 should create workspace `Cargo.toml` and crate manifests. [VERIFIED: `rg --files` found no Cargo.toml] |
| Quick run command | `cargo test -p es-core -p es-kernel -p example-commerce` after workspace creation. [ASSUMED] |
| Full suite command | `cargo test --workspace && cargo tree -p es-core && cargo tree -p es-kernel` after workspace creation. [ASSUMED] |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| CORE-01 | Workspace builds all required crates. [VERIFIED: .planning/REQUIREMENTS.md] | build/smoke | `cargo check --workspace` | No, Wave 0. [VERIFIED: `rg --files`] |
| CORE-02 | Example aggregate implements typed commands/events/state/replies/errors through kernel trait. [VERIFIED: .planning/REQUIREMENTS.md] | unit | `cargo test -p example-commerce aggregate_contract` | No, Wave 0. [VERIFIED: `rg --files`] |
| CORE-03 | Core types cover stream IDs, partition keys, expected revisions, command metadata, and event metadata. [VERIFIED: .planning/REQUIREMENTS.md] | unit/snapshot | `cargo test -p es-core metadata_contracts` | No, Wave 0. [VERIFIED: `rg --files`] |
| CORE-04 | Domain decision logic has no adapter/database/broker/network/runtime dependencies. [VERIFIED: .planning/REQUIREMENTS.md] | dependency boundary | `cargo tree -p es-kernel` plus absence checks for forbidden crates | No, Wave 0. [VERIFIED: `rg --files`] |

### Sampling Rate

- **Per task commit:** `cargo test -p es-core -p es-kernel` once crates exist. [ASSUMED]
- **Per wave merge:** `cargo check --workspace && cargo test --workspace`. [ASSUMED]
- **Phase gate:** Full workspace tests pass and `cargo tree -p es-core/-p es-kernel` shows no forbidden dependencies. [VERIFIED: CORE-04 in .planning/REQUIREMENTS.md]

### Wave 0 Gaps

- [ ] `Cargo.toml` root workspace manifest. [VERIFIED: no Cargo.toml exists]
- [ ] `crates/es-core` and `crates/es-kernel` manifests/source. [VERIFIED: no Cargo.toml exists]
- [ ] Required boundary placeholder crates for runtime/storage/projection/outbox/example/adapters/app. [VERIFIED: CORE-01 in .planning/REQUIREMENTS.md]
- [ ] Rust/Cargo toolchain upgrade to 1.85+. [VERIFIED: local `rustc --version`]
- [ ] Contract tests for core metadata, aggregate trait, replay determinism, and dependency boundaries. [VERIFIED: CORE-02/CORE-03/CORE-04 in .planning/REQUIREMENTS.md]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | No for Phase 1 | No adapter/auth boundary exists in Phase 1. [VERIFIED: .planning/ROADMAP.md] |
| V3 Session Management | No for Phase 1 | No sessions exist in Phase 1. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | No for Phase 1 | No request authorization boundary exists in Phase 1. [VERIFIED: .planning/ROADMAP.md] |
| V5 Input Validation | Yes | Validate constructor invariants for stream IDs, partition keys, revisions, and metadata types; avoid accepting invalid state into core contracts. [VERIFIED: CORE-03 in .planning/REQUIREMENTS.md] |
| V6 Cryptography | No for Phase 1 | UUID generation is identity/metadata support, not custom cryptography. [VERIFIED: CORE-03 in .planning/REQUIREMENTS.md] |

### Known Threat Patterns for Rust Workspace Contracts

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Dependency confusion or policy drift | Tampering | Use explicit workspace dependencies and add `cargo-deny` config once toolchain supports it. [VERIFIED: cargo search cargo-deny; Cargo workspace docs] |
| Accidental unsafe code in core/kernel | Elevation of Privilege | Set `unsafe_code = "forbid"` in `[workspace.lints.rust]` and inherit lints in member crates. [CITED: https://doc.rust-lang.org/cargo/reference/workspaces.html] |
| Log/serialization leakage from metadata | Information Disclosure | Keep metadata types explicit and avoid embedding arbitrary request payloads or secrets in core metadata. [ASSUMED] |

## Sources

### Primary (HIGH confidence)

- Cargo Book workspace docs - virtual workspaces, resolver, members, workspace package/dependency/lint inheritance. https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo Book manifest docs - edition and rust-version fields. https://doc.rust-lang.org/cargo/reference/manifest.html
- Rust 1.85.0 release blog - Rust 2024 stabilization. https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/
- Rust Edition Guide - Rust 2024 release version 1.85.0. https://doc.rust-lang.org/edition-guide/rust-2024/index.html
- Serde derive docs - derive-based Serialize/Deserialize. https://serde.rs/derive.html
- Local `cargo search` / `cargo info` - `serde`, `serde_json`, `uuid`, `time`, `thiserror`, `proptest`, `insta`, `cargo-deny`, `cargo-nextest`, `cargo-llvm-cov`.
- Project planning docs - `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/STATE.md`, `.planning/research/STACK.md`, `.planning/research/PITFALLS.md`.

### Secondary (MEDIUM confidence)

- Existing project research synthesis in `.planning/research/ARCHITECTURE.md` and `.planning/research/SUMMARY.md`. [VERIFIED: local file reads]

### Tertiary (LOW confidence)

- No web-search-only sources were used. [VERIFIED: research log]

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH - Cargo/Rust behavior and crate versions were verified through official docs and local crate metadata. [CITED: Cargo/Rust docs] [VERIFIED: cargo commands]
- Architecture: HIGH - Phase responsibilities come directly from CORE-01 through CORE-04 and existing architecture research. [VERIFIED: .planning/REQUIREMENTS.md]
- Pitfalls: MEDIUM - Boundary and determinism risks are strongly supported by project docs, but exact enforcement shape needs implementation feedback. [VERIFIED: .planning/research/PITFALLS.md] [ASSUMED]

**Research date:** 2026-04-16 [VERIFIED: local context]
**Valid until:** 2026-05-16 for workspace/kernel architecture; recheck crate versions immediately before implementation because Rust crate versions are moving and local toolchain is stale. [ASSUMED]

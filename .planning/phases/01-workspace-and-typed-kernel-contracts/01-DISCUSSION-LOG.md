# Phase 01: Workspace and Typed Kernel Contracts - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-04-16
**Phase:** 01 - Workspace and Typed Kernel Contracts
**Areas discussed:** Toolchain and workspace policy, Crate boundary strictness, Core ID model, Aggregate trait shape, Contract verification level, Tenant identity, Decision reply shape

---

## Toolchain and Workspace Policy

| Option | Description | Selected |
|--------|-------------|----------|
| `rust-toolchain.toml` pin | Pin Rust 1.85+ for reproducible Rust 2024 compatibility. | yes |
| `rust-version` only | Set MSRV in Cargo metadata but do not force local toolchain selection. | |
| Documentation only | Mention toolchain expectations without machine enforcement. | |

**User's choice:** `rust-toolchain.toml` pin.
**Notes:** User accepted the recommended path because Phase 1 should quickly stabilize the kernel contract and avoid environment-dependent failures.

---

## Crate Boundary Strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Full crate shell | Create all planned crate boundaries in Phase 1. | yes |
| Core/kernel/example only | Create only the crates needed for immediate typed kernel validation. | |
| Core/kernel only | Keep the workspace minimal and defer example/runtime boundary crates. | |

**User's choice:** Full crate shell.
**Notes:** The project needs strict dependency boundaries early so runtime, storage, broker, and adapter concerns do not leak into deterministic kernel crates.

---

## Core ID Model

| Option | Description | Selected |
|--------|-------------|----------|
| Opaque newtypes first | Use `StreamId(String)`, `PartitionKey(String)`, revisions, and explicit metadata. | yes |
| Structured typed keys | Encode aggregate type/name more deeply into the type system from the start. | |
| Hybrid | Use opaque public types plus aggregate-specific constructors. | |

**User's choice:** Opaque newtypes first.
**Notes:** User emphasized that stream identity and partition identity must not remain ambiguous because shard routing depends on them.

---

## Aggregate Trait Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Single aggregate trait | Include routing, expected revision, `decide`, and `apply` in one associated-type trait. | yes |
| Split traits | Separate routing and revision policy into additional traits. | |
| Minimal decide/apply only | Defer routing and revision derivation to later phases. | |

**User's choice:** Single aggregate trait.
**Notes:** User emphasized sync deterministic decision logic, no async, no side effects, no database access, no repository pattern in the kernel, and no dynamic/plugin abstraction in Phase 1.

---

## Contract Verification Level

| Option | Description | Selected |
|--------|-------------|----------|
| Strong verification | Build/test, dependency boundary checks, replay/property-style tests, and stable fixtures. | yes |
| Moderate verification | Compile, unit tests, and dependency boundary checks only. | |
| Minimal verification | Workspace builds and basic tests only. | |

**User's choice:** Strong verification.
**Notes:** Strong verification is necessary because later disruptor/runtime and stress-test results are only meaningful if the kernel remains deterministic and replayable.

---

## Tenant Identity

| Option | Description | Selected |
|--------|-------------|----------|
| `TenantId(String)` required | Required tenant identity with storage-agnostic string newtype. | yes |
| `Option<TenantId>` | Optional tenant for simpler single-tenant examples. | |
| `TenantId(Uuid)` | UUID-only tenant identity. | |

**User's choice:** `TenantId(String)` required.
**Notes:** This matches the user's metadata sketch while avoiding premature UUID-only assumptions for tenant naming.

---

## Decision Reply Shape

| Option | Description | Selected |
|--------|-------------|----------|
| `Decision { events, reply }` | Return typed events and typed reply from `decide`. | yes |
| `Vec<Event>` only | Keep the decision API to raw events and defer replies. | |
| Hybrid helper reply | Use events with optional helper reply outside the core decision type. | |

**User's choice:** `Decision { events, reply }`.
**Notes:** This preserves the roadmap requirement for typed replies while keeping command success tied to durable append in later phases.

---

## the agent's Discretion

- Exact module names and file organization inside crates.
- Exact adapter crate naming convention, as long as boundaries remain clear.
- Minimal example aggregate details needed to prove the Phase 1 kernel contract without pulling Phase 4 commerce scope forward.

## Deferred Ideas

None.

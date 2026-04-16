---
phase: 01
slug: workspace-and-typed-kernel-contracts
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-16
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness via `cargo test`; `proptest` for generated replay checks |
| **Config file** | `Cargo.toml`, `rust-toolchain.toml`, `deny.toml` |
| **Quick run command** | `cargo test -p es-core -p es-kernel -p example-commerce` |
| **Final gate smoke command** | `cargo test -p example-commerce aggregate_contract && cargo test -p example-commerce --test dependency_boundaries` |
| **Full suite command** | `cargo check --workspace && cargo test --workspace && cargo tree -p es-core && cargo tree -p es-kernel` |
| **Estimated runtime** | Smoke command should stay under focused feedback latency after crates exist; full suite is final-only and may take ~60 seconds after dependencies are cached |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p es-core -p es-kernel -p example-commerce` once those crates exist; before then run the task's grep/file verification command.
- **After every plan wave:** Run `cargo check --workspace && cargo test --workspace` once all workspace crates exist.
- **Before `$gsd-verify-work`:** Run the final gate smoke command first, then the full suite must be green.
- **Max feedback latency:** < 60 seconds for focused crate tests.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | CORE-01, CORE-04 | T-01-01 / T-01-02 | Workspace pins Rust 2024 and forbids unsafe code without runtime/storage dependencies | config | `rustup toolchain install 1.85 --profile minimal --component rustfmt --component clippy && rustc +1.85 --version && cargo +1.85 --version && grep -E 'resolver = "3"\|edition = "2024"\|rust-version = "1.85"\|unsafe_code = "forbid"' Cargo.toml` | W1 | pending |
| 01-01-02 | 01 | 1 | CORE-04 | T-01-01 | Supply-chain policy denies unknown registry/git sources | config | `test -f deny.toml && grep -E '\[advisories\]\|\[licenses\]\|unknown-registry = "deny"\|multiple-versions = "warn"' deny.toml` | W1 | pending |
| 01-01-03 | 01 | 1 | CORE-01, CORE-04 | T-01-03 | Validation commands are actionable and automated | docs | `grep -E 'nyquist_compliant: true\|cargo test -p es-core -p es-kernel -p example-commerce\|cargo check --workspace && cargo test --workspace\|All phase behaviors have automated verification\.' .planning/phases/01-workspace-and-typed-kernel-contracts/01-VALIDATION.md` | W1 | pending |
| 01-02-01 | 02 | 2 | CORE-03, CORE-04 | T-02-01 / T-02-02 | Core IDs and metadata reject invalid empty identifiers and avoid arbitrary payloads | unit | `cargo test -p es-core metadata_contracts` | W2 | pending |
| 01-02-02 | 02 | 2 | CORE-02, CORE-04 | T-02-03 | Aggregate trait is synchronous, typed, and replayable | unit | `cargo test -p es-kernel aggregate_kernel_contracts` | W2 | pending |
| 01-03-01 | 03 | 2 | CORE-01, CORE-04 | T-03-01 / T-03-03 | Runtime/storage/projection/outbox crates are boundary-only placeholders | config | `test -f crates/es-runtime/src/lib.rs && test -f crates/es-store-postgres/src/lib.rs && test -f crates/es-projection/src/lib.rs && test -f crates/es-outbox/src/lib.rs && grep -R 'PHASE_BOUNDARY' crates/es-runtime/src crates/es-store-postgres/src crates/es-projection/src crates/es-outbox/src && for manifest in crates/es-runtime/Cargo.toml crates/es-store-postgres/Cargo.toml crates/es-projection/Cargo.toml crates/es-outbox/Cargo.toml; do awk '/^\[dependencies\]/{in_deps=1; next} /^\[/{in_deps=0} in_deps && $0 !~ /^[[:space:]]*(#.*)?$/ { print FILENAME ":" $0; found=1 } END { exit found }' "$manifest" || exit 1; done` | W2 | pending |
| 01-03-02 | 03 | 2 | CORE-01, CORE-04 | T-03-02 | Adapter/app crates exist without network runtime dependencies | config | `test -f crates/adapter-http/src/lib.rs && test -f crates/adapter-grpc/src/lib.rs && test -f crates/app/src/main.rs && grep -E 'name = "adapter-http"\|name = "adapter-grpc"\|name = "app"' crates/adapter-http/Cargo.toml crates/adapter-grpc/Cargo.toml crates/app/Cargo.toml` | W2 | pending |
| 01-04-01 | 04 | 3 | CORE-02, CORE-03, CORE-04 | T-04-01 | Example aggregate proves typed deterministic decisions and replay | unit/property | `cargo test -p example-commerce aggregate_contract` | W3 | pending |
| 01-04-02 | 04 | 3 | CORE-01, CORE-04 | T-04-02 / T-04-03 | Dependency boundary tests block forbidden core/kernel dependencies | integration | `cargo test -p example-commerce --test dependency_boundaries` | W3 | pending |
| 01-04-03 | 04 | 3 | CORE-01, CORE-02, CORE-03, CORE-04 | T-04-03 | Full workspace builds and dependency boundaries are inspectable | full | `cargo test -p example-commerce aggregate_contract && cargo test -p example-commerce --test dependency_boundaries && cargo check --workspace && cargo test --workspace && ! cargo tree -p es-core | grep -E 'tokio|sqlx|axum|tonic|async-nats|rdkafka|postgres|disruptor' && ! cargo tree -p es-kernel | grep -E 'tokio|sqlx|axum|tonic|async-nats|rdkafka|postgres|disruptor'` | W3 | pending |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

- [x] `01-01-PLAN.md` creates root validation policy before code-producing tasks.
- [x] `01-02-PLAN.md` creates crate-local unit tests for core/kernel contracts.
- [x] `01-04-PLAN.md` creates `crates/example-commerce/tests/dependency_boundaries.rs` before the full phase gate.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|

All phase behaviors have automated verification.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 60s for focused tests after crates exist
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-04-16

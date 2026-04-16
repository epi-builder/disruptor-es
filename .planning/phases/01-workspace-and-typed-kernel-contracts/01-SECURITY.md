---
phase: 01
slug: workspace-and-typed-kernel-contracts
status: verified
threats_open: 0
asvs_level: 1
created: 2026-04-16
---

# Phase 01 - Security

Per-phase security contract for the workspace and typed kernel contracts phase.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| developer machine -> Cargo registry | Workspace resolves third-party crates from configured registries and package sources. | Package metadata, crate source, dependency graph |
| workspace policy -> member crates | Root dependency and lint rules constrain member crates through workspace inheritance. | Build policy, dependency versions, lint settings |
| domain author -> core constructors | Stream, partition, and tenant identity strings enter typed core contracts. | Business identifiers, tenant identifiers |
| kernel crate -> future runtime/storage/adapters | Deterministic aggregate code is consumed by later runtime, storage, projection, and adapter crates. | Commands, events, state transitions, metadata |
| future adapters -> domain/runtime | Placeholder adapter crates mark later protocol boundaries but do not expose network surfaces in Phase 01. | None in Phase 01 |
| future storage/outbox crates -> deterministic kernel | Placeholder storage and outbox crates must not be pulled into core or kernel dependencies. | None in Phase 01 |
| example domain author -> kernel contract | Example domain code exercises aggregate contracts and replay semantics. | Typed commands, events, replies, errors |
| integration test -> Cargo process | Boundary tests shell out to Cargo and inspect local workspace dependency output. | Local dependency tree output |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-01-01 | Tampering | `Cargo.toml`, `deny.toml` | mitigate | Workspace uses resolver 3, explicit workspace dependency versions, and `deny.toml` denies unknown registries and git sources. | closed |
| T-01-02 | Elevation of Privilege | Workspace Rust lints | mitigate | Root workspace sets `unsafe_code = "forbid"` and member manifests inherit workspace lints. | closed |
| T-01-03 | Information Disclosure | Metadata dependency catalog | accept | Phase 01 contains no runtime secrets or user payloads; accepted as `AR-01-03` until later metadata-bearing adapters/storage are implemented. | closed |
| T-02-01 | Tampering | `StreamId`, `PartitionKey`, `TenantId` | mitigate | Constructors reject empty strings with `CoreError::EmptyValue`; metadata contract tests passed. | closed |
| T-02-02 | Information Disclosure | `CommandMetadata`, `EventMetadata` | mitigate | Metadata structs include explicit IDs, tenant, causation/correlation, and timestamps only; no arbitrary payload or `serde_json::Value` in core source. | closed |
| T-02-03 | Elevation of Privilege | `es-kernel` dependency surface | mitigate | `es-kernel` depends only on `es-core`; Cargo tree checks found no forbidden runtime, storage, adapter, broker, or disruptor crates. | closed |
| T-03-01 | Tampering | Boundary crates | mitigate | Runtime, storage, projection, and outbox crates are dependency-empty shells with `PHASE_BOUNDARY` ownership markers only. | closed |
| T-03-02 | Elevation of Privilege | Adapter boundary shells | mitigate | Adapter and app manifests have empty dependencies; no Axum, Tonic, Tokio, or network surfaces exist in Phase 01. | closed |
| T-03-03 | Tampering | Workspace dependency graph | mitigate | Boundary manifests inherit workspace lints; dependency boundary tests protect core and kernel from forbidden dependency leakage. | closed |
| T-04-01 | Tampering | `example-commerce` aggregate | mitigate | Aggregate tests cover typed decisions, typed errors, duplicate rejection, and replay equivalence without global mutable state or I/O. | closed |
| T-04-02 | Repudiation | Dependency boundary verification | mitigate | Integration tests inspect Cargo package names and fail with explicit forbidden dependency messages. | closed |
| T-04-03 | Elevation of Privilege | Core/kernel dependency graph | mitigate | Full phase gate passed `cargo check --workspace`, `cargo test --workspace`, and Cargo tree checks for `es-core` and `es-kernel`. | closed |

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-01-03 | T-01-03 | Phase 01 has no runtime secrets, user records, request payloads, storage rows, adapter input, or external publication. The metadata catalog exposure risk is acceptable until later phases introduce runtime data boundaries. | gsd-security-auditor | 2026-04-16 |

Accepted risks do not resurface in future audit runs unless the corresponding boundary changes.

---

## Security Audit 2026-04-16

| Metric | Count |
|--------|-------|
| Threats found | 12 |
| Closed | 12 |
| Open | 0 |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-16 | 12 | 12 | 0 | gsd-security-auditor |

## Verification Evidence

- No `## Threat Flags` sections were present in the Phase 01 summary files.
- `cargo test -p es-core metadata_contracts`
- `cargo test -p es-kernel aggregate_kernel_contracts`
- `cargo test -p example-commerce aggregate_contract`
- `cargo test -p example-commerce --test dependency_boundaries`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo tree -p es-core --prefix none`
- `cargo tree -p es-kernel --prefix none`
- Targeted source checks for forbidden dependency names, `unsafe_code = "forbid"`, `PHASE_BOUNDARY`, constructor validation, and boundary-test markers.

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-16

---
phase: 05
slug: cqrs-projection-and-query-catch-up
status: verified
threats_open: 0
asvs_level: 1
created: 2026-04-18
verified: 2026-04-18T05:12:47Z
---

# Phase 05 - Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| query caller -> projection API | Caller supplies projector names, minimum global positions, batch limits, and wait-policy values. | Tenant-scoped query freshness controls; non-secret identifiers and timing bounds. |
| projection API -> storage implementation | Validated projection contracts are passed to PostgreSQL-backed projection storage. | Projector names, tenant IDs, global positions, batch limits, and read-model IDs. |
| stored JSON payload -> commerce event type | PostgreSQL JSONB payload is decoded into typed commerce event enums. | Event payload JSON from committed events; commerce fixture IDs, SKU/name, quantities, and rejection reason. |
| projection code -> domain crate | Projection reads event DTO shapes but must not mutate aggregate state or add async/storage dependencies to domain logic. | Typed commerce event DTOs and value objects. |
| committed events table -> projector | Stored event rows and JSON payloads are untrusted until decoded and schema-checked. | Committed event type, schema version, tenant ID, metadata, and payload. |
| query caller -> read-model repository | Caller supplies tenant ID, row IDs, optional minimum global position, and wait policy. | Tenant-scoped order/product query keys and freshness requirements. |
| projection repository -> PostgreSQL | SQL statements write read models and offsets inside durable transactions. | Bound SQL parameters for tenant IDs, projector names, row IDs, positions, statuses, and quantities. |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-05-01 | Information Disclosure | `ProjectorOffset` / `MinimumGlobalPosition` contracts | mitigate | `ProjectorOffset` carries `TenantId`; PostgreSQL offsets are keyed by `(tenant_id, projector_name)` and offset lookups bind both values. | closed |
| T-05-02 | Denial of Service | `wait_for_minimum_position` | mitigate | `WaitPolicy::new` rejects zero poll intervals and invalid intervals; `wait_for_minimum_position` uses a Tokio deadline and returns `ProjectionError::ProjectionLag`. | closed |
| T-05-03 | Tampering | cursor and limit constructors | mitigate | `MinimumGlobalPosition::new`, `ProjectorOffset::new`, and `ProjectionBatchLimit::new` reject negative positions and invalid limits before storage calls. | closed |
| T-05-04 | Denial of Service | malformed event payload path | mitigate | `ProjectionError::PayloadDecode` exists; handled event decode failures abort catch-up and rollback the transaction before offset advancement. | closed |
| T-05-05 | Tampering | future SQL use of projection values | mitigate | `PostgresProjectionStore` uses static SQL with SQLx `.bind(...)` for projection values; no dynamic SQL formatting is used in the projection store. | closed |
| T-05-06 | Denial of Service | `serde_json::from_value::<OrderEvent/ProductEvent>` consumers | mitigate | Commerce payload round-trip tests cover expected shapes; projection decode failures map to `PayloadDecode` and do not advance offsets. | closed |
| T-05-07 | Information Disclosure | commerce event payloads | accept | Accepted as fixture-domain exposure: payloads contain commerce IDs, SKU/name, quantities, and rejection reason; no secret fields are introduced by Phase 05. | closed |
| T-05-08 | Elevation of Privilege | domain crate dependency boundary | mitigate | `example-commerce` depends on `es-core`, `es-kernel`, `serde`, and `thiserror` only; storage/runtime/adapter dependencies were not added. | closed |
| T-05-09 | Tampering | event payload JSON shape | mitigate | Commerce events are closed Rust enums/value objects with serde derives; projection code decodes exact `OrderEvent` and `ProductEvent` variants instead of arbitrary maps. | closed |
| T-05-10 | Information Disclosure | read-model queries and offsets | mitigate | Migration primary keys and query predicates include `tenant_id`; integration coverage verifies same IDs across different tenants remain isolated. | closed |
| T-05-11 | Tampering | projection SQL | mitigate | Projection SQL is static and parameterized with `.bind(...)`; tenant IDs, projector names, row IDs, positions, statuses, and quantities are not interpolated. | closed |
| T-05-12 | Repudiation | checkpoint correctness | mitigate | `catch_up` applies read-model writes and `projector_offsets` upsert in one SQLx transaction, commits after both succeed, and uses monotonic offset upsert. | closed |
| T-05-13 | Denial of Service | malformed payload catch-up | mitigate | `projections_malformed_payload_does_not_advance_offset` verifies malformed handled events return `PayloadDecode` and leave offsets absent/unchanged. | closed |
| T-05-14 | Denial of Service | read-your-own-write query wait | mitigate | Query methods delegate to `wait_for_minimum_position`; lagging reads return `ProjectionLag` after `WaitPolicy.timeout`. | closed |
| T-05-15 | Tampering | invalid cursor/limit inputs | mitigate | `catch_up` accepts `ProjectionBatchLimit`, queries accept `MinimumGlobalPosition`, and constructor tests cover invalid values. | closed |

*Status: open | closed*
*Disposition: mitigate (implementation required) | accept (documented risk) | transfer (third-party)*

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-05-01 | T-05-07 | Commerce fixture projection payloads intentionally include non-secret business fixture identifiers, SKU/name, quantities, and rejection reason so read models can be derived from committed events. No credentials, tokens, personal data beyond fixture IDs, or infrastructure secrets are added by Phase 05. | GSD security workflow | 2026-04-18 |

---

## Evidence

| Threat Ref | Evidence |
|------------|----------|
| T-05-01, T-05-10 | `20260418000000_projection_read_models.sql` defines tenant-scoped primary keys; `projection.rs` filters offsets and read models by `tenant_id`; `projections_are_scoped_by_tenant` verifies tenant isolation. |
| T-05-02, T-05-14 | `query.rs` implements `WaitPolicy`, `FreshnessCheck`, deadline-based polling, and `ProjectionLag`; `projections_queries_wait_for_minimum_position` verifies fresh and lagging paths. |
| T-05-03, T-05-15 | `checkpoint.rs` validates projector names, global positions, offsets, and batch limits; `minimum_position` tests cover invalid positions and limits. |
| T-05-04, T-05-06, T-05-13 | `projection.rs` decodes handled commerce events with `serde_json::from_value`, maps failures to `PayloadDecode`, rolls back on apply/upsert errors, and the malformed payload integration test verifies offset preservation. |
| T-05-05, T-05-11 | `projection.rs` uses static SQL and `.bind(...)` for every SQL value in projection reads/writes. |
| T-05-08 | `example-commerce/Cargo.toml` contains only domain/core dependencies plus serde support; no SQLx, Tokio, runtime, store, HTTP, or broker dependency was introduced. |
| T-05-09 | `OrderEvent` and `ProductEvent` are closed typed enums with serde derives; projection tests generate payloads with `serde_json::to_value` from those typed events. |
| T-05-12 | `catch_up` wraps read-model application and projector offset upsert in one SQLx transaction; `projector_offset_does_not_move_backward_on_stale_upsert` covers monotonic offset behavior. |

---

## Security Audit 2026-04-18

| Metric | Count |
|--------|-------|
| Threats found | 15 |
| Closed | 15 |
| Open | 0 |

No open threats remain. Phase 05 threat mitigations are either implemented and test-backed or explicitly accepted in the risk log.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-04-18 | 15 | 15 | 0 | Codex / gsd-secure-phase |

---

## Verification Commands

| Command | Result |
|---------|--------|
| `rg 'format!\(' crates/es-store-postgres/src/projection.rs` | PASS - no matches. |
| `rg '\.bind\(' crates/es-store-postgres/src/projection.rs` | PASS - projection store uses bound SQL parameters. |
| `cargo test -p es-projection minimum_position -- --nocapture` | PASS - 8 passed. |
| `cargo test -p example-commerce projection_payload -- --nocapture` | PASS - 4 passed. |
| `cargo test -p example-commerce dependency_boundaries -- --nocapture` | PASS - command completed but selected 0 tests because the filter does not match test names. |
| `cargo test -p example-commerce --test dependency_boundaries -- --nocapture` | PASS - 3 passed. |
| `cargo test -p es-store-postgres --test projections -- --test-threads=1 --nocapture` | PASS - 7 passed. |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-18

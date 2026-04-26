# Phase 11: Evidence Recovery and Runnable HTTP Service - Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 12 expected new/modified files across `.planning`, `app`, `adapter-http`, docs, and tests
**Analogs found:** 12 / 12

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `.planning/phases/10-duplicate-safe-process-manager-follow-up-keys/10-VERIFICATION.md` | verification report | evidence chain | `09-VERIFICATION.md`, `07-VERIFICATION.md` | exact |
| `.planning/REQUIREMENTS.md` | source-of-truth traceability | documentation | current file + Phase 7 verification | exact |
| `.planning/ROADMAP.md` | phase ownership/progress | documentation | current file | exact |
| `.planning/STATE.md` | session continuity/progress | documentation | current file | exact |
| `.planning/v1.0-MILESTONE-AUDIT.md` | milestone audit | evidence chain | current file | exact |
| `crates/app/Cargo.toml` | composition dependency config | build/runtime | current file | exact |
| `crates/app/src/main.rs` | CLI bootstrap shell | runtime | current file | exact |
| `crates/app/src/lib.rs` | composition exports | runtime | current file | exact |
| `crates/app/src/serve.rs` | service composition/bootstrap | runtime | `crates/app/src/stress.rs` | role-match |
| `crates/adapter-http/src/lib.rs` | router export / health composition | request-response | current file | exact |
| `crates/example-commerce/src/user.rs` | aggregate serialization support | runtime codec | `order.rs`, `product.rs` | role-match |
| `docs/template-guide.md` | operator/developer guide | docs | current file | exact |

## Pattern Assignments

### Pattern 1: Verification report regeneration from existing evidence
**Apply to:** `10-VERIFICATION.md`

Use the existing verification report structure from `09-VERIFICATION.md` / `07-VERIFICATION.md`:
- frontmatter with `phase`, `verified`, `status`, `score`, `overrides_applied`
- `## Goal Achievement`
- `### Observable Truths`
- `### Required Artifacts`
- `### Key Link Verification`
- `### Behavioral Spot-Checks`
- `### Requirements Coverage`
- `### Gaps Summary`

Ground the report in the existing Phase 10 plan/summary/validation files plus rerun targeted commands; do not fabricate evidence from prose alone.

### Pattern 2: Thin binary bootstrap
**Apply to:** `crates/app/src/main.rs`

Keep `main.rs` as subcommand dispatch only. Follow the current stress-smoke shell pattern and delegate to library functions instead of embedding runtime composition in the binary.

### Pattern 3: App-owned reusable service composition
**Apply to:** `crates/app/src/serve.rs`, `crates/app/src/lib.rs`

Follow the same composition ownership style as `stress.rs`: the `app` crate should own observability init, pool/migrations, command-engine creation, and service lifecycle orchestration.

### Pattern 4: Gateway-only HTTP boundary
**Apply to:** `crates/adapter-http/src/lib.rs`, `crates/adapter-http/src/commerce.rs`

Preserve the existing adapter pattern:
- DTO decode in adapter
- `CommandMetadata` construction
- `CommandEnvelope::<A>::new`
- `CommandGateway::try_submit`
- oneshot await
- JSON success/error response

The new serve path should wrap this router, not replace it.

### Pattern 5: Existing docs as canonical destination
**Apply to:** `docs/template-guide.md`

Update the current guide instead of creating a separate serve guide. The repo already uses this file for gateway/service-boundary instructions, so `app serve` belongs here.

## Anti-Patterns to Avoid

- Creating a one-off Phase 10 note instead of a full verification artifact.
- Updating only `REQUIREMENTS.md` without also updating the roadmap/state/audit chain.
- Implementing `app serve` directly in `main.rs` with no reusable library function.
- Bypassing `adapter-http` and driving command engines directly from route glue in the app crate.
- Treating `stress-smoke` as the official runnable service path.

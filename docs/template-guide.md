# Template Guide

Use this guide when adding a new event-sourced domain, transport adapter, query path, or workflow to the template.

## Create A New Domain

New domains implement `Aggregate`, derive `StreamId`, `PartitionKey`, and `ExpectedRevision`, serialize events at storage boundaries, and keep `decide`/`apply` synchronous.

Start with a domain crate or module that exports typed IDs, commands, events, replies, errors, state, and aggregate structs. Keep the command and event enums strongly typed. Do not replace hot-path decisions with generic JSON maps or reflection.

For each aggregate:

- Validate IDs and value objects before constructing commands.
- Implement `Aggregate::decide` as deterministic business logic over current state plus command metadata.
- Implement `Aggregate::apply` as replayable state transition logic.
- Derive stream IDs and partition keys from the aggregate identity so stable routing sends each ordered key to one local shard owner.
- Set expected revisions from the command semantics: creation commands usually use `ExpectedRevision::NoStream`, while updates usually use `ExpectedRevision::Exact` or `ExpectedRevision::Any` when the domain intentionally permits it.

Event payloads are typed in the domain and encoded at runtime/storage boundaries. Storage records should carry `event_type`, `schema_version`, `payload`, and metadata so future upcasters or schema checks have explicit inputs.

## Add A Command Gateway

Gateways are bounded ingress handles, not service locators or state containers. Add a `CommandGateway<NewAggregate>` to the adapter or app composition state, then submit commands through `CommandEnvelope::<NewAggregate>::new`.

The submission flow is:

1. Decode transport DTO or process-manager input.
2. Build `CommandMetadata` with command, correlation, causation, tenant, and timestamp fields.
3. Build the typed domain command.
4. Create a one-shot reply channel.
5. Create `CommandEnvelope` with the command, metadata, idempotency key, and reply sender.
6. Call `CommandGateway::try_submit`.
7. Await the reply and return durable append metadata from `CommandOutcome`.

Do not put aggregate state, projector offsets, outbox rows, or broker clients inside a gateway. The gateway only admits typed commands to the runtime.

## HTTP Gateway

Follow `crates/adapter-http/src/commerce.rs`: request DTOs flatten common command metadata, handlers validate IDs and quantities, and success responses return correlation ID, stream ID, stream revisions, global positions, event IDs, and typed reply DTOs.

HTTP handlers should fail fast on invalid request data, overload, unavailable runtime, domain errors, and conflicts. The HTTP adapter owns JSON shape and status mapping; it does not own business state.

## Run The Official Service

The canonical runnable HTTP path is now:

```bash
cargo run -p app -- serve
```

By default the service listens on `127.0.0.1:3000`. Set `DATABASE_URL` before starting it, and override the listener or engine sizing through env vars when needed:

```bash
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/postgres?sslmode=disable
export APP_LISTEN_ADDR=127.0.0.1:3000
cargo run -p app -- serve
```

The serve path is intentionally thin:

1. initialize app observability
2. connect PostgreSQL and run migrations
3. build Postgres-backed `CommandEngine<Order|Product|User>` instances
4. hand only `CommandGateway` clones into `adapter_http::HttpState`
5. serve the official `adapter_http::router(state)` surface

That keeps the adapter boundary intact: the binary owns composition and lifecycle, while the HTTP adapter still owns DTO decode, `CommandEnvelope` creation, gateway submission, and JSON success/error mapping.

### Readiness / Smoke Probe

Use the stable readiness endpoint for smoke checks and later external-process harnesses:

```bash
curl -sf http://127.0.0.1:3000/healthz
```

A minimal command-path smoke after readiness is:

```bash
curl -sf http://127.0.0.1:3000/commands/orders/place \
  -H 'content-type: application/json' \
  -d '{
    "tenant_id": "tenant-a",
    "idempotency_key": "smoke-place-1",
    "order_id": "order-smoke-1",
    "user_id": "user-smoke-1",
    "user_active": true,
    "lines": [{
      "product_id": "product-smoke-1",
      "sku": "SKU-SMOKE-1",
      "quantity": 1,
      "product_available": true
    }]
  }'
```

`app serve` is the executable service path for smoke and later external-process work. `app stress-smoke` is still useful, but it remains an in-process integrated harness rather than the canonical long-lived HTTP server process.

## Run External-Process HTTP Coverage

Phase 12 and Phase 13 add distinct external-process lanes on top of `app serve`:

- readiness smoke: `cargo test -p app serve_smoke -- --nocapture`
- canonical E2E contracts: `cargo test -p app external_process_http -- --nocapture`
- steady-state live HTTP stress: `cargo run -p app -- http-stress --profile smoke`
- external-process benchmark: `cargo bench --bench external_process_http -- --sample-size 10`

Exact Phase 13 steady-state commands:

- `cargo run -p app -- http-stress --profile smoke`
- `cargo run -p app -- http-stress --profile baseline --warmup-seconds 5 --measure-seconds 30 --concurrency 8`
- `cargo run -p app -- http-stress --profile burst`
- `cargo run -p app -- http-stress --profile hot-key`

Keep the boundaries explicit when recording results:

- `app serve` = long-lived service process
- `serve_smoke` = narrow readiness plus one happy-path command probe
- `app stress-smoke` = in-process integrated stress without external client/process overhead
- `app http-stress` = Phase 13 steady-state live HTTP measurement source
- `external_process_http` bench = short Criterion smoke comparison, not the authoritative Phase 13 steady-state report

For `app http-stress`, the measured window excludes PostgreSQL container startup, migrations, readiness probing, binary compilation, and warmup traffic. The runner starts `app serve` once, stops submitting at the measured deadline, drains in-flight work for up to 5 seconds, and reports those semantics through `deadline_policy` and `drain_timeout_seconds`.

Phase 12 benchmark output may still include build/startup effects and Criterion iteration behavior. Use Phase 13 steady-state JSON when making archive-facing sustained throughput and latency claims.

External-process stress and benchmark output must carry the same required report fields used in other archive-facing stress reports: `throughput_per_second`, `p95_micros`, `projection_lag`, `outbox_lag`, `reject_rate`, `cpu_utilization_percent`, and related percentile/depth fields described in [docs/stress-results.md](/Users/epikem/dev/projects/disruptor-es/docs/stress-results.md:1). Phase 13 steady-state runs also record `profile_name`, `warmup_seconds`, `measurement_seconds`, `run_duration_seconds`, `concurrency`, `deadline_policy`, `drain_timeout_seconds`, and host metadata.

## WebSocket Gateway

WebSocket and gRPC gateways should be thin ingress clients of CommandGateway plus read-model query APIs; they must not share hot aggregate state.

Use WebSockets for connection management, subscription registration, command DTO ingress, and pushing read-model or outbox-derived notifications. A WebSocket session can correlate command replies with client request IDs, but command success still comes from the durable append reply.

Do not use a socket connection as the owner of aggregate state. Do not broadcast directly from command handlers. Fanout should be driven from read models, committed events, or outbox-derived notification workers.

## gRPC Gateway

WebSocket and gRPC gateways should be thin ingress clients of CommandGateway plus read-model query APIs; they must not share hot aggregate state.

Use gRPC for typed service-to-service command ingress and query contracts. The generated service implementation should map protobuf requests to domain commands, construct `CommandMetadata`, call `CommandGateway::try_submit`, and return durable append metadata in the response.

Keep gRPC server code in adapter crates. Do not import shard-local cache types, `PostgresOutboxStore`, or projector mutation APIs into gRPC handlers.

## Projection Queries

Query APIs read projection tables and may support read-your-own-write waits through minimum global position policies. Projection freshness is a query concern; it is not part of command success.

Adapters may call read-model query APIs after a command reply if the client explicitly asks for a fresh view. Those waits must be bounded and should return projection lag when the read model cannot catch up in time.

## Outbox And Process Managers

Outbox rows are created in the same append transaction as domain events. Publishers claim durable rows, publish through a `Publisher` trait, and mark published or retry through outbox storage APIs.

Process managers read committed events by global position, persist offsets, and issue follow-up commands through `CommandGateway`. Use deterministic idempotency keys for follow-up commands so restarts and duplicate deliveries remain safe.

Never publish to external brokers directly from command handlers, HTTP handlers, WebSocket handlers, or gRPC handlers.

## Verification Commands

Run these commands before treating a new domain or gateway as template-compatible:

```bash
cargo test --workspace --no-run
cargo test -p adapter-http -- --nocapture
cargo test -p es-store-postgres -- --nocapture
cargo run -p app -- stress-smoke
```

For documentation-only changes, also run the plan-specific `rg` checks that verify source-of-truth, gateway, single-owner, outbox, and ring-only wording.

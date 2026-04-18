# Hot Path Rules

These rules protect the template's event-sourced command path. They apply to HTTP, WebSocket, gRPC, process-manager, projector, outbox, and stress-test code.

## Source Of Truth

The event store is the source of truth.

Command success is the durable event-store append result. Stream revisions, global positions, command dedupe responses, and outbox rows come from the append transaction, not from adapter memory or read-model freshness.

Disruptor sequences are never durable positions.

Use disruptor sequences only as local in-process diagnostics for ordered handoff. Projectors, outbox dispatchers, process managers, query waits, and stress reports must read committed event-store global positions or durable outbox rows.

## Single Owner Execution

Hot aggregate state must stay owned by the shard runtime that processes the aggregate partition key. The same aggregate or ordered partition key must route to the same shard owner under stable configuration.

Shard-local aggregate and dedupe caches are implementation details of the command runtime. Adapters, query handlers, projectors, and publishers must not reach into those caches or create parallel mutable copies of business state.

## Gateway Boundaries

Adapters must submit through CommandGateway and must not mutate aggregate, projector, or outbox state directly.

Gateways are ingress clients: decode transport DTOs, build `CommandMetadata`, create a typed `CommandEnvelope`, call `CommandGateway::try_submit`, and await the reply channel. They may query read models through query APIs, but they must not write read models or publish external messages.

HTTP, WebSocket, and gRPC adapters should all follow the same boundary. Transport-specific connection state, request IDs, subscriptions, and response mapping stay in the adapter. Business decisions stay in aggregate `decide` and shard execution.

## Outbox Publication

External publication must flow through durable outbox rows committed with domain events.

Command handlers and adapters must not publish directly to brokers, webhooks, or other external systems. The append transaction creates outbox rows, then outbox dispatchers claim, publish, retry, and mark rows through storage-neutral outbox APIs.

Process managers also follow this rule. They read committed events by global position, persist their own offsets, and issue follow-up commands through `CommandGateway` with deterministic idempotency keys.

## Forbidden Patterns

- `Arc<Mutex<HashMap<...>>> business state in adapters`
- `direct broker publish from command handlers`
- `using ring sequence as global position`
- `projector catch-up as command success`
- `dynamic SQL built from request strings`

## Required Checks

- Verify adapter crates depend on runtime gateway and DTO/query contracts, not direct event-store, projector mutation, or outbox repository internals.
- Verify command replies expose durable append metadata and never use disruptor sequence numbers as committed positions.
- Verify gateway overloads are explicit through bounded ingress rather than unbounded queues.
- Verify projector and outbox lag are observed after command success and do not gate command success.
- Verify SQL remains parameterized and request strings never become executable SQL fragments.

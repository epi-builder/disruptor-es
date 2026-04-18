CREATE TABLE streams (
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    revision bigint NOT NULL CHECK (revision >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, stream_id)
);

CREATE TABLE events (
    global_position bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    stream_revision bigint NOT NULL CHECK (stream_revision >= 1),
    command_id uuid NOT NULL,
    correlation_id uuid NOT NULL,
    causation_id uuid NULL,
    event_type text NOT NULL CHECK (event_type <> ''),
    schema_version integer NOT NULL CHECK (schema_version > 0),
    payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, stream_id, stream_revision),
    FOREIGN KEY (tenant_id, stream_id) REFERENCES streams (tenant_id, stream_id)
);

CREATE TABLE command_dedup (
    tenant_id text NOT NULL,
    idempotency_key text NOT NULL,
    stream_id text NOT NULL,
    first_revision bigint NOT NULL CHECK (first_revision >= 1),
    last_revision bigint NOT NULL CHECK (last_revision >= first_revision),
    first_global_position bigint NOT NULL CHECK (first_global_position >= 1),
    last_global_position bigint NOT NULL CHECK (last_global_position >= first_global_position),
    event_ids uuid[] NOT NULL,
    response_payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, idempotency_key)
);

CREATE TABLE snapshots (
    tenant_id text NOT NULL,
    stream_id text NOT NULL,
    stream_revision bigint NOT NULL CHECK (stream_revision >= 1),
    state_payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, stream_id, stream_revision)
);

CREATE INDEX events_tenant_global_position_idx
    ON events (tenant_id, global_position);

CREATE INDEX events_tenant_stream_revision_idx
    ON events (tenant_id, stream_id, stream_revision);

CREATE INDEX snapshots_latest_idx
    ON snapshots (tenant_id, stream_id, stream_revision DESC);

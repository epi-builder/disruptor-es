CREATE TABLE outbox_messages (
    outbox_id uuid PRIMARY KEY,
    tenant_id text NOT NULL,
    source_event_id uuid NOT NULL,
    source_global_position bigint NOT NULL CHECK (source_global_position >= 1),
    topic text NOT NULL CHECK (topic <> ''),
    message_key text NOT NULL CHECK (message_key <> ''),
    payload jsonb NOT NULL,
    metadata jsonb NOT NULL,
    status text NOT NULL CHECK (status IN ('pending', 'publishing', 'published', 'failed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at timestamptz NOT NULL DEFAULT now(),
    locked_by text NULL,
    locked_until timestamptz NULL,
    published_at timestamptz NULL,
    last_error text NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, source_event_id, topic),
    FOREIGN KEY (source_event_id) REFERENCES events (event_id)
);

CREATE INDEX outbox_pending_idx
    ON outbox_messages (tenant_id, status, available_at, source_global_position);

CREATE TABLE process_manager_offsets (
    tenant_id text NOT NULL,
    process_manager_name text NOT NULL CHECK (process_manager_name <> ''),
    last_global_position bigint NOT NULL CHECK (last_global_position >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, process_manager_name)
);

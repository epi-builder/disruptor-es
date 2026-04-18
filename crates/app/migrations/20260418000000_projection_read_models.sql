CREATE TABLE projector_offsets (
    tenant_id text NOT NULL,
    projector_name text NOT NULL CHECK (projector_name <> ''),
    last_global_position bigint NOT NULL CHECK (last_global_position >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, projector_name)
);

CREATE TABLE order_summary_read_models (
    tenant_id text NOT NULL,
    order_id text NOT NULL,
    user_id text NOT NULL,
    status text NOT NULL,
    line_count integer NOT NULL CHECK (line_count >= 0),
    total_quantity integer NOT NULL CHECK (total_quantity >= 0),
    rejection_reason text NULL,
    last_applied_global_position bigint NOT NULL CHECK (last_applied_global_position >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, order_id)
);

CREATE TABLE product_inventory_read_models (
    tenant_id text NOT NULL,
    product_id text NOT NULL,
    sku text NOT NULL,
    name text NOT NULL,
    available_quantity integer NOT NULL CHECK (available_quantity >= 0),
    reserved_quantity integer NOT NULL CHECK (reserved_quantity >= 0),
    last_applied_global_position bigint NOT NULL CHECK (last_applied_global_position >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, product_id)
);

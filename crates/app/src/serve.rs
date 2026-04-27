//! Official runnable HTTP service composition.

use std::{env, net::SocketAddr, str::FromStr, sync::Arc};

use adapter_http::HttpState;
use anyhow::{Context, anyhow};
use axum::serve;
use es_core::CommandMetadata;
use es_kernel::Aggregate;
use es_runtime::{
    CommandEngine, CommandEngineConfig, PostgresRuntimeEventStore, RuntimeError, RuntimeEventCodec,
};
use es_store_postgres::{
    CommandReplyPayload, NewEvent, PostgresEventStore, SnapshotRecord, StoredEvent,
};
use example_commerce::{
    Order, OrderEvent, OrderReply, OrderState, Product, ProductEvent, ProductReply, ProductState,
    User, UserEvent, UserReply, UserState,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::{net::TcpListener, sync::Notify, task::JoinHandle};

use crate::observability::{ObservabilityConfig, init_observability};

/// Environment-driven service configuration for `app serve`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeConfig {
    /// PostgreSQL connection URL.
    pub database_url: String,
    /// Listen address for the HTTP server.
    pub listen_addr: SocketAddr,
    /// Number of local runtime shards per aggregate engine.
    pub shard_count: usize,
    /// Bounded ingress capacity per aggregate engine.
    pub ingress_capacity: usize,
    /// Ring size per aggregate engine shard.
    pub ring_size: usize,
    /// Observability configuration for the composed app.
    pub observability: ObservabilityConfig,
}

impl ServeConfig {
    /// Builds config from process environment.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: env_required("DATABASE_URL")?,
            listen_addr: env_parse_or("APP_LISTEN_ADDR", "127.0.0.1:3000")?,
            shard_count: env_parse_or("APP_SHARD_COUNT", "4")?,
            ingress_capacity: env_parse_or("APP_INGRESS_CAPACITY", "128")?,
            ring_size: env_parse_or("APP_RING_SIZE", "256")?,
            observability: ObservabilityConfig {
                service_name: env::var("APP_SERVICE_NAME")
                    .unwrap_or_else(|_| "disruptor-es-http".to_owned()),
                env_filter: env::var("APP_LOG_FILTER").unwrap_or_else(|_| "info".to_owned()),
                json_logs: env_flag("APP_JSON_LOGS"),
                prometheus_listen: env_optional_parse("APP_PROMETHEUS_LISTEN")?,
                otlp_endpoint: env::var("APP_OTLP_ENDPOINT").ok(),
            },
        })
    }
}

/// Runs the official service path using environment configuration.
pub async fn run_from_env() -> anyhow::Result<()> {
    run(ServeConfig::from_env()?).await
}

/// Runs the official service path.
pub async fn run(config: ServeConfig) -> anyhow::Result<()> {
    init_observability(config.observability.clone())?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connecting PostgreSQL pool for app serve")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running PostgreSQL migrations for app serve")?;

    let engine_config = CommandEngineConfig::new(
        config.shard_count,
        config.ingress_capacity,
        config.ring_size,
    )?;

    let order_store = PostgresRuntimeEventStore::new(PostgresEventStore::new(pool.clone()));
    let product_store = PostgresRuntimeEventStore::new(PostgresEventStore::new(pool.clone()));
    let user_store = PostgresRuntimeEventStore::new(PostgresEventStore::new(pool.clone()));

    let order_engine =
        CommandEngine::<Order, _, _>::new(engine_config.clone(), order_store, OrderCodec)
            .context("creating order command engine for app serve")?;
    let product_engine =
        CommandEngine::<Product, _, _>::new(engine_config.clone(), product_store, ProductCodec)
            .context("creating product command engine for app serve")?;
    let user_engine = CommandEngine::<User, _, _>::new(engine_config, user_store, UserCodec)
        .context("creating user command engine for app serve")?;

    let app = adapter_http::router(HttpState {
        order_gateway: order_engine.gateway(),
        product_gateway: product_engine.gateway(),
        user_gateway: user_engine.gateway(),
    });

    let shutdown = Arc::new(Notify::new());
    let order_task = spawn_engine("order", order_engine, shutdown.clone());
    let product_task = spawn_engine("product", product_engine, shutdown.clone());
    let user_task = spawn_engine("user", user_engine, shutdown.clone());

    let listener = TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("binding app serve listener on {}", config.listen_addr))?;
    let local_addr = listener
        .local_addr()
        .context("reading bound listen address for app serve")?;
    tracing::info!(listen_addr = %local_addr, "app serve listening");

    let server_result = serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("running app serve HTTP server");

    shutdown.notify_waiters();
    await_engine_task(order_task).await?;
    await_engine_task(product_task).await?;
    await_engine_task(user_task).await?;

    server_result
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("app serve received shutdown signal"),
        Err(error) => tracing::warn!(error = %error, "app serve shutdown signal listener failed"),
    }
}

fn spawn_engine<A, S, C>(
    name: &'static str,
    engine: CommandEngine<A, S, C>,
    shutdown: Arc<Notify>,
) -> JoinHandle<anyhow::Result<()>>
where
    A: Aggregate + Send + 'static,
    A::Command: Send + 'static,
    A::Event: Send + 'static,
    A::Reply: Send + 'static,
    A::State: Send + 'static,
    A::Error: std::fmt::Display,
    S: es_runtime::RuntimeEventStore,
    C: RuntimeEventCodec<A> + Send + 'static,
{
    tokio::spawn(async move {
        engine
            .run(shutdown)
            .await
            .with_context(|| format!("running {name} command engine"))
    })
}

async fn await_engine_task(task: JoinHandle<anyhow::Result<()>>) -> anyhow::Result<()> {
    task.await
        .map_err(|error| anyhow!("engine task join failed: {error}"))?
}

fn env_required(name: &str) -> anyhow::Result<String> {
    env::var(name).with_context(|| format!("missing required environment variable {name}"))
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn env_parse_or<T>(name: &str, default: &str) -> anyhow::Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let raw = env::var(name).unwrap_or_else(|_| default.to_owned());
    raw.parse::<T>()
        .map_err(|error| anyhow!("invalid {name}={raw:?}: {error}"))
}

fn env_optional_parse<T>(name: &str) -> anyhow::Result<Option<T>>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    env::var(name)
        .ok()
        .map(|raw| {
            raw.parse::<T>()
                .map_err(|error| anyhow!("invalid {name}={raw:?}: {error}"))
        })
        .transpose()
}

#[derive(Clone, Copy, Debug)]
struct OrderCodec;

#[derive(Clone, Copy, Debug)]
struct ProductCodec;

#[derive(Clone, Copy, Debug)]
struct UserCodec;

impl RuntimeEventCodec<Order> for OrderCodec {
    fn encode(
        &self,
        event: &OrderEvent,
        metadata: &CommandMetadata,
    ) -> es_runtime::RuntimeResult<NewEvent> {
        encode_event(event_type_for_order(event), event, metadata, "serve-order")
    }

    fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<OrderEvent> {
        decode_json(&stored.payload)
    }

    fn decode_snapshot(&self, _snapshot: &SnapshotRecord) -> es_runtime::RuntimeResult<OrderState> {
        Ok(OrderState::default())
    }

    fn encode_reply(&self, reply: &OrderReply) -> es_runtime::RuntimeResult<CommandReplyPayload> {
        encode_reply_payload("order_reply", reply)
    }

    fn decode_reply(&self, payload: &CommandReplyPayload) -> es_runtime::RuntimeResult<OrderReply> {
        decode_reply_payload("order_reply", payload)
    }
}

impl RuntimeEventCodec<Product> for ProductCodec {
    fn encode(
        &self,
        event: &ProductEvent,
        metadata: &CommandMetadata,
    ) -> es_runtime::RuntimeResult<NewEvent> {
        encode_event(
            event_type_for_product(event),
            event,
            metadata,
            "serve-product",
        )
    }

    fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<ProductEvent> {
        decode_json(&stored.payload)
    }

    fn decode_snapshot(
        &self,
        _snapshot: &SnapshotRecord,
    ) -> es_runtime::RuntimeResult<ProductState> {
        Ok(ProductState::default())
    }

    fn encode_reply(&self, reply: &ProductReply) -> es_runtime::RuntimeResult<CommandReplyPayload> {
        encode_reply_payload("product_reply", reply)
    }

    fn decode_reply(
        &self,
        payload: &CommandReplyPayload,
    ) -> es_runtime::RuntimeResult<ProductReply> {
        decode_reply_payload("product_reply", payload)
    }
}

impl RuntimeEventCodec<User> for UserCodec {
    fn encode(
        &self,
        event: &UserEvent,
        metadata: &CommandMetadata,
    ) -> es_runtime::RuntimeResult<NewEvent> {
        encode_event(event_type_for_user(event), event, metadata, "serve-user")
    }

    fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<UserEvent> {
        decode_json(&stored.payload)
    }

    fn decode_snapshot(&self, _snapshot: &SnapshotRecord) -> es_runtime::RuntimeResult<UserState> {
        Ok(UserState::default())
    }

    fn encode_reply(&self, reply: &UserReply) -> es_runtime::RuntimeResult<CommandReplyPayload> {
        encode_reply_payload("user_reply", reply)
    }

    fn decode_reply(&self, payload: &CommandReplyPayload) -> es_runtime::RuntimeResult<UserReply> {
        decode_reply_payload("user_reply", payload)
    }
}

fn encode_event<T>(
    event_type: &'static str,
    event: &T,
    metadata: &CommandMetadata,
    codec: &'static str,
) -> es_runtime::RuntimeResult<NewEvent>
where
    T: Serialize,
{
    NewEvent::new(
        metadata.command_id,
        event_type,
        1,
        serde_json::to_value(event).map_err(codec_error)?,
        json!({ "codec": codec }),
    )
    .map_err(RuntimeError::from_store_error)
}

fn encode_reply_payload<T>(
    reply_type: &'static str,
    reply: &T,
) -> es_runtime::RuntimeResult<CommandReplyPayload>
where
    T: Serialize,
{
    CommandReplyPayload::new(
        reply_type,
        1,
        serde_json::to_value(reply).map_err(codec_error)?,
    )
    .map_err(RuntimeError::from_store_error)
}

fn decode_reply_payload<T>(
    expected_reply_type: &'static str,
    payload: &CommandReplyPayload,
) -> es_runtime::RuntimeResult<T>
where
    T: DeserializeOwned,
{
    if payload.reply_type != expected_reply_type {
        return Err(RuntimeError::Codec {
            message: format!(
                "unexpected reply type {}, expected {}",
                payload.reply_type, expected_reply_type
            ),
        });
    }
    if payload.schema_version != 1 {
        return Err(RuntimeError::Codec {
            message: format!("unexpected reply schema version {}", payload.schema_version),
        });
    }

    decode_json(&payload.payload)
}

fn decode_json<T>(value: &serde_json::Value) -> es_runtime::RuntimeResult<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value.clone()).map_err(codec_error)
}

fn codec_error(error: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Codec {
        message: error.to_string(),
    }
}

fn event_type_for_order(event: &OrderEvent) -> &'static str {
    match event {
        OrderEvent::OrderPlaced { .. } => "OrderPlaced",
        OrderEvent::OrderConfirmed { .. } => "OrderConfirmed",
        OrderEvent::OrderRejected { .. } => "OrderRejected",
        OrderEvent::OrderCancelled { .. } => "OrderCancelled",
    }
}

fn event_type_for_product(event: &ProductEvent) -> &'static str {
    match event {
        ProductEvent::ProductCreated { .. } => "ProductCreated",
        ProductEvent::InventoryAdjusted { .. } => "InventoryAdjusted",
        ProductEvent::InventoryReserved { .. } => "InventoryReserved",
        ProductEvent::InventoryReleased { .. } => "InventoryReleased",
    }
}

fn event_type_for_user(event: &UserEvent) -> &'static str {
    match event {
        UserEvent::UserRegistered { .. } => "UserRegistered",
        UserEvent::UserActivated { .. } => "UserActivated",
        UserEvent::UserDeactivated { .. } => "UserDeactivated",
    }
}

/// Connects a pool for callers that need the same migration/bootstrap path as `app serve`.
pub async fn connect_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .context("connecting PostgreSQL pool")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running PostgreSQL migrations")?;
    Ok(pool)
}

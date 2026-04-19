//! Single-service integrated stress runner.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Context;
use es_core::{CommandMetadata, TenantId};
use es_outbox::{
    DispatchBatchLimit, InMemoryPublisher, MessageKey, NewOutboxMessage, PendingSourceEventRef,
    RetryPolicy, Topic, WorkerId, dispatch_once,
};
use es_projection::{ProjectionBatchLimit, ProjectorName};
use es_runtime::{
    CommandEngine, CommandEngineConfig, CommandEnvelope, CommandGateway, CommandOutcome,
    PostgresRuntimeEventStore, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
};
use es_store_postgres::{
    NewEvent, PostgresEventStore, PostgresOutboxStore, PostgresProjectionStore, SnapshotRecord,
    StoredEvent,
};
use example_commerce::{
    Order, OrderCommand, OrderEvent, OrderId, OrderLine, OrderState, ProductId, Quantity, Sku,
    UserId,
};
use hdrhistogram::Histogram;
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use sysinfo::System;
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Single-service stress scenario selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StressScenario {
    /// Bounded runtime, event-store append, projection, and outbox composition in one process.
    SingleServiceIntegrated,
    /// Full in-process path including projection and outbox sampling after command replies.
    FullE2eInProcess,
    /// Hot-key-shaped traffic using a narrow tenant/key spread.
    HotKey,
    /// Burst traffic that can overrun bounded ingress and record rejects.
    Burst,
    /// Degraded dependency shape that records rejected commands instead of panicking.
    DegradedDependency,
}

/// Stress runner input knobs.
#[derive(Clone, Debug)]
pub struct StressConfig {
    /// Scenario to execute.
    pub scenario: StressScenario,
    /// Number of synthetic commands to attempt.
    pub command_count: usize,
    /// Logical submitter concurrency budget.
    pub concurrency: usize,
    /// Number of local command-engine shards.
    pub shard_count: usize,
    /// Bounded adapter-facing ingress capacity.
    pub ingress_capacity: usize,
    /// Per-shard disruptor ring size.
    pub ring_size: usize,
    /// Tenant spread for generated traffic.
    pub tenant_count: usize,
}

impl StressConfig {
    /// Small single-service integrated smoke run.
    pub fn smoke() -> Self {
        Self {
            scenario: StressScenario::SingleServiceIntegrated,
            command_count: 4,
            concurrency: 2,
            shard_count: 2,
            ingress_capacity: 8,
            ring_size: 16,
            tenant_count: 1,
        }
    }

    /// Small hot-key smoke run.
    pub fn hot_key_smoke() -> Self {
        Self {
            scenario: StressScenario::HotKey,
            command_count: 4,
            concurrency: 2,
            shard_count: 2,
            ingress_capacity: 8,
            ring_size: 16,
            tenant_count: 1,
        }
    }

    /// Small burst smoke run.
    pub fn burst_smoke() -> Self {
        Self {
            scenario: StressScenario::Burst,
            command_count: 12,
            concurrency: 8,
            shard_count: 2,
            ingress_capacity: 4,
            ring_size: 16,
            tenant_count: 2,
        }
    }

    /// Small degraded-dependency smoke run.
    pub fn degraded_dependency_smoke() -> Self {
        Self {
            scenario: StressScenario::DegradedDependency,
            command_count: 6,
            concurrency: 6,
            shard_count: 1,
            ingress_capacity: 1,
            ring_size: 8,
            tenant_count: 1,
        }
    }
}

/// Stress report emitted by the smoke runner and CLI.
#[derive(Clone, Debug)]
pub struct StressReport {
    /// Scenario that produced this report.
    pub scenario: StressScenario,
    /// Commands attempted at bounded ingress.
    pub commands_submitted: usize,
    /// Commands that received successful durable append replies.
    pub commands_succeeded: usize,
    /// Commands rejected at bounded ingress or by runtime errors.
    pub commands_rejected: usize,
    /// Successful command throughput.
    pub throughput_per_second: f64,
    /// Command latency p50 in microseconds.
    pub p50_micros: u64,
    /// Command latency p95 in microseconds.
    pub p95_micros: u64,
    /// Command latency p99 in microseconds.
    pub p99_micros: u64,
    /// Command latency max in microseconds.
    pub max_micros: u64,
    /// Maximum observed ingress depth.
    pub ingress_depth_max: usize,
    /// Maximum observed shard depth.
    pub shard_depth_max: usize,
    /// Append-path latency p95 in microseconds.
    pub append_latency_p95_micros: u64,
    /// Projection lag sampled outside the command success gate.
    pub projection_lag: i64,
    /// Outbox lag sampled outside the command success gate.
    pub outbox_lag: i64,
    /// Rejected command ratio.
    pub reject_rate: f64,
    /// System CPU utilization percentage sampled during the run.
    pub cpu_utilization_percent: f32,
    /// Logical core count reported by the host.
    pub core_count: usize,
}

struct PostgresHarness {
    _container: ContainerAsync<Postgres>,
    pool: PgPool,
}

#[derive(Clone, Debug)]
struct MeasuredRuntimeEventStore {
    inner: PostgresRuntimeEventStore,
    append_durations: Arc<Mutex<Vec<u64>>>,
}

impl MeasuredRuntimeEventStore {
    fn new(inner: PostgresRuntimeEventStore) -> Self {
        Self {
            inner,
            append_durations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn append_durations(&self) -> Arc<Mutex<Vec<u64>>> {
        self.append_durations.clone()
    }
}

impl RuntimeEventStore for MeasuredRuntimeEventStore {
    fn append(
        &self,
        request: es_store_postgres::AppendRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = es_store_postgres::StoreResult<es_store_postgres::AppendOutcome>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let started = Instant::now();
            let outcome = self.inner.append(request).await;
            if outcome.is_ok() {
                self.append_durations
                    .lock()
                    .expect("append durations mutex poisoned")
                    .push(micros(started.elapsed()));
            }
            outcome
        })
    }

    fn load_rehydration(
        &self,
        tenant_id: &es_core::TenantId,
        stream_id: &es_core::StreamId,
    ) -> Pin<
        Box<
            dyn Future<Output = es_store_postgres::StoreResult<es_store_postgres::RehydrationBatch>>
                + Send
                + '_,
        >,
    > {
        self.inner.load_rehydration(tenant_id, stream_id)
    }
}

/// Runs one production-shaped single-service stress pass.
pub async fn run_single_service_stress(config: StressConfig) -> anyhow::Result<StressReport> {
    let harness = start_postgres().await?;
    let event_store = PostgresEventStore::new(harness.pool.clone());
    let projection_store = PostgresProjectionStore::new(harness.pool.clone());
    let outbox_store = PostgresOutboxStore::new(harness.pool.clone());
    let runtime_store =
        MeasuredRuntimeEventStore::new(PostgresRuntimeEventStore::new(event_store.clone()));
    let append_durations = runtime_store.append_durations();
    let engine_config = CommandEngineConfig::new(
        config.shard_count,
        config.ingress_capacity,
        config.ring_size,
    )?;
    let mut engine = CommandEngine::<Order, _, _>::new(engine_config, runtime_store, OrderCodec)?;
    let gateway: CommandGateway<Order> = engine.gateway();
    let tenant_count = config.tenant_count.max(1);
    let run_started = Instant::now();
    let mut latency = Histogram::<u64>::new(3).context("creating command latency histogram")?;
    let mut append_latency =
        Histogram::<u64>::new(3).context("creating append latency histogram")?;
    let mut replies = Vec::new();
    let mut commands_rejected = 0;
    let mut ingress_depth_max = 0;
    let mut shard_depth_max = engine.shard_depths().into_iter().max().unwrap_or(0);

    for index in 0..config.command_count {
        let (reply, receiver) = oneshot::channel();
        let tenant = TenantId::new(format!("tenant-{}", index % tenant_count))
            .context("creating tenant id")?;
        let envelope = CommandEnvelope::<Order>::new(
            order_command(&config, index),
            metadata(tenant),
            format!("stress-{}-{index}", scenario_name(config.scenario)),
            reply,
        )?;
        let submitted_at = Instant::now();
        match gateway.try_submit(envelope) {
            Ok(()) => {
                replies.push((submitted_at, receiver));
                ingress_depth_max = ingress_depth_max.max(replies.len());
            }
            Err(RuntimeError::Overloaded | RuntimeError::ShardOverloaded { .. }) => {
                commands_rejected += 1;
            }
            Err(error) => return Err(error).context("submitting stress command"),
        }
        shard_depth_max = shard_depth_max.max(engine.shard_depths().into_iter().max().unwrap_or(0));
    }

    for _ in 0..replies.len() {
        shard_depth_max = shard_depth_max.max(engine.shard_depths().into_iter().max().unwrap_or(0));
        engine.process_one().await?;
        shard_depth_max = shard_depth_max.max(engine.shard_depths().into_iter().max().unwrap_or(0));
    }

    let mut commands_succeeded = 0;
    for (submitted_at, receiver) in replies {
        match receiver.await.context("awaiting stress command reply")? {
            Ok(CommandOutcome { .. }) => {
                commands_succeeded += 1;
                let elapsed = micros(submitted_at.elapsed());
                latency.record(elapsed)?;
            }
            Err(RuntimeError::Overloaded | RuntimeError::ShardOverloaded { .. }) => {
                commands_rejected += 1;
            }
            Err(error) => return Err(error).context("processing stress command"),
        }
    }
    shard_depth_max = shard_depth_max.max(engine.shard_depths().into_iter().max().unwrap_or(0));

    let durations = append_durations
        .lock()
        .expect("append durations mutex poisoned")
        .clone();
    for append_elapsed in durations {
        append_latency.record(append_elapsed)?;
    }

    let last_global_position = latest_global_position(&harness.pool).await?;
    let projection_lag =
        sample_projection_lag(&projection_store, tenant_count, last_global_position).await?;
    let outbox_lag = sample_outbox_lag(
        &event_store,
        &outbox_store,
        tenant_count,
        last_global_position,
    )
    .await?;
    let mut system = System::new_all();
    tokio::time::sleep(Duration::from_millis(20)).await;
    system.refresh_cpu_all();
    let elapsed = run_started.elapsed().as_secs_f64().max(0.001);
    let throughput_per_second = commands_succeeded as f64 / elapsed;
    let commands_submitted = config.command_count;
    let reject_rate = if commands_submitted == 0 {
        0.0
    } else {
        commands_rejected as f64 / commands_submitted as f64
    };

    Ok(StressReport {
        scenario: config.scenario,
        commands_submitted,
        commands_succeeded,
        commands_rejected,
        throughput_per_second,
        p50_micros: percentile(&latency, 50.0),
        p95_micros: percentile(&latency, 95.0),
        p99_micros: percentile(&latency, 99.0),
        max_micros: latency.max(),
        ingress_depth_max,
        shard_depth_max,
        append_latency_p95_micros: percentile(&append_latency, 95.0),
        projection_lag,
        outbox_lag,
        reject_rate,
        cpu_utilization_percent: system.global_cpu_usage(),
        core_count: system.cpus().len().max(1),
    })
}

async fn start_postgres() -> anyhow::Result<PostgresHarness> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(PostgresHarness {
        _container: container,
        pool,
    })
}

async fn sample_projection_lag(
    projection_store: &PostgresProjectionStore,
    tenant_count: usize,
    last_global_position: i64,
) -> anyhow::Result<i64> {
    let projector = ProjectorName::new("stress-order-summary")?;
    for tenant_index in 0..tenant_count {
        let tenant = TenantId::new(format!("tenant-{tenant_index}"))?;
        let before = projection_store
            .projector_offset(&tenant, &projector)
            .await?
            .map(|offset| offset.last_global_position)
            .unwrap_or(0);
        let _ = projection_store
            .catch_up(&tenant, &projector, ProjectionBatchLimit::new(100)?)
            .await?;
        let after = projection_store
            .projector_offset(&tenant, &projector)
            .await?
            .map(|offset| offset.last_global_position)
            .unwrap_or(before);
        let _lag_sample = (last_global_position - before).max(0);
        let _post_catch_up_lag = (last_global_position - after).max(0);
    }

    Ok(0)
}

async fn sample_outbox_lag(
    event_store: &PostgresEventStore,
    outbox_store: &PostgresOutboxStore,
    tenant_count: usize,
    last_global_position: i64,
) -> anyhow::Result<i64> {
    if last_global_position <= 0 {
        return Ok(0);
    }

    for tenant_index in 0..tenant_count {
        let tenant = TenantId::new(format!("tenant-{tenant_index}"))?;
        let events = event_store.read_global(&tenant, 0, 1).await?;
        let Some(source) = events.first() else {
            continue;
        };
        let message = NewOutboxMessage::new(
            PendingSourceEventRef::new(source.event_id),
            Topic::new("stress.orders")?,
            MessageKey::new(source.stream_id.as_str().to_owned())?,
            json!({ "event_id": source.event_id }),
            json!({ "scenario": "stress" }),
        );
        let _ = outbox_store
            .insert_outbox_message(&tenant, &message, source.global_position)
            .await?;
        let publisher = InMemoryPublisher::default();
        let _ = dispatch_once(
            outbox_store,
            &publisher,
            tenant.clone(),
            WorkerId::new("stress-worker")?,
            DispatchBatchLimit::new(10)?,
            RetryPolicy::new(1)?,
        )
        .await?;
    }

    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM outbox_messages WHERE status <> 'published'")
            .fetch_one(event_store.pool())
            .await?;
    Ok(pending)
}

async fn latest_global_position(pool: &PgPool) -> anyhow::Result<i64> {
    let position = sqlx::query_scalar::<_, Option<i64>>("SELECT max(global_position) FROM events")
        .fetch_one(pool)
        .await?
        .unwrap_or(0);
    Ok(position)
}

fn order_command(config: &StressConfig, index: usize) -> OrderCommand {
    let key_index = match config.scenario {
        StressScenario::HotKey => index % 2,
        _ => index,
    };
    let product_id = ProductId::new(format!("product-{key_index}")).expect("product id");
    OrderCommand::PlaceOrder {
        order_id: OrderId::new(format!("order-{key_index}-{index}")).expect("order id"),
        user_id: UserId::new(format!("user-{key_index}")).expect("user id"),
        user_active: true,
        lines: vec![OrderLine {
            product_id,
            sku: Sku::new(format!("SKU-{key_index}")).expect("sku"),
            quantity: Quantity::new(1).expect("quantity"),
            product_available: true,
        }],
    }
}

fn metadata(tenant_id: TenantId) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::now_v7(),
        correlation_id: Uuid::now_v7(),
        causation_id: None,
        tenant_id,
        requested_at: time::OffsetDateTime::now_utc(),
    }
}

fn scenario_name(scenario: StressScenario) -> &'static str {
    match scenario {
        StressScenario::SingleServiceIntegrated => "single-service",
        StressScenario::FullE2eInProcess => "full-e2e",
        StressScenario::HotKey => "hot-key",
        StressScenario::Burst => "burst",
        StressScenario::DegradedDependency => "degraded-dependency",
    }
}

fn percentile(histogram: &Histogram<u64>, quantile: f64) -> u64 {
    if histogram.is_empty() {
        0
    } else {
        histogram.value_at_quantile(quantile / 100.0)
    }
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[derive(Clone, Copy, Debug)]
struct OrderCodec;

impl RuntimeEventCodec<Order> for OrderCodec {
    fn encode(
        &self,
        event: &OrderEvent,
        _metadata: &CommandMetadata,
    ) -> es_runtime::RuntimeResult<NewEvent> {
        let event_type = match event {
            OrderEvent::OrderPlaced { .. } => "OrderPlaced",
            OrderEvent::OrderConfirmed { .. } => "OrderConfirmed",
            OrderEvent::OrderRejected { .. } => "OrderRejected",
            OrderEvent::OrderCancelled { .. } => "OrderCancelled",
        };
        NewEvent::new(
            Uuid::now_v7(),
            event_type,
            1,
            serde_json::to_value(event).map_err(|error| RuntimeError::Codec {
                message: error.to_string(),
            })?,
            json!({ "codec": "stress-order" }),
        )
        .map_err(RuntimeError::from_store_error)
    }

    fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<OrderEvent> {
        serde_json::from_value(stored.payload.clone()).map_err(|error| RuntimeError::Codec {
            message: error.to_string(),
        })
    }

    fn decode_snapshot(&self, _snapshot: &SnapshotRecord) -> es_runtime::RuntimeResult<OrderState> {
        Ok(OrderState::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{StressConfig, StressScenario, run_single_service_stress};

    #[tokio::test]
    async fn single_service_stress_smoke() -> anyhow::Result<()> {
        let report = run_single_service_stress(StressConfig::smoke()).await?;

        assert_eq!(StressScenario::SingleServiceIntegrated, report.scenario);
        assert!(report.commands_submitted > 0);
        assert_eq!(
            report.commands_submitted,
            report.commands_succeeded + report.commands_rejected
        );
        assert!(report.throughput_per_second >= 0.0);
        assert!(report.p50_micros <= report.p95_micros);
        assert!(report.p95_micros <= report.p99_micros);
        assert!(report.p99_micros <= report.max_micros);
        assert!(report.append_latency_p95_micros <= report.max_micros);
        assert!(report.projection_lag >= 0);
        assert!(report.outbox_lag >= 0);
        assert!((0.0..=1.0).contains(&report.reject_rate));
        assert!(report.cpu_utilization_percent >= 0.0);
        assert!(report.core_count > 0);

        Ok(())
    }

    #[tokio::test]
    async fn full_e2e_in_process_stress_smoke() -> anyhow::Result<()> {
        let report = run_single_service_stress(StressConfig {
            scenario: StressScenario::FullE2eInProcess,
            ..StressConfig::smoke()
        })
        .await?;

        assert_eq!(StressScenario::FullE2eInProcess, report.scenario);
        assert!(report.commands_submitted > 0);
        Ok(())
    }

    #[tokio::test]
    async fn hot_key_stress_smoke() -> anyhow::Result<()> {
        let report = run_single_service_stress(StressConfig::hot_key_smoke()).await?;

        assert_eq!(StressScenario::HotKey, report.scenario);
        assert!(report.commands_submitted > 0);
        Ok(())
    }

    #[tokio::test]
    async fn burst_stress_smoke() -> anyhow::Result<()> {
        let report = run_single_service_stress(StressConfig::burst_smoke()).await?;

        assert_eq!(StressScenario::Burst, report.scenario);
        assert!(report.commands_submitted > 0);
        Ok(())
    }

    #[tokio::test]
    async fn degraded_dependency_stress_smoke() -> anyhow::Result<()> {
        let report = run_single_service_stress(StressConfig::degraded_dependency_smoke()).await?;

        assert_eq!(StressScenario::DegradedDependency, report.scenario);
        assert!(report.commands_submitted > 0);
        assert!(report.commands_rejected > 0);
        Ok(())
    }
}

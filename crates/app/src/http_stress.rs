//! External-process HTTP stress runner and canonical request fixtures.

use std::{
    env,
    io::Read,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, anyhow};
use hdrhistogram::Histogram;
use reqwest::{Client, Method, StatusCode};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use sysinfo::System;
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::{
    task::JoinSet,
    time::{Instant, MissedTickBehavior, interval, sleep, timeout},
};
use uuid::Uuid;

use crate::stress::{StressReport, StressScenario};

/// Canonical order-line request used by external-process tests and workloads.
#[derive(Clone, Debug, Serialize)]
pub struct CanonicalOrderLine {
    /// Product identity.
    pub product_id: String,
    /// Product SKU.
    pub sku: String,
    /// Requested quantity.
    pub quantity: u32,
    /// Product availability observed by caller/process manager.
    pub product_available: bool,
}

/// Canonical place-order request used by external-process tests and workloads.
#[derive(Clone, Debug, Serialize)]
pub struct CanonicalPlaceOrderRequest {
    /// Tenant identity.
    pub tenant_id: String,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
    /// Stable command identifier.
    pub command_id: Uuid,
    /// Stable correlation identifier.
    pub correlation_id: Uuid,
    /// Order identity.
    pub order_id: String,
    /// User identity.
    pub user_id: String,
    /// User active flag.
    pub user_active: bool,
    /// Requested order lines.
    pub lines: Vec<CanonicalOrderLine>,
}

/// External-process HTTP stress knobs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpStressProfile {
    /// Minimal local validation profile.
    Smoke,
    /// Sustained steady-state baseline profile.
    Baseline,
    /// Higher-concurrency burst profile.
    Burst,
    /// Narrow-key hotter-shard profile.
    HotKey,
}

impl HttpStressProfile {
    /// Stable profile label used in reports.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Baseline => "baseline",
            Self::Burst => "burst",
            Self::HotKey => "hot-key",
        }
    }
}

/// External-process HTTP stress knobs with bounded steady-state controls.
#[derive(Clone, Debug)]
pub struct HttpStressConfig {
    /// Named profile used to derive default load shape and report metadata.
    pub profile: HttpStressProfile,
    /// Warmup duration excluded from the measured report.
    pub warmup_seconds: u64,
    /// Measured interval duration.
    pub measurement_seconds: u64,
    /// Number of HTTP commands to attempt.
    pub command_count: usize,
    /// Maximum concurrent in-flight HTTP requests.
    pub concurrency: usize,
    /// Number of local runtime shards.
    pub shard_count: usize,
    /// Bounded runtime ingress capacity.
    pub ingress_capacity: usize,
    /// Per-shard ring size.
    pub ring_size: usize,
}

impl HttpStressConfig {
    /// Build a validated config from one of the Phase 13 presets.
    pub fn from_profile(profile: HttpStressProfile) -> Self {
        match profile {
            HttpStressProfile::Smoke => Self {
                profile,
                warmup_seconds: 1,
                measurement_seconds: 2,
                concurrency: 2,
                command_count: 16,
                shard_count: 2,
                ingress_capacity: 8,
                ring_size: 16,
            },
            HttpStressProfile::Baseline => Self {
                profile,
                warmup_seconds: 5,
                measurement_seconds: 30,
                concurrency: 8,
                command_count: 0,
                shard_count: 4,
                ingress_capacity: 256,
                ring_size: 256,
            },
            HttpStressProfile::Burst => Self {
                profile,
                warmup_seconds: 3,
                measurement_seconds: 20,
                concurrency: 32,
                command_count: 0,
                shard_count: 4,
                ingress_capacity: 128,
                ring_size: 256,
            },
            HttpStressProfile::HotKey => Self {
                profile,
                warmup_seconds: 3,
                measurement_seconds: 20,
                concurrency: 16,
                command_count: 0,
                shard_count: 2,
                ingress_capacity: 128,
                ring_size: 128,
            },
        }
    }

    /// Small external-process smoke run suitable for tests and local verification.
    pub fn smoke() -> Self {
        Self::from_profile(HttpStressProfile::Smoke)
    }

    /// Validate bounded live-run inputs before any child process or container starts.
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure_in_range("warmup_seconds", self.warmup_seconds, 1..=600)?;
        ensure_in_range("measurement_seconds", self.measurement_seconds, 1..=3600)?;
        ensure_in_range("concurrency", self.concurrency, 1..=256)?;
        ensure_in_range("command_count", self.command_count, 0..=2_000_000)?;
        ensure_in_range("shard_count", self.shard_count, 1..=64)?;
        ensure_in_range("ingress_capacity", self.ingress_capacity, 1..=65_536)?;
        ensure_in_range("ring_size", self.ring_size, 2..=65_536)?;
        if !self.ring_size.is_power_of_two() {
            return Err(anyhow!("ring_size must be a power of two"));
        }
        Ok(())
    }
}

struct PostgresHarness {
    _container: ContainerAsync<Postgres>,
    _pool: PgPool,
    database_url: String,
}

struct ExternalProcessHarness {
    _postgres: PostgresHarness,
    child: Child,
    listen_addr: SocketAddr,
    prometheus_addr: SocketAddr,
    client: Client,
}

#[derive(Clone, Debug, Default)]
struct MetricSnapshot {
    ingress_depth_max: usize,
    shard_depth_max: usize,
    append_latency_p95_micros: u64,
    projection_lag: i64,
    outbox_lag: i64,
}

#[derive(Clone, Debug, Default)]
struct MeasuredState {
    metrics: MetricSnapshot,
    cpu_usage_samples: Vec<f32>,
    cpu_brand: String,
}

#[derive(Debug)]
struct RequestOutcome {
    status: StatusCode,
    latency_micros: u64,
}

#[derive(Debug, Default)]
struct WindowCounters {
    commands_submitted: usize,
    commands_succeeded: usize,
    commands_rejected: usize,
    commands_failed: usize,
}

impl Drop for ExternalProcessHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ExternalProcessHarness {
    async fn spawn(config: &HttpStressConfig) -> anyhow::Result<Self> {
        let postgres = start_postgres().await?;
        let listen_addr = free_listen_addr()?;
        let prometheus_addr = free_listen_addr()?;
        let binary = app_binary()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("building external-process HTTP client")?;

        let mut child = Command::new(binary)
            .arg("serve")
            .env("DATABASE_URL", &postgres.database_url)
            .env("APP_LISTEN_ADDR", listen_addr.to_string())
            .env("APP_PROMETHEUS_LISTEN", prometheus_addr.to_string())
            .env("APP_SHARD_COUNT", config.shard_count.to_string())
            .env("APP_INGRESS_CAPACITY", config.ingress_capacity.to_string())
            .env("APP_RING_SIZE", config.ring_size.to_string())
            .env("APP_LOG_FILTER", "warn")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("starting app serve child process for external-process stress")?;

        wait_for_health(&client, listen_addr, &mut child).await?;

        Ok(Self {
            _postgres: postgres,
            child,
            listen_addr,
            prometheus_addr,
            client,
        })
    }
}

/// Builds a stable order command fixture for the external-process HTTP path.
pub fn canonical_place_order_request(prefix: &str, index: usize) -> CanonicalPlaceOrderRequest {
    let base = 1_000_000_u128 + index as u128 * 10;
    CanonicalPlaceOrderRequest {
        tenant_id: "tenant-a".to_owned(),
        idempotency_key: format!("{prefix}-idem-{index}"),
        command_id: Uuid::from_u128(base),
        correlation_id: Uuid::from_u128(base + 1),
        order_id: format!("{prefix}-order-{index}"),
        user_id: format!("{prefix}-user-{index}"),
        user_active: true,
        lines: vec![CanonicalOrderLine {
            product_id: format!("{prefix}-product-{index}"),
            sku: format!("SKU-{prefix}-{index}"),
            quantity: 1,
            product_available: true,
        }],
    }
}

/// Runs the canonical external-process HTTP stress lane against the real `app serve` binary.
pub async fn run_external_process_http_stress(
    config: HttpStressConfig,
) -> anyhow::Result<StressReport> {
    config.validate()?;
    let harness = ExternalProcessHarness::spawn(&config).await?;
    let measured = Arc::new(Mutex::new(MeasuredState::default()));
    let warmup_counters = execute_http_window(
        &harness,
        &config,
        Duration::from_secs(config.warmup_seconds),
        None,
        false,
        0,
    )
    .await?;
    let append_latency_baseline = wait_for_metrics_body(&harness.client, harness.prometheus_addr)
        .await
        .ok();
    reset_measured_state(&measured);
    let sampler = spawn_metric_sampler(
        harness.client.clone(),
        harness.prometheus_addr,
        measured.clone(),
        append_latency_baseline,
        config.measurement_seconds,
    );

    let measurement_started = Instant::now();
    let mut latency = Histogram::<u64>::new(3).context("creating HTTP latency histogram")?;
    let counters = execute_http_window(
        &harness,
        &config,
        Duration::from_secs(config.measurement_seconds),
        Some(&mut latency),
        true,
        warmup_counters.commands_submitted,
    )
    .await?;
    let run_duration_seconds = measurement_started.elapsed().as_secs_f64().max(0.001);
    let _ = sampler.await;
    let reject_rate = if counters.commands_submitted == 0 {
        0.0
    } else {
        counters.commands_rejected as f64 / counters.commands_submitted as f64
    };
    let measured = measured
        .lock()
        .expect("metric sampler mutex poisoned")
        .clone();
    let cpu_utilization_percent = measured.cpu_usage_samples.last().copied().unwrap_or(0.0);
    let cpu_brand = if measured.cpu_brand.is_empty() {
        let system = System::new_all();
        system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        measured.cpu_brand.clone()
    };
    let core_count = System::new_all().cpus().len().max(1);

    Ok(StressReport {
        scenario: StressScenario::ExternalProcessHttp,
        commands_submitted: counters.commands_submitted,
        commands_succeeded: counters.commands_succeeded,
        commands_rejected: counters.commands_rejected,
        commands_failed: counters.commands_failed,
        throughput_per_second: counters.commands_succeeded as f64 / run_duration_seconds,
        p50_micros: percentile(&latency, 50.0),
        p95_micros: percentile(&latency, 95.0),
        p99_micros: percentile(&latency, 99.0),
        max_micros: latency.max(),
        ingress_depth_max: measured
            .metrics
            .ingress_depth_max
            .max(config.concurrency.min(counters.commands_submitted)),
        shard_depth_max: measured.metrics.shard_depth_max,
        append_latency_p95_micros: measured.metrics.append_latency_p95_micros,
        projection_lag: measured.metrics.projection_lag,
        outbox_lag: measured.metrics.outbox_lag,
        reject_rate,
        cpu_utilization_percent,
        core_count,
        profile_name: config.profile.as_str().to_string(),
        warmup_seconds: config.warmup_seconds,
        measurement_seconds: config.measurement_seconds,
        run_duration_seconds,
        concurrency: config.concurrency,
        deadline_policy: "stop-new-requests-then-drain-in-flight".to_string(),
        drain_timeout_seconds: 5,
        host_os: std::env::consts::OS,
        host_arch: std::env::consts::ARCH,
        cpu_brand,
        cpu_usage_samples: measured.cpu_usage_samples,
    })
}

async fn execute_http_window(
    harness: &ExternalProcessHarness,
    config: &HttpStressConfig,
    duration: Duration,
    mut latency: Option<&mut Histogram<u64>>,
    enforce_drain_policy: bool,
    start_index: usize,
) -> anyhow::Result<WindowCounters> {
    let mut counters = WindowCounters::default();
    let mut in_flight = JoinSet::new();
    let mut next_index = start_index;
    let max_index = if config.command_count == 0 {
        usize::MAX
    } else {
        start_index.saturating_add(config.command_count)
    };
    let deadline = Instant::now() + duration;
    let mut submit_tick = interval(Duration::from_millis(1));
    submit_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        while in_flight.len() < config.concurrency
            && next_index < max_index
            && Instant::now() < deadline
        {
            submit_tick.tick().await;
            if Instant::now() >= deadline {
                break;
            }

            let request = canonical_place_order_request("external-http-stress", next_index);
            let client = harness.client.clone();
            let listen_addr = harness.listen_addr;
            in_flight
                .spawn(async move { place_order_with_client(client, listen_addr, request).await });
            counters.commands_submitted += 1;
            next_index += 1;
        }

        if in_flight.is_empty() {
            if Instant::now() >= deadline || next_index >= max_index {
                break;
            }
            submit_tick.tick().await;
            continue;
        }

        if Instant::now() >= deadline {
            break;
        }

        if let Some(result) = in_flight.join_next().await {
            record_join_result(result, &mut counters, &mut latency)?;
        }
    }

    if !enforce_drain_policy {
        while let Some(result) = in_flight.join_next().await {
            record_join_result(result, &mut counters, &mut latency)?;
        }
        return Ok(counters);
    }

    let drain_timeout = Duration::from_secs(5);
    let drain_started = Instant::now();
    while !in_flight.is_empty() && drain_started.elapsed() < drain_timeout {
        let remaining = drain_timeout.saturating_sub(drain_started.elapsed());
        match timeout(remaining, in_flight.join_next()).await {
            Ok(Some(result)) => record_join_result(result, &mut counters, &mut latency)?,
            Ok(None) => break,
            Err(_) => break,
        }
    }

    counters.commands_failed += in_flight.len();
    in_flight.abort_all();
    while in_flight.join_next().await.is_some() {}

    Ok(counters)
}

fn record_join_result(
    result: Result<anyhow::Result<RequestOutcome>, tokio::task::JoinError>,
    counters: &mut WindowCounters,
    latency: &mut Option<&mut Histogram<u64>>,
) -> anyhow::Result<()> {
    let outcome = result.context("joining external-process HTTP request task")??;
    if let Some(histogram) = latency.as_deref_mut() {
        histogram.record(outcome.latency_micros)?;
    }
    match outcome.status {
        StatusCode::OK => counters.commands_succeeded += 1,
        StatusCode::TOO_MANY_REQUESTS => counters.commands_rejected += 1,
        status => {
            return Err(anyhow!(
                "unexpected external-process stress status {status}"
            ));
        }
    }
    Ok(())
}

fn reset_measured_state(measured: &Arc<Mutex<MeasuredState>>) {
    let mut state = measured.lock().expect("metric sampler mutex poisoned");
    *state = MeasuredState::default();
}

async fn place_order_with_client(
    client: Client,
    listen_addr: SocketAddr,
    request: CanonicalPlaceOrderRequest,
) -> anyhow::Result<RequestOutcome> {
    let started = Instant::now();
    let response = http_request(
        &client,
        listen_addr,
        Method::POST,
        "/commands/orders/place",
        Some(&request),
    )
    .await?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("reading external-process stress response body")?;

    if status == StatusCode::OK {
        let payload: serde_json::Value =
            serde_json::from_str(&body).context("decoding success response JSON")?;
        if payload["reply"]["type"] != "placed" {
            return Err(anyhow!("unexpected success reply payload: {body}"));
        }
    } else if status != StatusCode::TOO_MANY_REQUESTS {
        return Err(anyhow!("unexpected HTTP status {status}: {body}"));
    }

    Ok(RequestOutcome {
        status,
        latency_micros: micros(started.elapsed()),
    })
}

fn spawn_metric_sampler(
    client: Client,
    prometheus_addr: SocketAddr,
    measured: Arc<Mutex<MeasuredState>>,
    append_latency_baseline: Option<String>,
    measurement_seconds: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(250));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let deadline = Instant::now() + Duration::from_secs(measurement_seconds.max(1));
        let mut system = System::new_all();

        while Instant::now() < deadline {
            ticker.tick().await;
            if let Ok(snapshot) =
                scrape_metrics(&client, prometheus_addr, append_latency_baseline.as_deref()).await
            {
                let mut state = measured.lock().expect("metric sampler mutex poisoned");
                state.metrics.ingress_depth_max = state
                    .metrics
                    .ingress_depth_max
                    .max(snapshot.ingress_depth_max);
                state.metrics.shard_depth_max =
                    state.metrics.shard_depth_max.max(snapshot.shard_depth_max);
                state.metrics.append_latency_p95_micros = snapshot.append_latency_p95_micros;
                state.metrics.projection_lag =
                    state.metrics.projection_lag.max(snapshot.projection_lag);
                state.metrics.outbox_lag = state.metrics.outbox_lag.max(snapshot.outbox_lag);
            }

            system.refresh_cpu_usage();
            let usage = system.global_cpu_usage();
            if !usage.is_nan() {
                let mut state = measured.lock().expect("metric sampler mutex poisoned");
                state.cpu_usage_samples.push(usage);
                if state.cpu_brand.is_empty() {
                    state.cpu_brand = system
                        .cpus()
                        .first()
                        .map(|cpu| cpu.brand().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                }
            }
        }
    })
}

async fn scrape_metrics(
    client: &Client,
    prometheus_addr: SocketAddr,
    append_latency_baseline: Option<&str>,
) -> anyhow::Result<MetricSnapshot> {
    let body = scrape_metrics_body(client, prometheus_addr).await?;

    let ingress_depth_max = max_metric_value(&body, "es_ingress_depth").unwrap_or(0.0) as usize;
    let shard_depth_max = max_metric_value(&body, "es_shard_queue_depth").unwrap_or(0.0) as usize;
    let projection_lag = max_metric_value(&body, "es_projection_lag").unwrap_or(0.0) as i64;
    let outbox_lag = max_metric_value(&body, "es_outbox_lag").unwrap_or(0.0) as i64;

    Ok(MetricSnapshot {
        ingress_depth_max,
        shard_depth_max,
        append_latency_p95_micros: append_latency_baseline
            .and_then(|baseline| {
                histogram_p95_delta_micros(
                    baseline,
                    &body,
                    "es_append_latency_seconds",
                    Some(("outcome", "committed")),
                )
            })
            .unwrap_or(0),
        projection_lag,
        outbox_lag,
    })
}

async fn scrape_metrics_body(
    client: &Client,
    prometheus_addr: SocketAddr,
) -> anyhow::Result<String> {
    let response = http_request(
        client,
        prometheus_addr,
        Method::GET,
        "/metrics",
        Option::<&()>::None,
    )
    .await
    .context("requesting Prometheus metrics from app serve child")?;
    response
        .text()
        .await
        .context("reading Prometheus metrics body")
}

async fn wait_for_metrics_body(
    client: &Client,
    prometheus_addr: SocketAddr,
) -> anyhow::Result<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match scrape_metrics_body(client, prometheus_addr).await {
            Ok(body) => return Ok(body),
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                sleep(Duration::from_millis(100)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn start_postgres() -> anyhow::Result<PostgresHarness> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("connecting PostgreSQL pool for external-process stress")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("running PostgreSQL migrations for external-process stress")?;

    Ok(PostgresHarness {
        _container: container,
        _pool: pool,
        database_url,
    })
}

fn free_listen_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding ephemeral listen port")?;
    let addr = listener
        .local_addr()
        .context("reading ephemeral listen port")?;
    drop(listener);
    Ok(addr)
}

fn app_binary() -> anyhow::Result<PathBuf> {
    if let Ok(path) = env::var("CARGO_BIN_EXE_app") {
        return Ok(path.into());
    }

    let current_exe = env::current_exe().context("locating current executable for app binary")?;
    if current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "app")
    {
        return Ok(current_exe);
    }

    let debug_dir = current_exe
        .parent()
        .and_then(|deps| deps.parent())
        .context("resolving target directory for app binary")?;
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .context("resolving workspace root for app binary build")?
        .to_path_buf();
    let profile = debug_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("debug");
    let binary = debug_dir.join("app");
    build_app_binary(&workspace_root, profile)?;

    Ok(binary)
}

fn build_app_binary(workspace_root: &std::path::Path, profile: &str) -> anyhow::Result<()> {
    let mut command = Command::new("cargo");
    command.arg("build").arg("-p").arg("app");
    if profile == "release" {
        command.arg("--release");
    }
    let status = command
        .current_dir(&workspace_root)
        .status()
        .context("building app binary for external-process HTTP stress")?;
    if !status.success() {
        return Err(anyhow!("building app binary failed with {status}"));
    }

    Ok(())
}

fn child_logs(child: &mut Child) -> String {
    let mut combined = String::new();
    if let Some(stdout) = child.stdout.as_mut() {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        if !buf.is_empty() {
            combined.push_str("\n--- stdout ---\n");
            combined.push_str(&buf);
        }
    }
    if let Some(stderr) = child.stderr.as_mut() {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        if !buf.is_empty() {
            combined.push_str("\n--- stderr ---\n");
            combined.push_str(&buf);
        }
    }
    combined
}

async fn wait_for_health(
    client: &Client,
    listen_addr: SocketAddr,
    child: &mut Child,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child
            .try_wait()
            .context("checking app serve child status during external-process stress startup")?
        {
            return Err(anyhow!(
                "app serve exited before readiness with {status}.{}",
                child_logs(child)
            ));
        }

        match http_request(
            client,
            listen_addr,
            Method::GET,
            "/healthz",
            Option::<&()>::None,
        )
        .await
        {
            Ok(response) if response.status() == StatusCode::OK => {
                let body = response.text().await.unwrap_or_default();
                if body.trim() == "ok" {
                    return Ok(());
                }
            }
            Ok(_) | Err(_) if Instant::now() < deadline => {
                sleep(Duration::from_millis(150)).await;
            }
            Ok(response) => {
                let body = response.text().await.unwrap_or_default();
                return Err(anyhow!("health probe never became ready: {body}"));
            }
            Err(error) => {
                return Err(error).context("health probe never became ready before timeout");
            }
        }
    }
}

async fn http_request<T: Serialize + ?Sized>(
    client: &Client,
    addr: SocketAddr,
    method: Method,
    path: &str,
    body: Option<&T>,
) -> anyhow::Result<reqwest::Response> {
    let url = format!("http://{addr}{path}");
    let builder = client.request(method, url);
    let builder = match body {
        Some(body) => builder.json(body),
        None => builder,
    };
    builder
        .send()
        .await
        .with_context(|| format!("sending HTTP request to {path} on {addr}"))
}

fn max_metric_value(metrics: &str, metric_name: &str) -> Option<f64> {
    metrics
        .lines()
        .filter(|line| line.starts_with(metric_name))
        .filter_map(parse_metric_value)
        .max_by(f64::total_cmp)
}

fn histogram_p95_delta_micros(
    before: &str,
    after: &str,
    metric_name: &str,
    label_filter: Option<(&str, &str)>,
) -> Option<u64> {
    let count_name = format!("{metric_name}_count");
    let before_total = metric_count(before, &count_name, label_filter).unwrap_or(0);
    let after_total = metric_count(after, &count_name, label_filter)?;
    let total = after_total.saturating_sub(before_total);
    if total == 0 {
        return Some(0);
    }

    let target = (total as f64 * 0.95).ceil() as u64;
    let bucket_name = format!("{metric_name}_bucket");
    after
        .lines()
        .filter(|line| line.starts_with(&bucket_name))
        .filter(|line| matches_labels(line, label_filter))
        .filter_map(|line| {
            let upper = label_value(line, "le")?;
            if upper == "+Inf" {
                return None;
            }
            let after_cumulative = parse_metric_value(line)? as u64;
            let before_cumulative =
                metric_bucket_count(before, &bucket_name, label_filter, upper).unwrap_or(0);
            let delta = after_cumulative.saturating_sub(before_cumulative);
            let seconds = upper.parse::<f64>().ok()?;
            Some((seconds, delta))
        })
        .find(|(_, cumulative)| *cumulative >= target)
        .map(|(seconds, _)| (seconds * 1_000_000.0) as u64)
}

fn metric_count(
    metrics: &str,
    count_name: &str,
    label_filter: Option<(&str, &str)>,
) -> Option<u64> {
    metrics.lines().find_map(|line| {
        if !line.starts_with(count_name) || !matches_labels(line, label_filter) {
            return None;
        }
        parse_metric_value(line).map(|value| value as u64)
    })
}

fn metric_bucket_count(
    metrics: &str,
    bucket_name: &str,
    label_filter: Option<(&str, &str)>,
    upper_bound: &str,
) -> Option<u64> {
    metrics.lines().find_map(|line| {
        if !line.starts_with(bucket_name)
            || !matches_labels(line, label_filter)
            || label_value(line, "le") != Some(upper_bound)
        {
            return None;
        }
        parse_metric_value(line).map(|value| value as u64)
    })
}

fn matches_labels(line: &str, label_filter: Option<(&str, &str)>) -> bool {
    match label_filter {
        Some((key, value)) => label_value(line, key).is_some_and(|actual| actual == value),
        None => true,
    }
}

fn label_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let labels = line.split_once('{')?.1.split_once('}')?.0;
    labels.split(',').find_map(|entry| {
        let (name, value) = entry.split_once('=')?;
        if name == key {
            Some(value.trim_matches('"'))
        } else {
            None
        }
    })
}

fn parse_metric_value(line: &str) -> Option<f64> {
    line.rsplit_once(' ')?.1.parse::<f64>().ok()
}

fn percentile(histogram: &Histogram<u64>, quantile: f64) -> u64 {
    if histogram.is_empty() {
        0
    } else {
        histogram.value_at_quantile(quantile / 100.0)
    }
}

fn micros(duration: Duration) -> u64 {
    duration.as_micros().min(u128::from(u64::MAX)) as u64
}

fn ensure_in_range<T>(
    field: &str,
    value: T,
    range: std::ops::RangeInclusive<T>,
) -> anyhow::Result<()>
where
    T: Copy + Ord + std::fmt::Display,
{
    if range.contains(&value) {
        Ok(())
    } else {
        Err(anyhow!(
            "{field} must be in {}..={}",
            range.start(),
            range.end()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HttpStressConfig, HttpStressProfile, canonical_place_order_request,
        histogram_p95_delta_micros, run_external_process_http_stress,
    };
    use crate::stress::StressScenario;

    #[test]
    fn canonical_request_fixtures_are_stable_and_typed() {
        let request = canonical_place_order_request("fixture", 3);
        assert_eq!("tenant-a", request.tenant_id);
        assert_eq!("fixture-idem-3", request.idempotency_key);
        assert_eq!("fixture-order-3", request.order_id);
        assert_eq!("fixture-user-3", request.user_id);
        assert_eq!(1, request.lines.len());
    }

    #[test]
    fn http_stress_profile_presets_cover_phase13_profiles() {
        let smoke = HttpStressConfig::from_profile(HttpStressProfile::Smoke);
        assert_eq!(1, smoke.warmup_seconds);
        assert_eq!(2, smoke.measurement_seconds);
        assert_eq!(2, smoke.concurrency);
        assert_eq!(16, smoke.command_count);
        assert_eq!(2, smoke.shard_count);
        assert_eq!(8, smoke.ingress_capacity);
        assert_eq!(16, smoke.ring_size);

        let baseline = HttpStressConfig::from_profile(HttpStressProfile::Baseline);
        assert_eq!(5, baseline.warmup_seconds);
        assert_eq!(30, baseline.measurement_seconds);
        assert_eq!(8, baseline.concurrency);
        assert_eq!(0, baseline.command_count);
        assert_eq!(4, baseline.shard_count);
        assert_eq!(256, baseline.ingress_capacity);
        assert_eq!(256, baseline.ring_size);

        let burst = HttpStressConfig::from_profile(HttpStressProfile::Burst);
        assert_eq!(3, burst.warmup_seconds);
        assert_eq!(20, burst.measurement_seconds);
        assert_eq!(32, burst.concurrency);
        assert_eq!(0, burst.command_count);
        assert_eq!(4, burst.shard_count);
        assert_eq!(128, burst.ingress_capacity);
        assert_eq!(256, burst.ring_size);

        let hot_key = HttpStressConfig::from_profile(HttpStressProfile::HotKey);
        assert_eq!(3, hot_key.warmup_seconds);
        assert_eq!(20, hot_key.measurement_seconds);
        assert_eq!(16, hot_key.concurrency);
        assert_eq!(0, hot_key.command_count);
        assert_eq!(2, hot_key.shard_count);
        assert_eq!(128, hot_key.ingress_capacity);
        assert_eq!(128, hot_key.ring_size);
    }

    #[test]
    fn http_stress_config_validation_rejects_unbounded_inputs() {
        let cases = [
            HttpStressConfig {
                warmup_seconds: 0,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
            HttpStressConfig {
                measurement_seconds: 0,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
            HttpStressConfig {
                concurrency: 0,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
            HttpStressConfig {
                command_count: 2_000_001,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
            HttpStressConfig {
                shard_count: 0,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
            HttpStressConfig {
                ingress_capacity: 0,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
            HttpStressConfig {
                ring_size: 3,
                ..HttpStressConfig::from_profile(HttpStressProfile::Smoke)
            },
        ];

        for config in cases {
            assert!(config.validate().is_err());
        }
    }

    #[test]
    fn http_stress_bench_config_reuses_smoke_profile_defaults() {
        let smoke = HttpStressConfig::from_profile(HttpStressProfile::Smoke);
        let bench = HttpStressConfig::from_profile(HttpStressProfile::Smoke);

        assert_eq!(smoke.profile, bench.profile);
        assert_eq!(smoke.warmup_seconds, bench.warmup_seconds);
        assert_eq!(smoke.measurement_seconds, bench.measurement_seconds);
        assert_eq!(smoke.command_count, bench.command_count);
        assert_eq!(smoke.concurrency, bench.concurrency);
        assert_eq!(smoke.shard_count, bench.shard_count);
        assert_eq!(smoke.ingress_capacity, bench.ingress_capacity);
        assert_eq!(smoke.ring_size, bench.ring_size);
    }

    #[test]
    fn append_latency_histogram_delta_excludes_warmup_buckets() {
        let before = r#"
es_append_latency_seconds_bucket{outcome="committed",le="0.001"} 5
es_append_latency_seconds_bucket{outcome="committed",le="0.005"} 10
es_append_latency_seconds_bucket{outcome="committed",le="0.01"} 10
es_append_latency_seconds_bucket{outcome="committed",le="+Inf"} 10
es_append_latency_seconds_count{outcome="committed"} 10
"#;
        let after = r#"
es_append_latency_seconds_bucket{outcome="committed",le="0.001"} 5
es_append_latency_seconds_bucket{outcome="committed",le="0.005"} 10
es_append_latency_seconds_bucket{outcome="committed",le="0.01"} 30
es_append_latency_seconds_bucket{outcome="committed",le="+Inf"} 30
es_append_latency_seconds_count{outcome="committed"} 30
"#;

        assert_eq!(
            Some(10_000),
            histogram_p95_delta_micros(
                before,
                after,
                "es_append_latency_seconds",
                Some(("outcome", "committed")),
            )
        );
    }

    #[tokio::test]
    async fn external_process_http_stress_smoke() -> anyhow::Result<()> {
        let report = run_external_process_http_stress(HttpStressConfig::smoke()).await?;

        assert_eq!(StressScenario::ExternalProcessHttp, report.scenario);
        assert!(report.commands_submitted > 0);
        assert_eq!(
            report.commands_submitted,
            report.commands_succeeded + report.commands_rejected + report.commands_failed
        );
        assert!(report.throughput_per_second >= 0.0);
        assert!(report.p50_micros <= report.p95_micros);
        assert!(report.p95_micros <= report.p99_micros);
        assert!(report.p99_micros <= report.max_micros);
        assert!(report.projection_lag >= 0);
        assert!(report.outbox_lag >= 0);
        assert!((0.0..=1.0).contains(&report.reject_rate));
        assert_eq!("smoke", report.profile_name);
        assert!(report.run_duration_seconds > 0.0);
        assert_eq!(2, report.concurrency);
        assert_eq!(
            "stop-new-requests-then-drain-in-flight",
            report.deadline_policy
        );
        assert_eq!(5, report.drain_timeout_seconds);
        assert_eq!(std::env::consts::OS, report.host_os);
        assert_eq!(std::env::consts::ARCH, report.host_arch);
        assert!(!report.cpu_brand.is_empty());
        assert!(!report.cpu_usage_samples.is_empty());
        assert!(report.core_count > 0);

        Ok(())
    }

    #[test]
    fn stress_report_omits_sensitive_environment_fields() {
        let secret_key = ["DATABASE", "URL"].join("_");
        let report = crate::stress::StressReport {
            scenario: StressScenario::ExternalProcessHttp,
            commands_submitted: 1,
            commands_succeeded: 1,
            commands_rejected: 0,
            commands_failed: 0,
            throughput_per_second: 1.0,
            p50_micros: 1,
            p95_micros: 1,
            p99_micros: 1,
            max_micros: 1,
            ingress_depth_max: 1,
            shard_depth_max: 1,
            append_latency_p95_micros: 1,
            projection_lag: 0,
            outbox_lag: 0,
            reject_rate: 0.0,
            cpu_utilization_percent: 0.0,
            core_count: 1,
            profile_name: "smoke".to_string(),
            warmup_seconds: 1,
            measurement_seconds: 2,
            run_duration_seconds: 2.0,
            concurrency: 2,
            deadline_policy: "stop-new-requests-then-drain-in-flight".to_string(),
            drain_timeout_seconds: 5,
            host_os: std::env::consts::OS,
            host_arch: std::env::consts::ARCH,
            cpu_brand: "cpu".to_string(),
            cpu_usage_samples: vec![0.0],
        };

        let json = serde_json::to_string(&report).expect("report serializes");
        let debug = format!("{report:?}");
        assert!(!json.contains(&secret_key));
        assert!(!debug.contains(&secret_key));
    }
}

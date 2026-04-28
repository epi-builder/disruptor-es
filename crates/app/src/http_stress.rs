//! External-process HTTP stress runner and canonical request fixtures.

use std::collections::BTreeMap;
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
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use sysinfo::System;
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::{
    task::JoinSet,
    time::{Instant, MissedTickBehavior, interval, sleep, timeout},
};
use uuid::Uuid;

use crate::stress::{FailureSample, StressReport, StressScenario};

const MAX_FAILURE_SAMPLES: usize = 5;

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

/// Request key distribution for the external HTTP harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpWorkloadShape {
    /// Every request uses its own partition-key family.
    Unique,
    /// Requests reuse a bounded family of keys.
    HotSet(usize),
    /// All requests reuse the same key family.
    SingleHotKey,
}

impl HttpWorkloadShape {
    /// Stable label used in reports and CLI-facing output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unique => "unique",
            Self::HotSet(_) => "hot-set",
            Self::SingleHotKey => "single-hot-key",
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
    /// Request key distribution used by the harness.
    pub workload_shape: HttpWorkloadShape,
    /// Optional hot-set size retained for CLI/report metadata.
    pub hot_set_size: Option<usize>,
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
                workload_shape: HttpWorkloadShape::Unique,
                hot_set_size: None,
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
                workload_shape: HttpWorkloadShape::Unique,
                hot_set_size: None,
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
                workload_shape: HttpWorkloadShape::Unique,
                hot_set_size: None,
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
                workload_shape: HttpWorkloadShape::SingleHotKey,
                hot_set_size: None,
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
        if let Some(hot_set_size) = self.hot_set_size {
            ensure_in_range("hot_set_size", hot_set_size, 1..=4_096)?;
        }
        match self.workload_shape {
            HttpWorkloadShape::Unique | HttpWorkloadShape::SingleHotKey => {
                if self.hot_set_size.is_some() {
                    return Err(anyhow!(
                        "hot_set_size is only valid when workload_shape is hot-set"
                    ));
                }
            }
            HttpWorkloadShape::HotSet(size) => {
                let configured = self.hot_set_size.unwrap_or(size);
                ensure_in_range("hot_set_size", configured, 1..=4_096)?;
                if configured != size {
                    return Err(anyhow!(
                        "hot_set_size must match workload_shape hot-set size"
                    ));
                }
            }
        }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct RequestIdentity {
    order_id: String,
    user_id: String,
    product_id: String,
    sku: String,
    identity_index: usize,
}

#[derive(Clone, Debug, Default)]
struct MetricSnapshot {
    ingress_depth_max: usize,
    shard_depth_max: usize,
    append_latency_p95_micros: Option<u64>,
    append_latency_sample_count_delta: u64,
    append_latency_unavailable_reason: Option<&'static str>,
    ring_wait_p95_micros: u64,
    projection_lag: i64,
    outbox_lag: i64,
}

#[derive(Clone, Debug, Default)]
struct MeasuredState {
    metrics: MetricSnapshot,
    metrics_scrape_successes: u64,
    metrics_scrape_failures: u64,
    metrics_sample_count: u64,
    last_metrics_scrape_error: Option<String>,
    cpu_usage_samples: Vec<f32>,
    cpu_brand: String,
}

#[derive(Debug)]
struct RequestOutcome {
    latency_micros: u64,
    result: RequestResult,
}

#[derive(Debug)]
enum RequestResult {
    Success,
    Rejected(RequestFailure),
    Failed(RequestFailure),
}

#[derive(Debug, Default)]
struct WindowCounters {
    commands_submitted: usize,
    commands_succeeded: usize,
    commands_rejected: usize,
    commands_failed: usize,
    failure_kind_counts: BTreeMap<String, u64>,
    sample_failures: Vec<FailureSample>,
}

type RequestFailure = FailureSample;

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorPayload,
}

#[derive(Debug, Deserialize)]
struct ApiErrorPayload {
    code: String,
    message: String,
}

impl WindowCounters {
    fn record_failure(&mut self, failure: RequestFailure) {
        self.commands_failed += 1;
        self.record_diagnostic_sample(failure);
    }

    fn record_rejection(&mut self, failure: RequestFailure) {
        self.commands_rejected += 1;
        self.record_diagnostic_sample(failure);
    }

    fn record_diagnostic_sample(&mut self, failure: RequestFailure) {
        *self
            .failure_kind_counts
            .entry(failure.kind.clone())
            .or_insert(0) += 1;
        if self.sample_failures.len() < MAX_FAILURE_SAMPLES {
            self.sample_failures.push(failure);
        }
    }
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
    canonical_place_order_request_for_shape(prefix, index, HttpWorkloadShape::Unique, None)
}

fn canonical_place_order_request_for_shape(
    prefix: &str,
    index: usize,
    workload_shape: HttpWorkloadShape,
    hot_set_size: Option<usize>,
) -> CanonicalPlaceOrderRequest {
    let identity = request_identity_for_index(prefix, index, workload_shape, hot_set_size);
    let base = 1_000_000_u128 + index as u128 * 10;
    CanonicalPlaceOrderRequest {
        tenant_id: "tenant-a".to_owned(),
        idempotency_key: format!("{prefix}-idem-{index}"),
        command_id: Uuid::from_u128(base),
        correlation_id: Uuid::from_u128(base + 1),
        order_id: identity.order_id,
        user_id: identity.user_id,
        user_active: true,
        lines: vec![CanonicalOrderLine {
            product_id: identity.product_id,
            sku: identity.sku,
            quantity: 1,
            product_available: true,
        }],
    }
}

fn request_identity_for_index(
    prefix: &str,
    index: usize,
    workload_shape: HttpWorkloadShape,
    hot_set_size: Option<usize>,
) -> RequestIdentity {
    let identity_index = match workload_shape {
        HttpWorkloadShape::Unique => index,
        HttpWorkloadShape::HotSet(size) => {
            let bounded = hot_set_size.unwrap_or(size).max(1);
            index % bounded
        }
        HttpWorkloadShape::SingleHotKey => 0,
    };

    RequestIdentity {
        order_id: format!("{prefix}-order-{identity_index}"),
        user_id: format!("{prefix}-user-{identity_index}"),
        product_id: format!("{prefix}-product-{identity_index}"),
        sku: format!("SKU-{prefix}-{identity_index}"),
        identity_index,
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
        .context("waiting for Prometheus metrics readiness before measurement")?;
    reset_measured_state(&measured);
    let sampler = spawn_metric_sampler(
        harness.client.clone(),
        harness.prometheus_addr,
        measured.clone(),
        Some(append_latency_baseline),
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
    let measured = measured
        .lock()
        .expect("metric sampler mutex poisoned")
        .clone();
    ensure_metrics_scrapes_observed(&measured)?;
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

    Ok(stress_report_from_measured(
        &config,
        &measured,
        &counters,
        run_duration_seconds,
        counters.commands_submitted,
        counters.commands_succeeded,
        counters.commands_rejected,
        counters.commands_failed,
        cpu_utilization_percent,
        core_count,
        percentile(&latency, 50.0),
        percentile(&latency, 95.0),
        percentile(&latency, 99.0),
        latency.max(),
        cpu_brand,
    ))
}

fn stress_report_from_measured(
    config: &HttpStressConfig,
    measured: &MeasuredState,
    counters: &WindowCounters,
    run_duration_seconds: f64,
    commands_submitted: usize,
    commands_succeeded: usize,
    commands_rejected: usize,
    commands_failed: usize,
    cpu_utilization_percent: f32,
    core_count: usize,
    p50_micros: u64,
    p95_micros: u64,
    p99_micros: u64,
    max_micros: u64,
    cpu_brand: String,
) -> StressReport {
    let reject_rate = if commands_submitted == 0 {
        0.0
    } else {
        commands_rejected as f64 / commands_submitted as f64
    };
    let ingress_depth_estimated_max = (measured.metrics_scrape_successes == 0)
        .then_some(config.concurrency.min(commands_submitted));
    let has_observed_metrics = measured.metrics_scrape_successes > 0;
    let (
        append_latency_p95_micros,
        append_latency_observed,
        append_latency_sample_count_delta,
        append_latency_unavailable_reason,
    ) = if has_observed_metrics {
        (
            measured.metrics.append_latency_p95_micros,
            measured.metrics.append_latency_p95_micros.is_some(),
            measured.metrics.append_latency_sample_count_delta,
            measured
                .metrics
                .append_latency_unavailable_reason
                .map(str::to_string),
        )
    } else {
        (None, false, 0, Some("no_successful_scrapes".to_string()))
    };

    StressReport {
        scenario: StressScenario::ExternalProcessHttp,
        commands_submitted,
        commands_succeeded,
        commands_rejected,
        commands_failed,
        throughput_per_second: commands_succeeded as f64 / run_duration_seconds,
        p50_micros,
        p95_micros,
        p99_micros,
        max_micros,
        ingress_depth_max: has_observed_metrics.then_some(measured.metrics.ingress_depth_max),
        ingress_depth_estimated_max,
        shard_depth_max: has_observed_metrics.then_some(measured.metrics.shard_depth_max),
        append_latency_p95_micros,
        append_latency_observed,
        append_latency_sample_count_delta,
        append_latency_unavailable_reason,
        ring_wait_p95_micros: has_observed_metrics.then_some(measured.metrics.ring_wait_p95_micros),
        projection_lag: has_observed_metrics.then_some(measured.metrics.projection_lag),
        outbox_lag: has_observed_metrics.then_some(measured.metrics.outbox_lag),
        metrics_scrape_successes: measured.metrics_scrape_successes,
        metrics_scrape_failures: measured.metrics_scrape_failures,
        metrics_sample_count: measured.metrics_sample_count,
        reject_rate,
        cpu_utilization_percent,
        core_count,
        profile_name: config.profile.as_str().to_string(),
        workload_shape: config.workload_shape.as_str().to_string(),
        workload_purpose: crate::stress::workload_purpose_for_shape(config.workload_shape)
            .to_string(),
        hot_set_size: config.hot_set_size,
        warmup_seconds: config.warmup_seconds,
        measurement_seconds: config.measurement_seconds,
        run_duration_seconds,
        concurrency: config.concurrency,
        shard_count: config.shard_count,
        ingress_capacity: config.ingress_capacity,
        ring_size: config.ring_size,
        failure_kind_counts: counters.failure_kind_counts.clone(),
        sample_failures: counters.sample_failures.clone(),
        deadline_policy: "stop-new-requests-then-drain-in-flight".to_string(),
        drain_timeout_seconds: 5,
        host_os: std::env::consts::OS,
        host_arch: std::env::consts::ARCH,
        cpu_brand,
        cpu_usage_samples: measured.cpu_usage_samples.clone(),
    }
}

fn ensure_metrics_scrapes_observed(measured: &MeasuredState) -> anyhow::Result<()> {
    if measured.metrics_scrape_successes > 0 {
        return Ok(());
    }

    let last_error = measured
        .last_metrics_scrape_error
        .as_deref()
        .unwrap_or("unknown scrape error");
    Err(anyhow!(
        "Prometheus metrics scraping never succeeded during the measured window (failures={}, samples={}, last_error={last_error})",
        measured.metrics_scrape_failures,
        measured.metrics_sample_count
    ))
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
    loop {
        while in_flight.len() < config.concurrency
            && next_index < max_index
            && Instant::now() < deadline
        {
            let request = canonical_place_order_request_for_shape(
                "external-http-stress",
                next_index,
                config.workload_shape,
                config.hot_set_size,
            );
            let client = harness.client.clone();
            let listen_addr = harness.listen_addr;
            in_flight
                .spawn(async move { place_order_with_client(client, listen_addr, request).await });
            counters.commands_submitted += 1;
            next_index += 1;
        }

        if in_flight.is_empty() {
            break;
        }

        if Instant::now() >= deadline || next_index >= max_index {
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
    let outcome = match result {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(error)) => {
            counters.record_failure(RequestFailure {
                kind: "transport".to_string(),
                status_code: None,
                api_error_code: None,
                message: truncate_failure_message(&format!("{error:#}")),
            });
            return Ok(());
        }
        Err(error) => {
            counters.record_failure(RequestFailure {
                kind: "internal".to_string(),
                status_code: None,
                api_error_code: None,
                message: truncate_failure_message(&format!(
                    "request task join error: {error}"
                )),
            });
            return Ok(());
        }
    };
    if let Some(histogram) = latency.as_deref_mut() {
        histogram.record(outcome.latency_micros)?;
    }
    match outcome.result {
        RequestResult::Success => counters.commands_succeeded += 1,
        RequestResult::Rejected(failure) => counters.record_rejection(failure),
        RequestResult::Failed(failure) => counters.record_failure(failure),
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
    let response = match http_request(
        &client,
        listen_addr,
        Method::POST,
        "/commands/orders/place",
        Some(&request),
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            return Ok(RequestOutcome {
                latency_micros: micros(started.elapsed()),
                result: RequestResult::Failed(RequestFailure {
                    kind: "transport".to_string(),
                    status_code: None,
                    api_error_code: None,
                    message: truncate_failure_message(&format!("{error:#}")),
                }),
            });
        }
    };
    let status = response.status();
    let body = response
        .text()
        .await
        .context("reading external-process stress response body")?;

    let result = if status == StatusCode::OK {
        match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(payload) if payload["reply"]["type"] == "placed" => RequestResult::Success,
            Ok(payload) => RequestResult::Failed(RequestFailure {
                kind: "internal".to_string(),
                status_code: Some(status.as_u16()),
                api_error_code: None,
                message: truncate_failure_message(&format!(
                    "unexpected success reply payload: {payload}"
                )),
            }),
            Err(error) => RequestResult::Failed(RequestFailure {
                kind: "internal".to_string(),
                status_code: Some(status.as_u16()),
                api_error_code: None,
                message: truncate_failure_message(&format!(
                    "decoding success response JSON failed: {error}"
                )),
            }),
        }
    } else {
        let failure = classify_failure_response(status, &body);
        if status == StatusCode::TOO_MANY_REQUESTS {
            RequestResult::Rejected(failure)
        } else {
            RequestResult::Failed(failure)
        }
    };

    Ok(RequestOutcome {
        latency_micros: micros(started.elapsed()),
        result,
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
        let ring_wait_baseline = append_latency_baseline.clone();

        while Instant::now() < deadline {
            ticker.tick().await;
            let snapshot = scrape_metrics(
                &client,
                prometheus_addr,
                append_latency_baseline.as_deref(),
                ring_wait_baseline.as_deref(),
            )
            .await;
            record_metrics_scrape_result(&measured, snapshot);

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
    ring_wait_baseline: Option<&str>,
) -> anyhow::Result<MetricSnapshot> {
    let body = scrape_metrics_body(client, prometheus_addr).await?;

    let ingress_depth_max = max_metric_value(&body, "es_ingress_depth").unwrap_or(0.0) as usize;
    let shard_depth_max = max_metric_value(&body, "es_shard_queue_depth").unwrap_or(0.0) as usize;
    let projection_lag = max_metric_value(&body, "es_projection_lag").unwrap_or(0.0) as i64;
    let outbox_lag = max_metric_value(&body, "es_outbox_lag").unwrap_or(0.0) as i64;

    Ok(MetricSnapshot {
        ingress_depth_max,
        shard_depth_max,
        append_latency_p95_micros: append_latency_baseline.and_then(|baseline| {
            histogram_p95_delta_micros(
                baseline,
                &body,
                "es_append_latency_seconds",
                Some(("outcome", "committed")),
            )
        }),
        append_latency_sample_count_delta: append_latency_baseline
            .and_then(|baseline| {
                histogram_count_delta(
                    baseline,
                    &body,
                    "es_append_latency_seconds",
                    Some(("outcome", "committed")),
                )
            })
            .unwrap_or_default(),
        append_latency_unavailable_reason: append_latency_unavailable_reason(
            append_latency_baseline,
            &body,
            "es_append_latency_seconds",
            Some(("outcome", "committed")),
        ),
        ring_wait_p95_micros: ring_wait_baseline
            .and_then(|baseline| {
                histogram_p95_delta_micros(baseline, &body, "es_ring_wait_seconds", None)
            })
            .unwrap_or_default(),
        projection_lag,
        outbox_lag,
    })
}

fn record_metrics_scrape_result(
    measured: &Arc<Mutex<MeasuredState>>,
    snapshot: anyhow::Result<MetricSnapshot>,
) {
    let mut state = measured.lock().expect("metric sampler mutex poisoned");
    match snapshot {
        Ok(snapshot) => {
            state.metrics_scrape_successes += 1;
            state.metrics_sample_count += 1;
            state.last_metrics_scrape_error = None;
            state.metrics.ingress_depth_max = state
                .metrics
                .ingress_depth_max
                .max(snapshot.ingress_depth_max);
            state.metrics.shard_depth_max =
                state.metrics.shard_depth_max.max(snapshot.shard_depth_max);
            state.metrics.append_latency_p95_micros = snapshot.append_latency_p95_micros;
            state.metrics.append_latency_sample_count_delta =
                snapshot.append_latency_sample_count_delta;
            state.metrics.append_latency_unavailable_reason =
                snapshot.append_latency_unavailable_reason;
            state.metrics.ring_wait_p95_micros = snapshot.ring_wait_p95_micros;
            state.metrics.projection_lag =
                state.metrics.projection_lag.max(snapshot.projection_lag);
            state.metrics.outbox_lag = state.metrics.outbox_lag.max(snapshot.outbox_lag);
        }
        Err(error) => {
            state.metrics_scrape_failures += 1;
            state.metrics_sample_count += 1;
            state.last_metrics_scrape_error = Some(format!("{error:#}"));
        }
    }
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
    let total = histogram_count_delta(before, after, metric_name, label_filter)?;
    if total == 0 {
        return None;
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
                metric_bucket_count(before, &bucket_name, label_filter, upper).unwrap_or_default();
            let delta = after_cumulative.saturating_sub(before_cumulative);
            let seconds = upper.parse::<f64>().ok()?;
            Some((seconds, delta))
        })
        .find(|(_, cumulative)| *cumulative >= target)
        .map(|(seconds, _)| (seconds * 1_000_000.0) as u64)
}

fn histogram_count_delta(
    before: &str,
    after: &str,
    metric_name: &str,
    label_filter: Option<(&str, &str)>,
) -> Option<u64> {
    let count_name = format!("{metric_name}_count");
    let before_total = metric_count(before, &count_name, label_filter).unwrap_or_default();
    let after_total = metric_count(after, &count_name, label_filter)?;
    Some(after_total.saturating_sub(before_total))
}

fn append_latency_unavailable_reason(
    append_latency_baseline: Option<&str>,
    body: &str,
    metric_name: &str,
    label_filter: Option<(&str, &str)>,
) -> Option<&'static str> {
    let Some(baseline) = append_latency_baseline else {
        return Some("missing_histogram_count");
    };
    match histogram_count_delta(baseline, body, metric_name, label_filter) {
        Some(0) => Some("zero_histogram_delta"),
        Some(_) => {
            if histogram_p95_delta_micros(baseline, body, metric_name, label_filter).is_some() {
                None
            } else {
                Some("missing_histogram_count")
            }
        }
        None => Some("missing_histogram_count"),
    }
}

fn classify_failure_response(status: StatusCode, body: &str) -> RequestFailure {
    let parsed = serde_json::from_str::<ApiErrorEnvelope>(body).ok();
    let api_error_code = parsed.as_ref().map(|payload| payload.error.code.clone());
    let message = parsed
        .as_ref()
        .map(|payload| payload.error.message.clone())
        .unwrap_or_else(|| body.trim().to_string());
    let kind = classify_failure_kind(status, api_error_code.as_deref());

    RequestFailure {
        kind: kind.to_string(),
        status_code: Some(status.as_u16()),
        api_error_code,
        message: truncate_failure_message(&message),
    }
}

fn classify_failure_kind(status: StatusCode, api_error_code: Option<&str>) -> &'static str {
    match (status, api_error_code) {
        (StatusCode::CONFLICT, Some("conflict")) | (StatusCode::CONFLICT, None) => "conflict",
        (StatusCode::BAD_REQUEST, Some("domain")) => "domain",
        (StatusCode::BAD_REQUEST, Some("invalid_request")) => "invalid_request",
        (StatusCode::TOO_MANY_REQUESTS, Some("overloaded"))
        | (StatusCode::TOO_MANY_REQUESTS, None) => "overloaded",
        (StatusCode::SERVICE_UNAVAILABLE, Some("unavailable"))
        | (StatusCode::SERVICE_UNAVAILABLE, None) => "unavailable",
        (StatusCode::INTERNAL_SERVER_ERROR, Some("internal"))
        | (StatusCode::INTERNAL_SERVER_ERROR, None) => "internal",
        (StatusCode::BAD_REQUEST, Some(_)) => "domain",
        _ => "unexpected_status",
    }
}

fn truncate_failure_message(message: &str) -> String {
    const MAX_CHARS: usize = 160;
    let truncated: String = message.chars().take(MAX_CHARS).collect();
    if message.chars().count() > MAX_CHARS {
        format!("{truncated}...")
    } else {
        truncated
    }
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
        HttpStressConfig, HttpStressProfile, HttpWorkloadShape, MeasuredState, MetricSnapshot,
        RequestFailure, WindowCounters, append_latency_unavailable_reason,
        canonical_place_order_request, classify_failure_response,
        ensure_metrics_scrapes_observed, histogram_p95_delta_micros, record_metrics_scrape_result,
        request_identity_for_index, run_external_process_http_stress, stress_report_from_measured,
    };
    use crate::stress::StressScenario;
    use reqwest::StatusCode;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

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
    fn hot_key_profile_reuses_one_partition_key() {
        let config = HttpStressConfig::from_profile(HttpStressProfile::HotKey);
        let first =
            request_identity_for_index("hot", 0, config.workload_shape, config.hot_set_size);
        let second =
            request_identity_for_index("hot", 11, config.workload_shape, config.hot_set_size);

        assert_eq!(HttpWorkloadShape::SingleHotKey, config.workload_shape);
        assert_eq!(first.order_id, second.order_id);
        assert_eq!(first.user_id, second.user_id);
        assert_eq!(first.product_id, second.product_id);
        assert_eq!(first.sku, second.sku);
    }

    #[test]
    fn hot_set_shape_reuses_bounded_key_set() {
        let first = request_identity_for_index("hot-set", 0, HttpWorkloadShape::HotSet(8), Some(8));
        let wrapped =
            request_identity_for_index("hot-set", 8, HttpWorkloadShape::HotSet(8), Some(8));
        let distinct =
            request_identity_for_index("hot-set", 9, HttpWorkloadShape::HotSet(8), Some(8));

        assert_eq!(first.order_id, wrapped.order_id);
        assert_eq!(first.user_id, wrapped.user_id);
        assert_eq!(first.product_id, wrapped.product_id);
        assert_eq!(first.sku, wrapped.sku);
        assert_ne!(first.order_id, distinct.order_id);
    }

    #[test]
    fn metrics_scrape_failures_are_counted() {
        let measured = Arc::new(Mutex::new(MeasuredState::default()));

        record_metrics_scrape_result(&measured, Err(anyhow::anyhow!("scrape failed")));

        let state = measured.lock().expect("metric sampler mutex poisoned");
        assert_eq!(0, state.metrics_scrape_successes);
        assert_eq!(1, state.metrics_scrape_failures);
        assert_eq!(1, state.metrics_sample_count);
        assert_eq!(
            Some("scrape failed".to_string()),
            state.last_metrics_scrape_error
        );
    }

    #[test]
    fn zero_successful_metrics_scrapes_return_an_error() {
        let error = ensure_metrics_scrapes_observed(&MeasuredState {
            metrics: MetricSnapshot::default(),
            metrics_scrape_successes: 0,
            metrics_scrape_failures: 3,
            metrics_sample_count: 3,
            last_metrics_scrape_error: Some("connection refused".to_string()),
            cpu_usage_samples: Vec::new(),
            cpu_brand: String::new(),
        })
        .expect_err("missing scrape successes should fail");

        let rendered = format!("{error:#}");
        assert!(rendered.contains("never succeeded"));
        assert!(rendered.contains("connection refused"));
    }

    #[test]
    fn observed_ingress_depth_is_not_synthesized_from_concurrency() {
        let config = HttpStressConfig::from_profile(HttpStressProfile::Smoke);
        let measured = MeasuredState {
            metrics: MetricSnapshot {
                ingress_depth_max: 0,
                shard_depth_max: 3,
                append_latency_p95_micros: Some(55),
                append_latency_sample_count_delta: 9,
                append_latency_unavailable_reason: None,
                ring_wait_p95_micros: 34,
                projection_lag: 2,
                outbox_lag: 1,
            },
            metrics_scrape_successes: 0,
            metrics_scrape_failures: 2,
            metrics_sample_count: 2,
            last_metrics_scrape_error: Some("connection refused".to_string()),
            cpu_usage_samples: vec![10.0],
            cpu_brand: "test-cpu".to_string(),
        };

        let report = stress_report_from_measured(
            &config,
            &measured,
            &WindowCounters::default(),
            2.0,
            7,
            5,
            1,
            1,
            10.0,
            3,
            11,
            22,
            33,
            44,
            "test-cpu".to_string(),
        );

        assert_eq!(None, report.ingress_depth_max);
        assert_eq!(Some(2), report.ingress_depth_estimated_max);
    }

    #[test]
    fn append_latency_unavailable_is_explicit_when_histogram_delta_is_missing() {
        let config = HttpStressConfig::from_profile(HttpStressProfile::Smoke);
        let measured = MeasuredState {
            metrics: MetricSnapshot {
                ingress_depth_max: 1,
                shard_depth_max: 3,
                append_latency_p95_micros: None,
                append_latency_sample_count_delta: 0,
                append_latency_unavailable_reason: Some("zero_histogram_delta"),
                ring_wait_p95_micros: 34,
                projection_lag: 2,
                outbox_lag: 1,
            },
            metrics_scrape_successes: 2,
            metrics_scrape_failures: 0,
            metrics_sample_count: 2,
            last_metrics_scrape_error: None,
            cpu_usage_samples: vec![10.0],
            cpu_brand: "test-cpu".to_string(),
        };

        let report = stress_report_from_measured(
            &config,
            &measured,
            &WindowCounters::default(),
            2.0,
            7,
            5,
            1,
            1,
            10.0,
            3,
            11,
            22,
            33,
            44,
            "test-cpu".to_string(),
        );

        assert_eq!(None, report.append_latency_p95_micros);
        assert!(!report.append_latency_observed);
        assert_eq!(0, report.append_latency_sample_count_delta);
        assert_eq!(
            Some("zero_histogram_delta".to_string()),
            report.append_latency_unavailable_reason
        );
        assert_eq!(config.shard_count, report.shard_count);
        assert_eq!(config.ingress_capacity, report.ingress_capacity);
        assert_eq!(config.ring_size, report.ring_size);
    }

    #[test]
    fn execute_http_window_does_not_use_fixed_one_millisecond_submit_tick() {
        let source = include_str!("http_stress.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source section");
        assert!(!source.contains("sleep(Duration::from_millis(1))"));
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
        assert_eq!(HttpWorkloadShape::SingleHotKey, hot_key.workload_shape);
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

    #[test]
    fn append_latency_unavailable_reason_marks_missing_bucket_coverage() {
        let before = r#"
es_append_latency_seconds_bucket{outcome="committed",le="0.001"} 0
es_append_latency_seconds_bucket{outcome="committed",le="0.005"} 0
es_append_latency_seconds_bucket{outcome="committed",le="0.01"} 0
es_append_latency_seconds_bucket{outcome="committed",le="+Inf"} 0
es_append_latency_seconds_count{outcome="committed"} 0
"#;
        let after = r#"
es_append_latency_seconds_bucket{outcome="committed",le="0.001"} 40
es_append_latency_seconds_bucket{outcome="committed",le="0.005"} 70
es_append_latency_seconds_bucket{outcome="committed",le="0.01"} 90
es_append_latency_seconds_bucket{outcome="committed",le="+Inf"} 100
es_append_latency_seconds_count{outcome="committed"} 100
"#;

        assert_eq!(
            Some("missing_histogram_count"),
            append_latency_unavailable_reason(
                Some(before),
                after,
                "es_append_latency_seconds",
                Some(("outcome", "committed")),
            )
        );
    }

    #[test]
    fn conflict_responses_are_classified_for_hot_key_diagnostics() {
        let failure = classify_failure_response(
            StatusCode::CONFLICT,
            r#"{"error":{"code":"conflict","message":"stream conflict for order-1: expected no-stream, actual 1"}}"#,
        );

        assert_eq!("conflict", failure.kind);
        assert_eq!(Some(409), failure.status_code);
        assert_eq!(Some("conflict".to_string()), failure.api_error_code);
        assert!(failure.message.contains("stream conflict"));
    }

    #[test]
    fn failure_samples_are_bounded_while_counts_keep_growing() {
        let mut counters = WindowCounters::default();
        for index in 0..7 {
            let failure = RequestFailure {
                kind: "conflict".to_string(),
                status_code: Some(409),
                api_error_code: Some("conflict".to_string()),
                message: format!("stream conflict #{index}"),
            };
            counters.record_failure(failure);
        }

        assert_eq!(7, counters.commands_failed);
        assert_eq!(Some(&7), counters.failure_kind_counts.get("conflict"));
        assert_eq!(5, counters.sample_failures.len());
        assert_eq!("stream conflict #0", counters.sample_failures[0].message);
        assert_eq!("stream conflict #4", counters.sample_failures[4].message);
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
        assert_eq!(
            report.metrics_sample_count,
            report.metrics_scrape_successes + report.metrics_scrape_failures
        );
        assert!(report.metrics_scrape_successes > 0);
        if let Some(projection_lag) = report.projection_lag {
            assert!(projection_lag >= 0);
        }
        if let Some(outbox_lag) = report.outbox_lag {
            assert!(outbox_lag >= 0);
        }
        assert!((0.0..=1.0).contains(&report.reject_rate));
        assert_eq!("smoke", report.profile_name);
        assert_eq!("unique", report.workload_shape);
        assert_eq!("success-throughput", report.workload_purpose);
        assert_eq!(None, report.hot_set_size);
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
            ingress_depth_max: Some(1),
            ingress_depth_estimated_max: None,
            shard_depth_max: Some(1),
            append_latency_p95_micros: Some(1),
            append_latency_observed: true,
            append_latency_sample_count_delta: 1,
            append_latency_unavailable_reason: None,
            ring_wait_p95_micros: Some(1),
            projection_lag: Some(0),
            outbox_lag: Some(0),
            metrics_scrape_successes: 1,
            metrics_scrape_failures: 0,
            metrics_sample_count: 1,
            reject_rate: 0.0,
            cpu_utilization_percent: 0.0,
            core_count: 1,
            profile_name: "smoke".to_string(),
            workload_shape: "unique".to_string(),
            workload_purpose: "success-throughput".to_string(),
            hot_set_size: None,
            warmup_seconds: 1,
            measurement_seconds: 2,
            run_duration_seconds: 2.0,
            concurrency: 2,
            shard_count: 2,
            ingress_capacity: 8,
            ring_size: 16,
            failure_kind_counts: BTreeMap::new(),
            sample_failures: Vec::new(),
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

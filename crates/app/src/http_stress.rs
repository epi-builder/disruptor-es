//! External-process HTTP stress runner and canonical request fixtures.

use std::{
    env,
    io::Read,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow};
use futures::{StreamExt, stream};
use hdrhistogram::Histogram;
use reqwest::{Client, Method, StatusCode};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use sysinfo::System;
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::time::sleep;
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
#[derive(Clone, Debug)]
pub struct HttpStressConfig {
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
    /// Small external-process smoke run suitable for tests and local verification.
    pub fn smoke() -> Self {
        Self {
            command_count: 4,
            concurrency: 2,
            shard_count: 2,
            ingress_capacity: 8,
            ring_size: 16,
        }
    }

    /// Tiny benchmark config so Criterion exercises the lane without long runs.
    pub fn bench() -> Self {
        Self {
            command_count: 2,
            concurrency: 1,
            shard_count: 2,
            ingress_capacity: 8,
            ring_size: 16,
        }
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

#[derive(Debug)]
struct RequestOutcome {
    status: StatusCode,
    latency_micros: u64,
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

    async fn place_order(
        &self,
        request: &CanonicalPlaceOrderRequest,
    ) -> anyhow::Result<RequestOutcome> {
        let started = Instant::now();
        let response = http_request(
            &self.client,
            self.listen_addr,
            Method::POST,
            "/commands/orders/place",
            Some(request),
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
    let harness = ExternalProcessHarness::spawn(&config).await?;
    let sample = Arc::new(Mutex::new(MetricSnapshot::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = spawn_metric_sampler(
        harness.client.clone(),
        harness.prometheus_addr,
        sample.clone(),
        stop.clone(),
    );

    let started = Instant::now();
    let mut latency = Histogram::<u64>::new(3).context("creating HTTP latency histogram")?;
    let mut commands_succeeded = 0_usize;
    let mut commands_rejected = 0_usize;
    let concurrency = config.concurrency.max(1);
    let harness_ref = &harness;

    let results = stream::iter(0..config.command_count)
        .map(|index| {
            let request = canonical_place_order_request("external-http-stress", index);
            async move { harness_ref.place_order(&request).await }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.await;

    for result in results {
        let outcome = result?;
        latency.record(outcome.latency_micros)?;
        match outcome.status {
            StatusCode::OK => commands_succeeded += 1,
            StatusCode::TOO_MANY_REQUESTS => commands_rejected += 1,
            status => {
                return Err(anyhow!(
                    "unexpected external-process stress status {status}"
                ));
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let commands_submitted = config.command_count;
    let reject_rate = if commands_submitted == 0 {
        0.0
    } else {
        commands_rejected as f64 / commands_submitted as f64
    };
    let metrics = sample
        .lock()
        .expect("metric sampler mutex poisoned")
        .clone();
    let mut system = System::new_all();
    sleep(Duration::from_millis(20)).await;
    system.refresh_cpu_all();

    Ok(StressReport {
        scenario: StressScenario::ExternalProcessHttp,
        commands_submitted,
        commands_succeeded,
        commands_rejected,
        throughput_per_second: commands_succeeded as f64 / elapsed,
        p50_micros: percentile(&latency, 50.0),
        p95_micros: percentile(&latency, 95.0),
        p99_micros: percentile(&latency, 99.0),
        max_micros: latency.max(),
        ingress_depth_max: metrics
            .ingress_depth_max
            .max(concurrency.min(commands_submitted)),
        shard_depth_max: metrics.shard_depth_max,
        append_latency_p95_micros: metrics.append_latency_p95_micros,
        projection_lag: metrics.projection_lag,
        outbox_lag: metrics.outbox_lag,
        reject_rate,
        cpu_utilization_percent: system.global_cpu_usage(),
        core_count: system.cpus().len().max(1),
    })
}

fn spawn_metric_sampler(
    client: Client,
    prometheus_addr: SocketAddr,
    sample: Arc<Mutex<MetricSnapshot>>,
    stop: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while !stop.load(Ordering::Relaxed) {
            if let Ok(snapshot) = scrape_metrics(&client, prometheus_addr).await {
                let mut state = sample.lock().expect("metric sampler mutex poisoned");
                state.ingress_depth_max = state.ingress_depth_max.max(snapshot.ingress_depth_max);
                state.shard_depth_max = state.shard_depth_max.max(snapshot.shard_depth_max);
                state.append_latency_p95_micros = snapshot.append_latency_p95_micros;
                state.projection_lag = state.projection_lag.max(snapshot.projection_lag);
                state.outbox_lag = state.outbox_lag.max(snapshot.outbox_lag);
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
}

async fn scrape_metrics(
    client: &Client,
    prometheus_addr: SocketAddr,
) -> anyhow::Result<MetricSnapshot> {
    let response = http_request(
        client,
        prometheus_addr,
        Method::GET,
        "/metrics",
        Option::<&()>::None,
    )
    .await
    .context("requesting Prometheus metrics from app serve child")?;
    let body = response
        .text()
        .await
        .context("reading Prometheus metrics body")?;

    let ingress_depth_max = max_metric_value(&body, "es_ingress_depth").unwrap_or(0.0) as usize;
    let shard_depth_max = max_metric_value(&body, "es_shard_queue_depth").unwrap_or(0.0) as usize;
    let projection_lag = max_metric_value(&body, "es_projection_lag").unwrap_or(0.0) as i64;
    let outbox_lag = max_metric_value(&body, "es_outbox_lag").unwrap_or(0.0) as i64;

    Ok(MetricSnapshot {
        ingress_depth_max,
        shard_depth_max,
        append_latency_p95_micros: histogram_p95_micros(
            &body,
            "es_append_latency_seconds",
            Some(("outcome", "committed")),
        )
        .unwrap_or(0),
        projection_lag,
        outbox_lag,
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
    let debug_dir = current_exe
        .parent()
        .and_then(|deps| deps.parent())
        .context("resolving target directory for app binary")?;
    let binary = debug_dir.join("app");
    if binary.exists() {
        return Ok(binary);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .context("resolving workspace root for app binary build")?
        .to_path_buf();
    let profile = debug_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("debug");
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

    Ok(binary)
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

fn histogram_p95_micros(
    metrics: &str,
    metric_name: &str,
    label_filter: Option<(&str, &str)>,
) -> Option<u64> {
    let count_name = format!("{metric_name}_count");
    let bucket_name = format!("{metric_name}_bucket");
    let total = metrics.lines().find_map(|line| {
        if !line.starts_with(&count_name) || !matches_labels(line, label_filter) {
            return None;
        }
        parse_metric_value(line).map(|value| value as u64)
    })?;
    if total == 0 {
        return Some(0);
    }

    let target = (total as f64 * 0.95).ceil() as u64;
    metrics
        .lines()
        .filter(|line| line.starts_with(&bucket_name))
        .filter(|line| matches_labels(line, label_filter))
        .filter_map(|line| {
            let upper = label_value(line, "le")?;
            if upper == "+Inf" {
                return None;
            }
            let cumulative = parse_metric_value(line)? as u64;
            let seconds = upper.parse::<f64>().ok()?;
            Some((seconds, cumulative))
        })
        .find(|(_, cumulative)| *cumulative >= target)
        .map(|(seconds, _)| (seconds * 1_000_000.0) as u64)
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

#[cfg(test)]
mod tests {
    use super::{
        HttpStressConfig, canonical_place_order_request, run_external_process_http_stress,
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

    #[tokio::test]
    async fn external_process_http_stress_smoke() -> anyhow::Result<()> {
        let report = run_external_process_http_stress(HttpStressConfig::smoke()).await?;

        assert_eq!(StressScenario::ExternalProcessHttp, report.scenario);
        assert!(report.commands_submitted > 0);
        assert_eq!(
            report.commands_submitted,
            report.commands_succeeded + report.commands_rejected
        );
        assert!(report.throughput_per_second >= 0.0);
        assert!(report.p50_micros <= report.p95_micros);
        assert!(report.p95_micros <= report.p99_micros);
        assert!(report.p99_micros <= report.max_micros);
        assert!(report.projection_lag >= 0);
        assert!(report.outbox_lag >= 0);
        assert!((0.0..=1.0).contains(&report.reject_rate));
        assert!(report.core_count > 0);

        Ok(())
    }
}

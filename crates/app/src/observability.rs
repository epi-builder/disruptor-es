//! Application-level observability bootstrap and metric catalog.

use std::net::SocketAddr;

use anyhow::Context;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Runtime observability configuration owned by the app composition layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservabilityConfig {
    /// Logical service name attached to exported telemetry resources.
    pub service_name: String,
    /// Tracing subscriber filter directive, for example `info,es_runtime=debug`.
    pub env_filter: String,
    /// Emit JSON logs instead of compact text logs.
    pub json_logs: bool,
    /// Optional Prometheus scrape listener address.
    pub prometheus_listen: Option<SocketAddr>,
    /// Optional OTLP endpoint for trace export.
    pub otlp_endpoint: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            service_name: "disruptor-es".to_string(),
            env_filter: "info".to_string(),
            json_logs: false,
            prometheus_listen: None,
            otlp_endpoint: None,
        }
    }
}

/// Phase 7 metric names emitted by the runtime, storage, projection, and outbox paths.
pub const PHASE7_METRIC_NAMES: &[&str] = &[
    "es_ingress_depth",
    "es_shard_queue_depth",
    "es_ring_wait_seconds",
    "es_decision_latency_seconds",
    "es_append_latency_seconds",
    "es_occ_conflicts_total",
    "es_dedupe_hits_total",
    "es_projection_lag",
    "es_outbox_lag",
    "es_command_latency_seconds",
    "es_command_total",
    "es_command_rejected_total",
];

/// Metric labels that are intentionally forbidden because they are high-cardinality identifiers.
pub const FORBIDDEN_METRIC_LABELS: &[&str] = &[
    "tenant_id",
    "command_id",
    "correlation_id",
    "causation_id",
    "stream_id",
    "event_id",
    "idempotency_key",
];

/// Bounded metric labels allowed by the Phase 7 instrumentation contract.
pub const ALLOWED_METRIC_LABELS: &[&str] = &[
    "aggregate",
    "outcome",
    "reason",
    "shard",
    "projector",
    "topic",
];

/// Initialize global tracing and metrics exporters for the composed application.
pub fn init_observability(config: ObservabilityConfig) -> anyhow::Result<Option<PrometheusHandle>> {
    let prometheus = if let Some(listen_addr) = config.prometheus_listen {
        Some(
            PrometheusBuilder::new()
                .with_http_listener(listen_addr)
                .install_recorder()
                .context("installing Prometheus metrics recorder")?,
        )
    } else {
        None
    };

    let env_filter = EnvFilter::try_new(config.env_filter.as_str())
        .with_context(|| format!("parsing tracing env filter `{}`", config.env_filter))?;
    if let Some(endpoint) = config.otlp_endpoint {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .build()
            .context("building OTLP span exporter")?;
        let resource = Resource::builder_empty()
            .with_service_name(config.service_name)
            .build();
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();
        if config.json_logs {
            let tracer = provider.tracer("disruptor-es");
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .try_init()
                .context("installing JSON tracing subscriber with OTLP layer")?;
        } else {
            let tracer = provider.tracer("disruptor-es");
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().compact())
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .try_init()
                .context("installing compact tracing subscriber with OTLP layer")?;
        }
    } else if config.json_logs {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json())
            .try_init()
            .context("installing JSON tracing subscriber")?;
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().compact())
            .try_init()
            .context("installing compact tracing subscriber")?;
    }

    Ok(prometheus)
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED_METRIC_LABELS, FORBIDDEN_METRIC_LABELS, PHASE7_METRIC_NAMES};

    #[test]
    fn observability_metrics_catalog_covers_phase7() {
        let required = [
            "es_ingress_depth",
            "es_shard_queue_depth",
            "es_ring_wait_seconds",
            "es_decision_latency_seconds",
            "es_append_latency_seconds",
            "es_occ_conflicts_total",
            "es_dedupe_hits_total",
            "es_projection_lag",
            "es_outbox_lag",
            "es_command_latency_seconds",
            "es_command_total",
            "es_command_rejected_total",
        ];

        for metric_name in required {
            assert!(
                PHASE7_METRIC_NAMES.contains(&metric_name),
                "missing Phase 7 metric: {metric_name}",
            );
        }
    }

    #[test]
    fn observability_metric_labels_are_bounded() {
        let forbidden = [
            "tenant_id",
            "command_id",
            "correlation_id",
            "causation_id",
            "stream_id",
            "event_id",
            "idempotency_key",
        ];

        for label_name in forbidden {
            assert!(
                FORBIDDEN_METRIC_LABELS.contains(&label_name),
                "forbidden label not tracked: {label_name}",
            );
        }

        for label_name in ALLOWED_METRIC_LABELS {
            assert!(
                !FORBIDDEN_METRIC_LABELS.contains(label_name),
                "bounded label marked forbidden: {label_name}",
            );
        }
    }
}

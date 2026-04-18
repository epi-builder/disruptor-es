//! Application-level observability bootstrap and metric catalog.

/// Phase 7 metric names emitted by the runtime, storage, projection, and outbox paths.
pub const PHASE7_METRIC_NAMES: &[&str] = &[];

/// Metric labels that are intentionally forbidden because they are high-cardinality identifiers.
pub const FORBIDDEN_METRIC_LABELS: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::{FORBIDDEN_METRIC_LABELS, PHASE7_METRIC_NAMES};

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
    }
}

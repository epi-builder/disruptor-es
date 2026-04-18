//! Composition binary shell for later service wiring.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Some(command) = std::env::args().nth(1) else {
        println!("usage: app stress-smoke");
        return Ok(());
    };

    if command != "stress-smoke" {
        println!("usage: app stress-smoke");
        return Ok(());
    }

    let report = app::stress::run_single_service_stress(app::stress::StressConfig::smoke()).await?;
    println!(
        "{}",
        serde_json::json!({
            "commands_submitted": report.commands_submitted,
            "commands_succeeded": report.commands_succeeded,
            "commands_rejected": report.commands_rejected,
            "throughput_per_second": report.throughput_per_second,
            "p50_micros": report.p50_micros,
            "p95_micros": report.p95_micros,
            "p99_micros": report.p99_micros,
            "ingress_depth_max": report.ingress_depth_max,
            "shard_depth_max": report.shard_depth_max,
            "append_latency_p95_micros": report.append_latency_p95_micros,
            "projection_lag": report.projection_lag,
            "outbox_lag": report.outbox_lag,
            "reject_rate": report.reject_rate,
            "cpu_utilization_percent": report.cpu_utilization_percent,
            "core_count": report.core_count,
        })
    );

    Ok(())
}

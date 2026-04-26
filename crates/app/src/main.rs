//! Thin binary shell for runnable service and stress-smoke entrypoints.

fn print_stress_report(report: &app::stress::StressReport) {
    println!(
        "{}",
        serde_json::json!({
            "scenario": report.scenario.as_str(),
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("serve") => app::serve::run_from_env().await,
        Some("stress-smoke") => {
            let report =
                app::stress::run_single_service_stress(app::stress::StressConfig::smoke()).await?;
            print_stress_report(&report);
            Ok(())
        }
        Some("http-stress") => {
            let report = app::http_stress::run_external_process_http_stress(
                app::http_stress::HttpStressConfig::smoke(),
            )
            .await?;
            print_stress_report(&report);
            Ok(())
        }
        _ => {
            println!("usage: app serve | app stress-smoke | app http-stress");
            Ok(())
        }
    }
}

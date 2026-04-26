//! Thin binary shell for runnable service and stress-smoke entrypoints.

const HTTP_STRESS_USAGE: &str = "usage: app serve | app stress-smoke | app http-stress [--profile smoke|baseline|burst|hot-key] [--warmup-seconds <u64>] [--measure-seconds <u64>] [--concurrency <usize>] [--command-count <usize>] [--shard-count <usize>] [--ingress-capacity <usize>] [--ring-size <usize>]";

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

#[cfg(test)]
mod tests {
    use app::{
        http_stress::HttpStressProfile,
        stress::{StressReport, StressScenario},
    };

    use super::{HTTP_STRESS_USAGE, parse_http_stress_args, stress_report_json};

    #[test]
    fn http_stress_usage_lists_phase13_flags() {
        assert!(HTTP_STRESS_USAGE.contains("--profile"));
        assert!(HTTP_STRESS_USAGE.contains("--warmup-seconds"));
        assert!(HTTP_STRESS_USAGE.contains("--measure-seconds"));
        assert!(HTTP_STRESS_USAGE.contains("--concurrency"));
        assert!(HTTP_STRESS_USAGE.contains("--command-count"));
        assert!(HTTP_STRESS_USAGE.contains("--shard-count"));
        assert!(HTTP_STRESS_USAGE.contains("--ingress-capacity"));
        assert!(HTTP_STRESS_USAGE.contains("--ring-size"));
    }

    #[test]
    fn http_stress_cli_accepts_profile_and_overrides() {
        let config = parse_http_stress_args([
            "--profile",
            "baseline",
            "--warmup-seconds",
            "5",
            "--measure-seconds",
            "30",
            "--concurrency",
            "8",
            "--command-count",
            "64",
            "--shard-count",
            "4",
            "--ingress-capacity",
            "256",
            "--ring-size",
            "256",
        ])
        .expect("cli args parse");

        assert_eq!("baseline", config.profile.as_str());
        assert_eq!(5, config.warmup_seconds);
        assert_eq!(30, config.measurement_seconds);
        assert_eq!(8, config.concurrency);
        assert_eq!(64, config.command_count);
        assert_eq!(4, config.shard_count);
        assert_eq!(256, config.ingress_capacity);
        assert_eq!(256, config.ring_size);
    }

    #[test]
    fn http_stress_report_includes_phase13_json_fields() {
        let report = StressReport {
            scenario: StressScenario::ExternalProcessHttp,
            commands_submitted: 16,
            commands_succeeded: 12,
            commands_rejected: 3,
            commands_failed: 1,
            throughput_per_second: 6.0,
            p50_micros: 10,
            p95_micros: 20,
            p99_micros: 30,
            max_micros: 40,
            ingress_depth_max: 2,
            shard_depth_max: 1,
            append_latency_p95_micros: 50,
            projection_lag: 0,
            outbox_lag: 0,
            reject_rate: 0.25,
            cpu_utilization_percent: 11.0,
            core_count: 8,
            profile_name: "smoke".to_string(),
            warmup_seconds: 1,
            measurement_seconds: 2,
            run_duration_seconds: 2.0,
            concurrency: 2,
            deadline_policy: "stop-new-requests-then-drain-in-flight".to_string(),
            drain_timeout_seconds: 5,
            host_os: "macos",
            host_arch: "aarch64",
            cpu_brand: "test-cpu".to_string(),
            cpu_usage_samples: vec![10.0, 11.0],
        };

        let json = stress_report_json(&report);

        assert_eq!("smoke", json["profile_name"]);
        assert_eq!(1, json["warmup_seconds"]);
        assert_eq!(2, json["measurement_seconds"]);
        assert_eq!(2.0, json["run_duration_seconds"]);
        assert_eq!(2, json["concurrency"]);
        assert_eq!("stop-new-requests-then-drain-in-flight", json["deadline_policy"]);
        assert_eq!(5, json["drain_timeout_seconds"]);
        assert_eq!(1, json["commands_failed"]);
        assert_eq!("macos", json["host_os"]);
        assert_eq!("aarch64", json["host_arch"]);
        assert_eq!("test-cpu", json["cpu_brand"]);
        assert_eq!(2, json["cpu_usage_samples"].as_array().expect("array").len());
    }

    #[test]
    fn http_stress_cli_rejects_unknown_flags_with_usage() {
        let error = parse_http_stress_args(["--unknown", "value"]).expect_err("unknown flag");
        let message = error.to_string();
        assert!(message.contains("--unknown"));
        assert!(message.contains(HTTP_STRESS_USAGE));
        assert!(message.contains("--profile"));
        assert!(message.contains("--ring-size"));
    }
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

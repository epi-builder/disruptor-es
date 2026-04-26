//! Thin binary shell for runnable service and stress-smoke entrypoints.

use anyhow::{Context, anyhow, bail};

const HTTP_STRESS_USAGE: &str = "usage: app serve | app stress-smoke | app http-stress [--profile smoke|baseline|burst|hot-key] [--workload-shape unique|hot-set|single-hot-key] [--hot-set-size <usize>] [--warmup-seconds <u64>] [--measure-seconds <u64>] [--concurrency <usize>] [--command-count <usize>] [--shard-count <usize>] [--ingress-capacity <usize>] [--ring-size <usize>]";

fn stress_report_json(report: &app::stress::StressReport) -> serde_json::Value {
    serde_json::json!({
        "scenario": report.scenario.as_str(),
        "commands_submitted": report.commands_submitted,
        "commands_succeeded": report.commands_succeeded,
        "commands_rejected": report.commands_rejected,
        "commands_failed": report.commands_failed,
        "throughput_per_second": report.throughput_per_second,
        "p50_micros": report.p50_micros,
        "p95_micros": report.p95_micros,
        "p99_micros": report.p99_micros,
        "max_micros": report.max_micros,
        "ingress_depth_max": report.ingress_depth_max,
        "shard_depth_max": report.shard_depth_max,
        "append_latency_p95_micros": report.append_latency_p95_micros,
        "projection_lag": report.projection_lag,
        "outbox_lag": report.outbox_lag,
        "reject_rate": report.reject_rate,
        "cpu_utilization_percent": report.cpu_utilization_percent,
        "core_count": report.core_count,
        "profile_name": report.profile_name,
        "warmup_seconds": report.warmup_seconds,
        "measurement_seconds": report.measurement_seconds,
        "run_duration_seconds": report.run_duration_seconds,
        "concurrency": report.concurrency,
        "deadline_policy": report.deadline_policy,
        "drain_timeout_seconds": report.drain_timeout_seconds,
        "host_os": report.host_os,
        "host_arch": report.host_arch,
        "cpu_brand": report.cpu_brand,
        "cpu_usage_samples": report.cpu_usage_samples,
    })
}

fn print_stress_report(report: &app::stress::StressReport) {
    println!("{}", stress_report_json(report));
}

fn parse_profile(value: &str) -> anyhow::Result<app::http_stress::HttpStressProfile> {
    match value {
        "smoke" => Ok(app::http_stress::HttpStressProfile::Smoke),
        "baseline" => Ok(app::http_stress::HttpStressProfile::Baseline),
        "burst" => Ok(app::http_stress::HttpStressProfile::Burst),
        "hot-key" => Ok(app::http_stress::HttpStressProfile::HotKey),
        _ => bail!("invalid value for --profile: {value}\n{HTTP_STRESS_USAGE}"),
    }
}

fn parse_workload_shape(
    value: &str,
    hot_set_size: Option<usize>,
) -> anyhow::Result<app::http_stress::HttpWorkloadShape> {
    match value {
        "unique" => Ok(app::http_stress::HttpWorkloadShape::Unique),
        "hot-set" => Ok(app::http_stress::HttpWorkloadShape::HotSet(
            hot_set_size.unwrap_or(8),
        )),
        "single-hot-key" => Ok(app::http_stress::HttpWorkloadShape::SingleHotKey),
        _ => bail!("invalid value for --workload-shape: {value}\n{HTTP_STRESS_USAGE}"),
    }
}

fn parse_usize_flag(flag: &str, value: &str) -> anyhow::Result<usize> {
    value
        .parse::<usize>()
        .with_context(|| format!("invalid value for {flag}: {value}\n{HTTP_STRESS_USAGE}"))
}

fn parse_u64_flag(flag: &str, value: &str) -> anyhow::Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("invalid value for {flag}: {value}\n{HTTP_STRESS_USAGE}"))
}

fn parse_http_stress_args<I, S>(args: I) -> anyhow::Result<app::http_stress::HttpStressConfig>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut remaining = args.into_iter();
    let mut config = app::http_stress::HttpStressConfig::smoke();
    let mut workload_shape_override: Option<String> = None;
    let mut hot_set_size_override: Option<usize> = None;

    while let Some(flag) = remaining.next() {
        let flag = flag.as_ref();
        let raw_value = remaining
            .next()
            .ok_or_else(|| anyhow!("missing value for {flag}\n{HTTP_STRESS_USAGE}"))?;
        let value = raw_value.as_ref();

        match flag {
            "--profile" => {
                config = app::http_stress::HttpStressConfig::from_profile(parse_profile(value)?);
            }
            "--workload-shape" => workload_shape_override = Some(value.to_string()),
            "--hot-set-size" => hot_set_size_override = Some(parse_usize_flag(flag, value)?),
            "--warmup-seconds" => config.warmup_seconds = parse_u64_flag(flag, value)?,
            "--measure-seconds" => config.measurement_seconds = parse_u64_flag(flag, value)?,
            "--concurrency" => config.concurrency = parse_usize_flag(flag, value)?,
            "--command-count" => config.command_count = parse_usize_flag(flag, value)?,
            "--shard-count" => config.shard_count = parse_usize_flag(flag, value)?,
            "--ingress-capacity" => config.ingress_capacity = parse_usize_flag(flag, value)?,
            "--ring-size" => config.ring_size = parse_usize_flag(flag, value)?,
            _ => bail!("unknown flag: {flag}\n{HTTP_STRESS_USAGE}"),
        }
    }

    if let Some(raw_shape) = workload_shape_override.as_deref() {
        config.workload_shape = parse_workload_shape(raw_shape, hot_set_size_override)?;
        config.hot_set_size = match (config.workload_shape, hot_set_size_override) {
            (app::http_stress::HttpWorkloadShape::HotSet(size), Some(explicit)) => {
                Some(explicit.max(size))
            }
            (app::http_stress::HttpWorkloadShape::HotSet(size), None) => Some(size),
            (
                app::http_stress::HttpWorkloadShape::Unique
                | app::http_stress::HttpWorkloadShape::SingleHotKey,
                explicit,
            ) => explicit,
        };
    } else if let Some(hot_set_size) = hot_set_size_override {
        config.workload_shape = app::http_stress::HttpWorkloadShape::HotSet(hot_set_size);
        config.hot_set_size = Some(hot_set_size);
    }

    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use app::stress::{StressReport, StressScenario};

    use super::{HTTP_STRESS_USAGE, parse_http_stress_args, stress_report_json};

    #[test]
    fn http_stress_usage_lists_phase13_flags() {
        assert!(HTTP_STRESS_USAGE.contains("--profile"));
        assert!(HTTP_STRESS_USAGE.contains("--workload-shape"));
        assert!(HTTP_STRESS_USAGE.contains("--hot-set-size"));
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
    fn http_stress_cli_accepts_hot_set_workload_shape() {
        let config = parse_http_stress_args([
            "--profile",
            "smoke",
            "--workload-shape",
            "hot-set",
            "--hot-set-size",
            "8",
        ])
        .expect("cli args parse");

        assert_eq!(app::http_stress::HttpWorkloadShape::HotSet(8), config.workload_shape);
        assert_eq!(Some(8), config.hot_set_size);
    }

    #[test]
    fn http_stress_cli_accepts_single_hot_key_workload_shape() {
        let config = parse_http_stress_args([
            "--profile",
            "smoke",
            "--workload-shape",
            "single-hot-key",
        ])
        .expect("cli args parse");

        assert_eq!(
            app::http_stress::HttpWorkloadShape::SingleHotKey,
            config.workload_shape
        );
        assert_eq!(None, config.hot_set_size);
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
        assert_eq!(
            "stop-new-requests-then-drain-in-flight",
            json["deadline_policy"]
        );
        assert_eq!(5, json["drain_timeout_seconds"]);
        assert_eq!(1, json["commands_failed"]);
        assert_eq!("macos", json["host_os"]);
        assert_eq!("aarch64", json["host_arch"]);
        assert_eq!("test-cpu", json["cpu_brand"]);
        assert_eq!(
            2,
            json["cpu_usage_samples"].as_array().expect("array").len()
        );
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
    let mut args = std::env::args();
    let _ = args.next();

    match args.next().as_deref() {
        Some("serve") => app::serve::run_from_env().await,
        Some("stress-smoke") => {
            let report =
                app::stress::run_single_service_stress(app::stress::StressConfig::smoke()).await?;
            print_stress_report(&report);
            Ok(())
        }
        Some("http-stress") => {
            let config = parse_http_stress_args(args)?;
            let report = app::http_stress::run_external_process_http_stress(config).await?;
            print_stress_report(&report);
            Ok(())
        }
        _ => {
            println!("{HTTP_STRESS_USAGE}");
            Ok(())
        }
    }
}

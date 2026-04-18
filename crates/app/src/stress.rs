//! Single-service integrated stress runner.

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

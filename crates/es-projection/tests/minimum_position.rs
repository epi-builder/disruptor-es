//! Minimum-position projection contract tests.

use std::{future::Future, pin::Pin, time::Duration};

use es_projection::{
    CatchUpOutcome, FreshnessCheck, MinimumGlobalPosition, ProjectionBatchLimit, ProjectionError,
    ProjectionResult, ProjectorName, WaitPolicy, wait_for_minimum_position,
};

#[test]
fn minimum_position_rejects_empty_projector_name() {
    let error = ProjectorName::new("").expect_err("empty projector name must fail");

    assert_eq!(ProjectionError::InvalidProjectorName, error);
}

#[test]
fn minimum_position_rejects_negative_required_position() {
    let error = MinimumGlobalPosition::new(-1).expect_err("negative minimum position must fail");

    assert_eq!(ProjectionError::InvalidGlobalPosition { value: -1 }, error);
}

#[test]
fn minimum_position_rejects_invalid_batch_limits() {
    let zero = ProjectionBatchLimit::new(0).expect_err("zero batch limit must fail");
    let too_large = ProjectionBatchLimit::new(1001).expect_err("oversized batch limit must fail");

    assert_eq!(ProjectionError::InvalidBatchLimit { value: 0 }, zero);
    assert_eq!(
        ProjectionError::InvalidBatchLimit { value: 1001 },
        too_large
    );
}

#[test]
fn minimum_position_catch_up_outcome_exposes_applied_fields() {
    let outcome = CatchUpOutcome::Applied {
        event_count: 2,
        last_global_position: 7,
    };

    match outcome {
        CatchUpOutcome::Applied {
            event_count,
            last_global_position,
        } => {
            assert_eq!(2, event_count);
            assert_eq!(7, last_global_position);
        }
        CatchUpOutcome::Idle => panic!("expected applied outcome"),
    }
}

#[test]
fn minimum_position_freshness_compare_reports_fresh_position() {
    let required = MinimumGlobalPosition::new(7).expect("minimum position");

    assert_eq!(
        FreshnessCheck::Fresh { actual: 10 },
        FreshnessCheck::compare(required, 10)
    );
}

#[test]
fn minimum_position_freshness_compare_reports_lagging_position() {
    let required = MinimumGlobalPosition::new(7).expect("minimum position");

    assert_eq!(
        FreshnessCheck::Lagging {
            required: 7,
            actual: 3
        },
        FreshnessCheck::compare(required, 3)
    );
}

#[tokio::test]
async fn minimum_position_zero_timeout_returns_projection_lag() {
    let required = MinimumGlobalPosition::new(7).expect("minimum position");
    let policy = WaitPolicy::new(Duration::ZERO, Duration::from_millis(1)).expect("wait policy");
    let result = wait_for_minimum_position(required, policy, position_loader(3)).await;

    assert_eq!(
        ProjectionError::ProjectionLag {
            required: 7,
            actual: 3
        },
        result.expect_err("lagging wait should fail at deadline")
    );
}

#[test]
fn minimum_position_wait_policy_rejects_poll_interval_greater_than_timeout() {
    let error = WaitPolicy::new(Duration::from_millis(5), Duration::from_millis(10))
        .expect_err("poll interval greater than timeout must fail");

    assert_eq!(ProjectionError::InvalidBatchLimit { value: 10 }, error);
}

fn position_loader(
    actual: i64,
) -> impl FnMut() -> Pin<Box<dyn Future<Output = ProjectionResult<i64>> + Send>> {
    move || Box::pin(async move { Ok(actual) })
}

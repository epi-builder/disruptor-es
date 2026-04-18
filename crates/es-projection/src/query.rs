use std::{future::Future, pin::Pin, time::Duration};

use tokio::time::Instant;

use crate::{MinimumGlobalPosition, ProjectionError, ProjectionResult};

/// Bounded wait policy for minimum-position query freshness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaitPolicy {
    /// Maximum time to wait for projection freshness.
    pub timeout: Duration,
    /// Time between freshness checks.
    pub poll_interval: Duration,
}

impl WaitPolicy {
    /// Creates a wait policy.
    pub fn new(timeout: Duration, poll_interval: Duration) -> ProjectionResult<Self> {
        if poll_interval.is_zero() {
            return Err(ProjectionError::InvalidBatchLimit { value: 0 });
        }
        if !timeout.is_zero() && poll_interval > timeout {
            return Err(ProjectionError::InvalidBatchLimit {
                value: duration_millis_i64(poll_interval),
            });
        }

        Ok(Self {
            timeout,
            poll_interval,
        })
    }
}

/// Result of comparing required and actual projection freshness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessCheck {
    /// Projection is fresh enough.
    Fresh {
        /// Actual global position observed.
        actual: i64,
    },
    /// Projection has not reached the requested position.
    Lagging {
        /// Required global position.
        required: i64,
        /// Actual global position observed.
        actual: i64,
    },
}

impl FreshnessCheck {
    /// Compares a required and actual global position.
    pub fn compare(required: MinimumGlobalPosition, actual: i64) -> Self {
        if actual >= required.value() {
            Self::Fresh { actual }
        } else {
            Self::Lagging {
                required: required.value(),
                actual,
            }
        }
    }
}

/// Waits until the loaded projection position reaches the required minimum.
pub async fn wait_for_minimum_position<F>(
    required: MinimumGlobalPosition,
    policy: WaitPolicy,
    load_actual: F,
) -> ProjectionResult<FreshnessCheck>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = ProjectionResult<i64>> + Send>>,
{
    let mut load_actual = load_actual;
    let deadline = Instant::now() + policy.timeout;

    loop {
        let actual = load_actual().await?;
        match FreshnessCheck::compare(required, actual) {
            fresh @ FreshnessCheck::Fresh { .. } => return Ok(fresh),
            FreshnessCheck::Lagging { required, actual } if Instant::now() >= deadline => {
                return Err(ProjectionError::ProjectionLag { required, actual });
            }
            FreshnessCheck::Lagging { .. } => {
                tokio::time::sleep(policy.poll_interval).await;
            }
        }
    }
}

fn duration_millis_i64(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

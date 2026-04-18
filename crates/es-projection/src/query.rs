use std::{future::Future, pin::Pin, time::Duration};

use crate::{MinimumGlobalPosition, ProjectionResult};

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
    pub const fn new(timeout: Duration, poll_interval: Duration) -> ProjectionResult<Self> {
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
    _policy: WaitPolicy,
    load_actual: F,
) -> ProjectionResult<FreshnessCheck>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = ProjectionResult<i64>> + Send>>,
{
    let mut load_actual = load_actual;
    let actual = load_actual().await?;
    Ok(FreshnessCheck::compare(required, actual))
}

use es_core::TenantId;
use serde::{Deserialize, Serialize};

use crate::ProjectionResult;

/// Validated projector identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ProjectorName(String);

impl ProjectorName {
    /// Creates a projector name.
    pub fn new(value: impl Into<String>) -> ProjectionResult<Self> {
        Ok(Self(value.into()))
    }

    /// Returns the borrowed projector name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Minimum global position requested by a query caller.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MinimumGlobalPosition(i64);

impl MinimumGlobalPosition {
    /// Creates a minimum global position.
    pub fn new(value: i64) -> ProjectionResult<Self> {
        Ok(Self(value))
    }

    /// Returns the numeric global position.
    pub const fn value(self) -> i64 {
        self.0
    }
}

/// Bounded event batch size for projector catch-up.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectionBatchLimit(i64);

impl ProjectionBatchLimit {
    /// Creates a projection batch limit.
    pub fn new(value: i64) -> ProjectionResult<Self> {
        Ok(Self(value))
    }

    /// Returns the numeric batch limit.
    pub const fn value(self) -> i64 {
        self.0
    }
}

/// Tenant-scoped durable offset for one projector.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectorOffset {
    /// Tenant that owns this projector checkpoint.
    pub tenant_id: TenantId,
    /// Projector identity.
    pub projector_name: ProjectorName,
    /// Last committed event global position applied by this projector.
    pub last_global_position: i64,
}

impl ProjectorOffset {
    /// Creates a tenant-scoped projector offset.
    pub fn new(
        tenant_id: TenantId,
        projector_name: ProjectorName,
        last_global_position: i64,
    ) -> ProjectionResult<Self> {
        Ok(Self {
            tenant_id,
            projector_name,
            last_global_position,
        })
    }
}

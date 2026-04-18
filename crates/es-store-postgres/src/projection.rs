//! PostgreSQL-backed CQRS projection storage and read-model queries.

use es_core::TenantId;

/// PostgreSQL projection repository.
#[derive(Clone, Debug)]
pub struct PostgresProjectionStore {
    pool: sqlx::PgPool,
}

impl PostgresProjectionStore {
    /// Creates a projection repository backed by the provided PostgreSQL pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Returns the underlying PostgreSQL pool.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

/// Denormalized order summary read model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSummaryReadModel {
    pub tenant_id: TenantId,
    pub order_id: String,
    pub user_id: String,
    pub status: String,
    pub line_count: i32,
    pub total_quantity: i32,
    pub rejection_reason: Option<String>,
    pub last_applied_global_position: i64,
}

/// Denormalized product inventory read model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductInventoryReadModel {
    pub tenant_id: TenantId,
    pub product_id: String,
    pub sku: String,
    pub name: String,
    pub available_quantity: i32,
    pub reserved_quantity: i32,
    pub last_applied_global_position: i64,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use es_projection::{ProjectionError, ProjectionEvent};
    use serde_json::json;

    fn event(event_type: &str, payload: serde_json::Value) -> ProjectionEvent {
        ProjectionEvent {
            global_position: 1,
            event_type: event_type.to_owned(),
            schema_version: 1,
            payload,
            metadata: json!({}),
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
        }
    }

    #[test]
    fn handled_projection_decode_rejects_malformed_payload() {
        let error = decode_order_event(&event("OrderPlaced", json!({ "not": "an order" })))
            .expect_err("malformed order payload rejected");

        assert_eq!(
            ProjectionError::PayloadDecode {
                event_type: "OrderPlaced".to_owned(),
                schema_version: 1,
            },
            error
        );
    }

    #[test]
    fn unknown_projection_events_are_ignored() {
        assert!(decode_order_event(&event("OtherEvent", json!({}))).is_ok());
        assert!(decode_product_event(&event("OtherEvent", json!({}))).is_ok());
    }
}

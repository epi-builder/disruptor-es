//! PostgreSQL-backed CQRS projection storage and read-model queries.

use std::{future::Future, pin::Pin};

use es_core::TenantId;
use es_projection::{
    CatchUpOutcome, MinimumGlobalPosition, ProjectionBatchLimit, ProjectionError, ProjectionEvent,
    ProjectionResult, ProjectorName, ProjectorOffset, WaitPolicy, wait_for_minimum_position,
};
use example_commerce::{OrderEvent, ProductEvent};
use metrics::{gauge, histogram};
use sqlx::{Postgres, Transaction};
use tracing::info_span;

use crate::{PostgresEventStore, StoreError, StoredEvent};

/// PostgreSQL projection repository.
#[derive(Clone, Debug)]
pub struct PostgresProjectionStore {
    pool: sqlx::PgPool,
    event_store: PostgresEventStore,
}

impl PostgresProjectionStore {
    /// Creates a projection repository backed by the provided PostgreSQL pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            event_store: PostgresEventStore::new(pool.clone()),
            pool,
        }
    }

    /// Returns the underlying PostgreSQL pool.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Loads the saved tenant-scoped offset for a projector.
    pub async fn projector_offset(
        &self,
        tenant_id: &TenantId,
        projector_name: &ProjectorName,
    ) -> ProjectionResult<Option<ProjectorOffset>> {
        let position = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT last_global_position
            FROM projector_offsets
            WHERE tenant_id = $1 AND projector_name = $2
            "#,
        )
        .bind(tenant_id.as_str())
        .bind(projector_name.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(projection_store_error)?;

        position
            .map(|last_global_position| {
                ProjectorOffset::new(
                    tenant_id.clone(),
                    projector_name.clone(),
                    last_global_position,
                )
            })
            .transpose()
    }

    /// Applies committed events after the saved projector offset.
    pub async fn catch_up(
        &self,
        tenant_id: &TenantId,
        projector_name: &ProjectorName,
        limit: ProjectionBatchLimit,
    ) -> ProjectionResult<CatchUpOutcome> {
        let started_at = std::time::Instant::now();
        let span = info_span!(
            "projection.catch_up",
            tenant_id = %tenant_id.as_str(),
            projector = %projector_name.as_str(),
            global_position = tracing::field::Empty,
        );
        let _entered = span.enter();
        let current_offset = self
            .projector_offset(tenant_id, projector_name)
            .await?
            .map(|offset| offset.last_global_position)
            .unwrap_or(0);

        let stored_events = self
            .event_store
            .read_global(tenant_id, current_offset, limit.value())
            .await
            .map_err(store_error)?;
        if stored_events.is_empty() {
            gauge!("es_projection_lag", "projector" => projector_name.as_str().to_owned()).set(0.0);
            return Ok(CatchUpOutcome::Idle);
        }

        let events = stored_events
            .into_iter()
            .map(projection_event_from_stored)
            .collect::<Vec<_>>();
        let last_global_position = events
            .last()
            .expect("non-empty projection batch")
            .global_position;
        span.record("global_position", last_global_position);
        gauge!("es_projection_lag", "projector" => projector_name.as_str().to_owned())
            .set((last_global_position - current_offset).max(0) as f64);

        let mut tx = self.pool.begin().await.map_err(projection_store_error)?;
        let apply_result = async {
            for event in &events {
                apply_projection_event(&mut tx, event).await?;
            }
            upsert_projector_offset(&mut tx, tenant_id, projector_name, last_global_position).await
        }
        .await;
        if let Err(error) = apply_result {
            tx.rollback().await.map_err(projection_store_error)?;
            return Err(error);
        }
        tx.commit().await.map_err(projection_store_error)?;
        histogram!("es_projection_catch_up_seconds", "projector" => projector_name.as_str().to_owned())
            .record(started_at.elapsed().as_secs_f64());

        Ok(CatchUpOutcome::Applied {
            event_count: events.len(),
            last_global_position,
        })
    }

    /// Loads an order summary, optionally waiting for read-model freshness.
    pub async fn order_summary(
        &self,
        tenant_id: &TenantId,
        order_id: &str,
        minimum_global_position: Option<MinimumGlobalPosition>,
        wait_policy: Option<WaitPolicy>,
    ) -> ProjectionResult<Option<OrderSummaryReadModel>> {
        if let Some(required) = minimum_global_position {
            let policy = wait_policy.unwrap_or_else(default_wait_policy);
            let pool = self.pool.clone();
            let tenant_id = tenant_id.clone();
            let order_id = order_id.to_owned();
            wait_for_minimum_position(required, policy, move || {
                let pool = pool.clone();
                let tenant_id = tenant_id.clone();
                let order_id = order_id.clone();
                Box::pin(async move { order_summary_position(&pool, &tenant_id, &order_id).await })
                    as Pin<Box<dyn Future<Output = ProjectionResult<i64>> + Send>>
            })
            .await?;
        }

        select_order_summary(&self.pool, tenant_id, order_id).await
    }

    /// Loads product inventory, optionally waiting for read-model freshness.
    pub async fn product_inventory(
        &self,
        tenant_id: &TenantId,
        product_id: &str,
        minimum_global_position: Option<MinimumGlobalPosition>,
        wait_policy: Option<WaitPolicy>,
    ) -> ProjectionResult<Option<ProductInventoryReadModel>> {
        if let Some(required) = minimum_global_position {
            let policy = wait_policy.unwrap_or_else(default_wait_policy);
            let pool = self.pool.clone();
            let tenant_id = tenant_id.clone();
            let product_id = product_id.to_owned();
            wait_for_minimum_position(required, policy, move || {
                let pool = pool.clone();
                let tenant_id = tenant_id.clone();
                let product_id = product_id.clone();
                Box::pin(
                    async move { product_inventory_position(&pool, &tenant_id, &product_id).await },
                ) as Pin<Box<dyn Future<Output = ProjectionResult<i64>> + Send>>
            })
            .await?;
        }

        select_product_inventory(&self.pool, tenant_id, product_id).await
    }
}

/// Denormalized order summary read model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderSummaryReadModel {
    /// Tenant that owns the row.
    pub tenant_id: TenantId,
    /// Order identity.
    pub order_id: String,
    /// User identity that owns the order.
    pub user_id: String,
    /// Current order lifecycle status.
    pub status: String,
    /// Number of order lines.
    pub line_count: i32,
    /// Total quantity across order lines.
    pub total_quantity: i32,
    /// Optional rejection reason.
    pub rejection_reason: Option<String>,
    /// Last event global position applied to this row.
    pub last_applied_global_position: i64,
}

/// Denormalized product inventory read model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductInventoryReadModel {
    /// Tenant that owns the row.
    pub tenant_id: TenantId,
    /// Product identity.
    pub product_id: String,
    /// Product stock-keeping unit.
    pub sku: String,
    /// Product display name.
    pub name: String,
    /// Available quantity.
    pub available_quantity: i32,
    /// Reserved quantity.
    pub reserved_quantity: i32,
    /// Last event global position applied to this row.
    pub last_applied_global_position: i64,
}

async fn apply_projection_event(
    tx: &mut Transaction<'_, Postgres>,
    event: &ProjectionEvent,
) -> ProjectionResult<()> {
    decode_order_event(event)?;
    decode_product_event(event)?;

    match &event.event_type[..] {
        "OrderPlaced" => {
            let OrderEvent::OrderPlaced {
                order_id,
                user_id,
                lines,
            } = decode_order_payload(event)?
            else {
                unreachable!("event_type matched OrderPlaced");
            };
            let line_count = i32::try_from(lines.len()).unwrap_or(i32::MAX);
            let total_quantity = lines.iter().fold(0_i32, |total, line| {
                total.saturating_add(i32::try_from(line.quantity.value()).unwrap_or(i32::MAX))
            });
            upsert_order_summary(
                tx,
                event,
                order_id.as_str(),
                user_id.as_str(),
                "Placed",
                line_count,
                total_quantity,
                None,
            )
            .await
        }
        "OrderConfirmed" => {
            let OrderEvent::OrderConfirmed { order_id } = decode_order_payload(event)? else {
                unreachable!("event_type matched OrderConfirmed");
            };
            update_order_status(tx, event, order_id.as_str(), "Confirmed", None).await
        }
        "OrderRejected" => {
            let OrderEvent::OrderRejected { order_id, reason } = decode_order_payload(event)?
            else {
                unreachable!("event_type matched OrderRejected");
            };
            update_order_status(tx, event, order_id.as_str(), "Rejected", Some(reason)).await
        }
        "OrderCancelled" => {
            let OrderEvent::OrderCancelled { order_id } = decode_order_payload(event)? else {
                unreachable!("event_type matched OrderCancelled");
            };
            update_order_status(tx, event, order_id.as_str(), "Cancelled", None).await
        }
        "ProductCreated" => {
            let ProductEvent::ProductCreated {
                product_id,
                sku,
                name,
                initial_quantity,
            } = decode_product_payload(event)?
            else {
                unreachable!("event_type matched ProductCreated");
            };
            upsert_product_inventory(
                tx,
                event,
                product_id.as_str(),
                sku.as_str(),
                &name,
                i32::try_from(initial_quantity.value()).unwrap_or(i32::MAX),
                0,
            )
            .await
        }
        "InventoryAdjusted" => {
            let ProductEvent::InventoryAdjusted { product_id, delta } =
                decode_product_payload(event)?
            else {
                unreachable!("event_type matched InventoryAdjusted");
            };
            update_product_inventory(tx, event, product_id.as_str(), delta, 0).await
        }
        "InventoryReserved" => {
            let ProductEvent::InventoryReserved {
                product_id,
                quantity,
            } = decode_product_payload(event)?
            else {
                unreachable!("event_type matched InventoryReserved");
            };
            let quantity = i32::try_from(quantity.value()).unwrap_or(i32::MAX);
            update_product_inventory(tx, event, product_id.as_str(), -quantity, quantity).await
        }
        "InventoryReleased" => {
            let ProductEvent::InventoryReleased {
                product_id,
                quantity,
            } = decode_product_payload(event)?
            else {
                unreachable!("event_type matched InventoryReleased");
            };
            let quantity = i32::try_from(quantity.value()).unwrap_or(i32::MAX);
            update_product_inventory(tx, event, product_id.as_str(), quantity, -quantity).await
        }
        _ => Ok(()),
    }
}

fn decode_order_event(event: &ProjectionEvent) -> ProjectionResult<()> {
    if is_order_event(&event.event_type) {
        decode_order_payload(event).map(|_| ())
    } else {
        Ok(())
    }
}

fn decode_product_event(event: &ProjectionEvent) -> ProjectionResult<()> {
    if is_product_event(&event.event_type) {
        decode_product_payload(event).map(|_| ())
    } else {
        Ok(())
    }
}

fn decode_order_payload(event: &ProjectionEvent) -> ProjectionResult<OrderEvent> {
    serde_json::from_value(event.payload.clone()).map_err(|_| ProjectionError::PayloadDecode {
        event_type: event.event_type.clone(),
        schema_version: event.schema_version,
    })
}

fn decode_product_payload(event: &ProjectionEvent) -> ProjectionResult<ProductEvent> {
    serde_json::from_value(event.payload.clone()).map_err(|_| ProjectionError::PayloadDecode {
        event_type: event.event_type.clone(),
        schema_version: event.schema_version,
    })
}

fn is_order_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "OrderPlaced" | "OrderConfirmed" | "OrderRejected" | "OrderCancelled"
    )
}

fn is_product_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "ProductCreated" | "InventoryAdjusted" | "InventoryReserved" | "InventoryReleased"
    )
}

async fn upsert_order_summary(
    tx: &mut Transaction<'_, Postgres>,
    event: &ProjectionEvent,
    order_id: &str,
    user_id: &str,
    status: &str,
    line_count: i32,
    total_quantity: i32,
    rejection_reason: Option<String>,
) -> ProjectionResult<()> {
    sqlx::query(
        r#"
        INSERT INTO order_summary_read_models (
            tenant_id,
            order_id,
            user_id,
            status,
            line_count,
            total_quantity,
            rejection_reason,
            last_applied_global_position
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (tenant_id, order_id) DO UPDATE
        SET user_id = EXCLUDED.user_id,
            status = EXCLUDED.status,
            line_count = EXCLUDED.line_count,
            total_quantity = EXCLUDED.total_quantity,
            rejection_reason = EXCLUDED.rejection_reason,
            last_applied_global_position = GREATEST(
                order_summary_read_models.last_applied_global_position,
                EXCLUDED.last_applied_global_position
            ),
            updated_at = now()
        WHERE order_summary_read_models.last_applied_global_position <= EXCLUDED.last_applied_global_position
        "#,
    )
    .bind(event.tenant_id.as_str())
    .bind(order_id)
    .bind(user_id)
    .bind(status)
    .bind(line_count)
    .bind(total_quantity)
    .bind(rejection_reason)
    .bind(event.global_position)
    .execute(&mut **tx)
    .await
    .map_err(projection_store_error)?;

    Ok(())
}

async fn update_order_status(
    tx: &mut Transaction<'_, Postgres>,
    event: &ProjectionEvent,
    order_id: &str,
    status: &str,
    rejection_reason: Option<String>,
) -> ProjectionResult<()> {
    sqlx::query(
        r#"
        UPDATE order_summary_read_models
        SET status = $3,
            rejection_reason = $4,
            last_applied_global_position = $5,
            updated_at = now()
        WHERE tenant_id = $1
          AND order_id = $2
          AND last_applied_global_position < $5
        "#,
    )
    .bind(event.tenant_id.as_str())
    .bind(order_id)
    .bind(status)
    .bind(rejection_reason)
    .bind(event.global_position)
    .execute(&mut **tx)
    .await
    .map_err(projection_store_error)?;

    Ok(())
}

async fn upsert_product_inventory(
    tx: &mut Transaction<'_, Postgres>,
    event: &ProjectionEvent,
    product_id: &str,
    sku: &str,
    name: &str,
    available_quantity: i32,
    reserved_quantity: i32,
) -> ProjectionResult<()> {
    sqlx::query(
        r#"
        INSERT INTO product_inventory_read_models (
            tenant_id,
            product_id,
            sku,
            name,
            available_quantity,
            reserved_quantity,
            last_applied_global_position
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (tenant_id, product_id) DO UPDATE
        SET sku = EXCLUDED.sku,
            name = EXCLUDED.name,
            available_quantity = EXCLUDED.available_quantity,
            reserved_quantity = EXCLUDED.reserved_quantity,
            last_applied_global_position = GREATEST(
                product_inventory_read_models.last_applied_global_position,
                EXCLUDED.last_applied_global_position
            ),
            updated_at = now()
        WHERE product_inventory_read_models.last_applied_global_position <= EXCLUDED.last_applied_global_position
        "#,
    )
    .bind(event.tenant_id.as_str())
    .bind(product_id)
    .bind(sku)
    .bind(name)
    .bind(available_quantity)
    .bind(reserved_quantity)
    .bind(event.global_position)
    .execute(&mut **tx)
    .await
    .map_err(projection_store_error)?;

    Ok(())
}

async fn update_product_inventory(
    tx: &mut Transaction<'_, Postgres>,
    event: &ProjectionEvent,
    product_id: &str,
    available_delta: i32,
    reserved_delta: i32,
) -> ProjectionResult<()> {
    sqlx::query(
        r#"
        UPDATE product_inventory_read_models
        SET available_quantity = available_quantity + $3,
            reserved_quantity = reserved_quantity + $4,
            last_applied_global_position = $5,
            updated_at = now()
        WHERE tenant_id = $1
          AND product_id = $2
          AND last_applied_global_position < $5
        "#,
    )
    .bind(event.tenant_id.as_str())
    .bind(product_id)
    .bind(available_delta)
    .bind(reserved_delta)
    .bind(event.global_position)
    .execute(&mut **tx)
    .await
    .map_err(projection_store_error)?;

    Ok(())
}

async fn upsert_projector_offset(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    projector_name: &ProjectorName,
    last_global_position: i64,
) -> ProjectionResult<()> {
    sqlx::query(
        r#"
        INSERT INTO projector_offsets (
            tenant_id,
            projector_name,
            last_global_position
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (tenant_id, projector_name) DO UPDATE
        SET last_global_position = GREATEST(
                projector_offsets.last_global_position,
                EXCLUDED.last_global_position
            ),
            updated_at = now()
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(projector_name.as_str())
    .bind(last_global_position)
    .execute(&mut **tx)
    .await
    .map_err(projection_store_error)?;

    Ok(())
}

async fn select_order_summary(
    pool: &sqlx::PgPool,
    tenant_id: &TenantId,
    order_id: &str,
) -> ProjectionResult<Option<OrderSummaryReadModel>> {
    let row = sqlx::query_as::<_, OrderSummaryRow>(
        r#"
        SELECT
            tenant_id,
            order_id,
            user_id,
            status,
            line_count,
            total_quantity,
            rejection_reason,
            last_applied_global_position
        FROM order_summary_read_models
        WHERE tenant_id = $1 AND order_id = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(order_id)
    .fetch_optional(pool)
    .await
    .map_err(projection_store_error)?;

    row.map(TryInto::try_into).transpose()
}

async fn select_product_inventory(
    pool: &sqlx::PgPool,
    tenant_id: &TenantId,
    product_id: &str,
) -> ProjectionResult<Option<ProductInventoryReadModel>> {
    let row = sqlx::query_as::<_, ProductInventoryRow>(
        r#"
        SELECT
            tenant_id,
            product_id,
            sku,
            name,
            available_quantity,
            reserved_quantity,
            last_applied_global_position
        FROM product_inventory_read_models
        WHERE tenant_id = $1 AND product_id = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .map_err(projection_store_error)?;

    row.map(TryInto::try_into).transpose()
}

async fn order_summary_position(
    pool: &sqlx::PgPool,
    tenant_id: &TenantId,
    order_id: &str,
) -> ProjectionResult<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT last_applied_global_position
        FROM order_summary_read_models
        WHERE tenant_id = $1 AND order_id = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(order_id)
    .fetch_optional(pool)
    .await
    .map(|position| position.unwrap_or(0))
    .map_err(projection_store_error)
}

async fn product_inventory_position(
    pool: &sqlx::PgPool,
    tenant_id: &TenantId,
    product_id: &str,
) -> ProjectionResult<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT last_applied_global_position
        FROM product_inventory_read_models
        WHERE tenant_id = $1 AND product_id = $2
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(product_id)
    .fetch_optional(pool)
    .await
    .map(|position| position.unwrap_or(0))
    .map_err(projection_store_error)
}

fn projection_event_from_stored(event: StoredEvent) -> ProjectionEvent {
    ProjectionEvent {
        global_position: event.global_position,
        event_type: event.event_type,
        schema_version: event.schema_version,
        payload: event.payload,
        metadata: event.metadata,
        tenant_id: event.tenant_id,
    }
}

fn default_wait_policy() -> WaitPolicy {
    WaitPolicy::new(
        std::time::Duration::from_millis(250),
        std::time::Duration::from_millis(10),
    )
    .expect("default wait policy is valid")
}

fn store_error(error: StoreError) -> ProjectionError {
    ProjectionError::Store {
        message: error.to_string(),
    }
}

fn projection_store_error(error: sqlx::Error) -> ProjectionError {
    ProjectionError::Store {
        message: error.to_string(),
    }
}

#[derive(sqlx::FromRow)]
struct OrderSummaryRow {
    tenant_id: String,
    order_id: String,
    user_id: String,
    status: String,
    line_count: i32,
    total_quantity: i32,
    rejection_reason: Option<String>,
    last_applied_global_position: i64,
}

impl TryFrom<OrderSummaryRow> for OrderSummaryReadModel {
    type Error = ProjectionError;

    fn try_from(row: OrderSummaryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            tenant_id: TenantId::new(row.tenant_id).map_err(|error| ProjectionError::Store {
                message: error.to_string(),
            })?,
            order_id: row.order_id,
            user_id: row.user_id,
            status: row.status,
            line_count: row.line_count,
            total_quantity: row.total_quantity,
            rejection_reason: row.rejection_reason,
            last_applied_global_position: row.last_applied_global_position,
        })
    }
}

#[derive(sqlx::FromRow)]
struct ProductInventoryRow {
    tenant_id: String,
    product_id: String,
    sku: String,
    name: String,
    available_quantity: i32,
    reserved_quantity: i32,
    last_applied_global_position: i64,
}

impl TryFrom<ProductInventoryRow> for ProductInventoryReadModel {
    type Error = ProjectionError;

    fn try_from(row: ProductInventoryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            tenant_id: TenantId::new(row.tenant_id).map_err(|error| ProjectionError::Store {
                message: error.to_string(),
            })?,
            product_id: row.product_id,
            sku: row.sku,
            name: row.name,
            available_quantity: row.available_quantity,
            reserved_quantity: row.reserved_quantity,
            last_applied_global_position: row.last_applied_global_position,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_projection::ProjectionError;
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

//! PostgreSQL projection catch-up integration tests.

mod common;

use std::time::Duration;

use es_core::{CommandMetadata, ExpectedRevision, StreamId, TenantId};
use es_projection::{
    CatchUpOutcome, MinimumGlobalPosition, ProjectionBatchLimit, ProjectionError, ProjectorName,
    WaitPolicy,
};
use es_store_postgres::{
    AppendOutcome, AppendRequest, NewEvent, PostgresEventStore, PostgresProjectionStore,
};
use example_commerce::{
    OrderEvent, OrderId, OrderLine, ProductEvent, ProductId, Quantity, Sku, UserId,
};
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;

static POSTGRES_TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn tenant_id(value: &str) -> TenantId {
    TenantId::new(value).expect("valid tenant id")
}

fn stream_id(value: &str) -> StreamId {
    StreamId::new(value).expect("valid stream id")
}

fn order_id(value: &str) -> OrderId {
    OrderId::new(value).expect("valid order id")
}

fn user_id(value: &str) -> UserId {
    UserId::new(value).expect("valid user id")
}

fn product_id(value: &str) -> ProductId {
    ProductId::new(value).expect("valid product id")
}

fn sku(value: &str) -> Sku {
    Sku::new(value).expect("valid sku")
}

fn quantity(value: u32) -> Quantity {
    Quantity::new(value).expect("valid quantity")
}

fn projector_name() -> ProjectorName {
    ProjectorName::new("commerce-read-models").expect("valid projector name")
}

fn limit() -> ProjectionBatchLimit {
    ProjectionBatchLimit::new(100).expect("valid batch limit")
}

fn wait_policy() -> WaitPolicy {
    WaitPolicy::new(Duration::from_millis(50), Duration::from_millis(5)).expect("valid wait policy")
}

fn command_metadata(tenant: TenantId, seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: Some(Uuid::from_u128(seed + 2)),
        tenant_id: tenant,
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
            .expect("valid requested_at"),
    }
}

fn order_line(product: &str, quantity_value: u32) -> OrderLine {
    OrderLine {
        product_id: product_id(product),
        sku: sku(&format!("SKU-{product}")),
        quantity: quantity(quantity_value),
        product_available: true,
    }
}

fn order_event(seed: u128, event_type: &str, event: OrderEvent) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        serde_json::to_value(event).expect("serialize order event"),
        json!({ "source": "projections" }),
    )
    .expect("valid order event")
}

fn product_event(seed: u128, event_type: &str, event: ProductEvent) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        serde_json::to_value(event).expect("serialize product event"),
        json!({ "source": "projections" }),
    )
    .expect("valid product event")
}

fn invalid_order_event(seed: u128) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        "OrderPlaced",
        1,
        json!({ "not": "an order" }),
        json!({ "source": "projections" }),
    )
    .expect("valid malformed event row")
}

async fn append_events(
    store: &PostgresEventStore,
    tenant: TenantId,
    stream: StreamId,
    expected_revision: ExpectedRevision,
    idempotency_key: &str,
    command_seed: u128,
    events: Vec<NewEvent>,
) -> anyhow::Result<Vec<i64>> {
    let outcome = store
        .append(AppendRequest::new(
            stream,
            expected_revision,
            command_metadata(tenant, command_seed),
            idempotency_key,
            events,
        )?)
        .await?;

    let AppendOutcome::Committed(committed) = outcome else {
        panic!("append should commit");
    };

    Ok(committed.global_positions)
}

async fn append_order_lifecycle(
    store: &PostgresEventStore,
    tenant: TenantId,
    order: &str,
    seed: u128,
) -> anyhow::Result<Vec<i64>> {
    let mut positions = append_events(
        store,
        tenant.clone(),
        stream_id(&format!("order-{order}")),
        ExpectedRevision::NoStream,
        &format!("order-{order}-place"),
        seed,
        vec![order_event(
            seed + 10,
            "OrderPlaced",
            OrderEvent::OrderPlaced {
                order_id: order_id(order),
                user_id: user_id("user-1"),
                lines: vec![order_line("product-1", 2), order_line("product-2", 3)],
            },
        )],
    )
    .await?;
    positions.extend(
        append_events(
            store,
            tenant,
            stream_id(&format!("order-{order}")),
            ExpectedRevision::Any,
            &format!("order-{order}-confirm"),
            seed + 1,
            vec![order_event(
                seed + 11,
                "OrderConfirmed",
                OrderEvent::OrderConfirmed {
                    order_id: order_id(order),
                },
            )],
        )
        .await?,
    );

    Ok(positions)
}

async fn append_product_lifecycle(
    store: &PostgresEventStore,
    tenant: TenantId,
    product: &str,
    seed: u128,
) -> anyhow::Result<Vec<i64>> {
    append_events(
        store,
        tenant,
        stream_id(&format!("product-{product}")),
        ExpectedRevision::NoStream,
        &format!("product-{product}-events"),
        seed,
        vec![
            product_event(
                seed + 10,
                "ProductCreated",
                ProductEvent::ProductCreated {
                    product_id: product_id(product),
                    sku: sku("SKU-1"),
                    name: "Keyboard".to_owned(),
                    initial_quantity: quantity(10),
                },
            ),
            product_event(
                seed + 11,
                "InventoryAdjusted",
                ProductEvent::InventoryAdjusted {
                    product_id: product_id(product),
                    delta: 5,
                },
            ),
            product_event(
                seed + 12,
                "InventoryReserved",
                ProductEvent::InventoryReserved {
                    product_id: product_id(product),
                    quantity: quantity(4),
                },
            ),
            product_event(
                seed + 13,
                "InventoryReleased",
                ProductEvent::InventoryReleased {
                    product_id: product_id(product),
                    quantity: quantity(1),
                },
            ),
        ],
    )
    .await
}

async fn offset_position(
    projections: &PostgresProjectionStore,
    tenant: &TenantId,
    projector: &ProjectorName,
) -> anyhow::Result<Option<i64>> {
    Ok(projections
        .projector_offset(tenant, projector)
        .await?
        .map(|offset| offset.last_global_position))
}

#[tokio::test]
async fn projections_offset_commits_with_read_models() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();

    let positions = append_order_lifecycle(&events, tenant.clone(), "order-1", 100).await?;
    let last_position = *positions.last().expect("positions");

    let outcome = projections.catch_up(&tenant, &projector, limit()).await?;
    assert_eq!(
        CatchUpOutcome::Applied {
            event_count: 2,
            last_global_position: last_position,
        },
        outcome
    );

    let order = projections
        .order_summary(&tenant, "order-1", None, None)
        .await?
        .expect("order summary row");
    assert_eq!("Confirmed", order.status);
    assert_eq!(last_position, order.last_applied_global_position);
    assert_eq!(
        Some(last_position),
        offset_position(&projections, &tenant, &projector).await?
    );

    Ok(())
}

#[tokio::test]
async fn projections_build_commerce_read_models() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();

    append_order_lifecycle(&events, tenant.clone(), "order-1", 200).await?;
    let product_positions =
        append_product_lifecycle(&events, tenant.clone(), "product-1", 300).await?;
    projections.catch_up(&tenant, &projector, limit()).await?;

    let order = projections
        .order_summary(&tenant, "order-1", None, None)
        .await?
        .expect("order summary row");
    assert_eq!("Confirmed", order.status);
    assert_eq!("user-1", order.user_id);
    assert_eq!(2, order.line_count);
    assert_eq!(5, order.total_quantity);
    assert_eq!(None, order.rejection_reason);

    let product = projections
        .product_inventory(&tenant, "product-1", None, None)
        .await?
        .expect("product inventory row");
    assert_eq!("SKU-1", product.sku);
    assert_eq!("Keyboard", product.name);
    assert_eq!(12, product.available_quantity);
    assert_eq!(3, product.reserved_quantity);
    assert_eq!(
        *product_positions.last().expect("product position"),
        product.last_applied_global_position
    );

    Ok(())
}

#[tokio::test]
async fn projections_resume_without_duplicate_effects() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();

    let positions = append_product_lifecycle(&events, tenant.clone(), "product-1", 400).await?;
    let last_position = *positions.last().expect("positions");
    projections.catch_up(&tenant, &projector, limit()).await?;
    let before = projections
        .product_inventory(&tenant, "product-1", None, None)
        .await?
        .expect("product inventory row");

    let restarted = PostgresProjectionStore::new(harness.pool.clone());
    assert_eq!(
        CatchUpOutcome::Idle,
        restarted.catch_up(&tenant, &projector, limit()).await?
    );
    let after = restarted
        .product_inventory(&tenant, "product-1", None, None)
        .await?
        .expect("product inventory row");

    assert_eq!(before, after);
    assert_eq!(
        Some(last_position),
        offset_position(&restarted, &tenant, &projector).await?
    );

    Ok(())
}

#[tokio::test]
async fn projections_queries_wait_for_minimum_position() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();

    let positions = append_order_lifecycle(&events, tenant.clone(), "order-1", 500).await?;
    let last_position = *positions.last().expect("positions");
    projections.catch_up(&tenant, &projector, limit()).await?;

    let fresh = projections
        .order_summary(
            &tenant,
            "order-1",
            Some(MinimumGlobalPosition::new(last_position)?),
            Some(wait_policy()),
        )
        .await?;
    assert!(fresh.is_some());

    let error = projections
        .order_summary(
            &tenant,
            "order-1",
            Some(MinimumGlobalPosition::new(last_position + 10)?),
            Some(wait_policy()),
        )
        .await
        .expect_err("lagging query returns typed timeout");
    assert!(matches!(
        error,
        ProjectionError::ProjectionLag {
            required,
            actual
        } if required == last_position + 10 && actual == last_position
    ));

    Ok(())
}

#[tokio::test]
async fn projections_are_scoped_by_tenant() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant_a = tenant_id("tenant-a");
    let tenant_b = tenant_id("tenant-b");
    let projector = projector_name();

    append_events(
        &events,
        tenant_a.clone(),
        stream_id("order-shared"),
        ExpectedRevision::NoStream,
        "tenant-a-order",
        600,
        vec![order_event(
            610,
            "OrderPlaced",
            OrderEvent::OrderPlaced {
                order_id: order_id("shared"),
                user_id: user_id("tenant-a-user"),
                lines: vec![order_line("product-1", 1)],
            },
        )],
    )
    .await?;
    append_events(
        &events,
        tenant_b.clone(),
        stream_id("order-shared"),
        ExpectedRevision::NoStream,
        "tenant-b-order",
        700,
        vec![order_event(
            710,
            "OrderPlaced",
            OrderEvent::OrderPlaced {
                order_id: order_id("shared"),
                user_id: user_id("tenant-b-user"),
                lines: vec![order_line("product-1", 2)],
            },
        )],
    )
    .await?;

    projections.catch_up(&tenant_a, &projector, limit()).await?;
    projections.catch_up(&tenant_b, &projector, limit()).await?;

    let tenant_a_order = projections
        .order_summary(&tenant_a, "shared", None, None)
        .await?
        .expect("tenant a order");
    let tenant_b_order = projections
        .order_summary(&tenant_b, "shared", None, None)
        .await?
        .expect("tenant b order");

    assert_eq!("tenant-a-user", tenant_a_order.user_id);
    assert_eq!(1, tenant_a_order.total_quantity);
    assert_eq!("tenant-b-user", tenant_b_order.user_id);
    assert_eq!(2, tenant_b_order.total_quantity);

    Ok(())
}

#[tokio::test]
async fn projections_malformed_payload_does_not_advance_offset() -> anyhow::Result<()> {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let harness = common::start_postgres().await?;
    let events = PostgresEventStore::new(harness.pool.clone());
    let projections = PostgresProjectionStore::new(harness.pool.clone());
    let tenant = tenant_id("tenant-a");
    let projector = projector_name();

    append_events(
        &events,
        tenant.clone(),
        stream_id("order-bad"),
        ExpectedRevision::NoStream,
        "bad-order",
        800,
        vec![invalid_order_event(810)],
    )
    .await?;

    let error = projections
        .catch_up(&tenant, &projector, limit())
        .await
        .expect_err("malformed payload fails catch-up");
    assert!(matches!(
        error,
        ProjectionError::PayloadDecode {
            event_type,
            schema_version: 1
        } if event_type == "OrderPlaced"
    ));
    assert_eq!(
        None,
        offset_position(&projections, &tenant, &projector).await?
    );

    Ok(())
}

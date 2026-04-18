use axum::{Json, Router, extract::State, routing::post};
use es_core::{CommandMetadata, TenantId};
use es_runtime::{CommandEnvelope, CommandGateway, CommandOutcome};
use example_commerce::{
    Order, OrderCommand, OrderId, OrderLine, OrderReply, Product, ProductCommand, ProductId,
    ProductReply, Quantity, Sku, User, UserCommand, UserId, UserReply,
};
use metrics::histogram;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::oneshot;
use tracing::{Instrument, info_span};
use uuid::Uuid;

use crate::ApiError;

/// Shared HTTP adapter state. It owns no business state, only bounded runtime ingress handles.
#[derive(Clone)]
pub struct HttpState {
    /// Order command gateway.
    pub order_gateway: CommandGateway<Order>,
    /// Product command gateway.
    pub product_gateway: CommandGateway<Product>,
    /// User command gateway.
    pub user_gateway: CommandGateway<User>,
}

/// Builds commerce command routes.
pub fn commerce_routes(state: HttpState) -> Router {
    Router::new()
        .route("/commands/orders/place", post(place_order))
        .route("/commands/orders/confirm", post(confirm_order))
        .route("/commands/orders/reject", post(reject_order))
        .route("/commands/orders/cancel", post(cancel_order))
        .route("/commands/products/create", post(create_product))
        .route(
            "/commands/products/adjust-inventory",
            post(adjust_inventory),
        )
        .route("/commands/products/reserve", post(reserve_inventory))
        .route("/commands/products/release", post(release_inventory))
        .route("/commands/users/register", post(register_user))
        .route("/commands/users/activate", post(activate_user))
        .route("/commands/users/deactivate", post(deactivate_user))
        .with_state(state)
}

/// Common command request metadata.
#[derive(Clone, Debug, Deserialize)]
pub struct CommandRequestMetadata {
    /// Tenant identity.
    pub tenant_id: String,
    /// Tenant-scoped idempotency key.
    pub idempotency_key: String,
    /// Optional caller-supplied command ID.
    pub command_id: Option<Uuid>,
    /// Optional caller-supplied correlation ID.
    pub correlation_id: Option<Uuid>,
    /// Optional caller-supplied causation ID.
    pub causation_id: Option<Uuid>,
}

/// Place-order request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct PlaceOrderRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// Order identity.
    pub order_id: String,
    /// User identity.
    pub user_id: String,
    /// User active state observed by caller/process manager.
    pub user_active: bool,
    /// Requested order lines.
    pub lines: Vec<OrderLineRequest>,
}

/// Order-line request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct OrderLineRequest {
    /// Product identity.
    pub product_id: String,
    /// Product SKU.
    pub sku: String,
    /// Requested quantity.
    pub quantity: u32,
    /// Product availability observed by caller/process manager.
    pub product_available: bool,
}

/// Single order ID request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct OrderIdRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// Order identity.
    pub order_id: String,
}

/// Reject-order request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct RejectOrderRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// Order identity.
    pub order_id: String,
    /// Rejection reason.
    pub reason: String,
}

/// Create-product request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct CreateProductRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// Product identity.
    pub product_id: String,
    /// Product SKU.
    pub sku: String,
    /// Display name.
    pub name: String,
    /// Initial available quantity.
    pub initial_quantity: u32,
}

/// Product inventory request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct InventoryRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// Product identity.
    pub product_id: String,
    /// Quantity to reserve/release.
    pub quantity: u32,
}

/// Inventory adjustment request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct AdjustInventoryRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// Product identity.
    pub product_id: String,
    /// Signed inventory delta.
    pub delta: i32,
}

/// Register-user request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct RegisterUserRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// User identity.
    pub user_id: String,
    /// User email.
    pub email: String,
    /// User display name.
    pub display_name: String,
}

/// Single user ID request DTO.
#[derive(Clone, Debug, Deserialize)]
pub struct UserIdRequest {
    /// Common command metadata.
    #[serde(flatten)]
    pub metadata: CommandRequestMetadata,
    /// User identity.
    pub user_id: String,
}

/// Successful command response.
#[derive(Clone, Debug, Serialize)]
pub struct CommandSuccess<R> {
    /// Correlation ID associated with the command.
    pub correlation_id: Uuid,
    /// Stream affected by the command.
    pub stream_id: String,
    /// Last stream revision assigned by the append.
    pub stream_revision: u64,
    /// First stream revision assigned by the append.
    pub first_revision: u64,
    /// Last stream revision assigned by the append.
    pub last_revision: u64,
    /// Global positions assigned by the durable event store.
    pub global_positions: Vec<i64>,
    /// Event IDs assigned by the durable event store.
    pub event_ids: Vec<Uuid>,
    /// Typed command reply payload.
    pub reply: R,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OrderReplyDto {
    Placed { order_id: String },
    Confirmed { order_id: String },
    Rejected { order_id: String },
    Cancelled { order_id: String },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ProductReplyDto {
    Created { product_id: String },
    InventoryAdjusted { product_id: String },
    InventoryReserved { product_id: String },
    InventoryReleased { product_id: String },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum UserReplyDto {
    Registered { user_id: String },
    Activated { user_id: String },
    Deactivated { user_id: String },
}

async fn place_order(
    State(state): State<HttpState>,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<CommandSuccess<OrderReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let lines = request
        .lines
        .into_iter()
        .map(OrderLineRequest::into_domain)
        .collect::<Result<Vec<_>, _>>()?;
    let command = OrderCommand::PlaceOrder {
        order_id: OrderId::new(request.order_id).map_err(ApiError::invalid_request)?,
        user_id: UserId::new(request.user_id).map_err(ApiError::invalid_request)?,
        user_active: request.user_active,
        lines,
    };
    let gateway = CommandGateway::<Order>::clone(&state.order_gateway);
    submit_command::<Order, _, _>(
        gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        |reply| reply.into(),
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn confirm_order(
    State(state): State<HttpState>,
    Json(request): Json<OrderIdRequest>,
) -> Result<Json<CommandSuccess<OrderReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = OrderCommand::ConfirmOrder {
        order_id: OrderId::new(request.order_id).map_err(ApiError::invalid_request)?,
    };
    submit_command::<Order, _, _>(
        state.order_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        OrderReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn reject_order(
    State(state): State<HttpState>,
    Json(request): Json<RejectOrderRequest>,
) -> Result<Json<CommandSuccess<OrderReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = OrderCommand::RejectOrder {
        order_id: OrderId::new(request.order_id).map_err(ApiError::invalid_request)?,
        reason: request.reason,
    };
    submit_command::<Order, _, _>(
        state.order_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        OrderReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn cancel_order(
    State(state): State<HttpState>,
    Json(request): Json<OrderIdRequest>,
) -> Result<Json<CommandSuccess<OrderReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = OrderCommand::CancelOrder {
        order_id: OrderId::new(request.order_id).map_err(ApiError::invalid_request)?,
    };
    submit_command::<Order, _, _>(
        state.order_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        OrderReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn create_product(
    State(state): State<HttpState>,
    Json(request): Json<CreateProductRequest>,
) -> Result<Json<CommandSuccess<ProductReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = ProductCommand::CreateProduct {
        product_id: ProductId::new(request.product_id).map_err(ApiError::invalid_request)?,
        sku: Sku::new(request.sku).map_err(ApiError::invalid_request)?,
        name: request.name,
        initial_quantity: Quantity::new(request.initial_quantity)
            .map_err(ApiError::invalid_request)?,
    };
    submit_command::<Product, _, _>(
        state.product_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        ProductReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn adjust_inventory(
    State(state): State<HttpState>,
    Json(request): Json<AdjustInventoryRequest>,
) -> Result<Json<CommandSuccess<ProductReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = ProductCommand::AdjustInventory {
        product_id: ProductId::new(request.product_id).map_err(ApiError::invalid_request)?,
        delta: request.delta,
    };
    submit_command::<Product, _, _>(
        state.product_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        ProductReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn reserve_inventory(
    State(state): State<HttpState>,
    Json(request): Json<InventoryRequest>,
) -> Result<Json<CommandSuccess<ProductReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = ProductCommand::ReserveInventory {
        product_id: ProductId::new(request.product_id).map_err(ApiError::invalid_request)?,
        quantity: Quantity::new(request.quantity).map_err(ApiError::invalid_request)?,
    };
    submit_command::<Product, _, _>(
        state.product_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        ProductReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn release_inventory(
    State(state): State<HttpState>,
    Json(request): Json<InventoryRequest>,
) -> Result<Json<CommandSuccess<ProductReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = ProductCommand::ReleaseInventory {
        product_id: ProductId::new(request.product_id).map_err(ApiError::invalid_request)?,
        quantity: Quantity::new(request.quantity).map_err(ApiError::invalid_request)?,
    };
    submit_command::<Product, _, _>(
        state.product_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        ProductReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn register_user(
    State(state): State<HttpState>,
    Json(request): Json<RegisterUserRequest>,
) -> Result<Json<CommandSuccess<UserReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = UserCommand::RegisterUser {
        user_id: UserId::new(request.user_id).map_err(ApiError::invalid_request)?,
        email: request.email,
        display_name: request.display_name,
    };
    submit_command::<User, _, _>(
        state.user_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        UserReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn activate_user(
    State(state): State<HttpState>,
    Json(request): Json<UserIdRequest>,
) -> Result<Json<CommandSuccess<UserReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = UserCommand::ActivateUser {
        user_id: UserId::new(request.user_id).map_err(ApiError::invalid_request)?,
    };
    submit_command::<User, _, _>(
        state.user_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        UserReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn deactivate_user(
    State(state): State<HttpState>,
    Json(request): Json<UserIdRequest>,
) -> Result<Json<CommandSuccess<UserReplyDto>>, ApiError> {
    let metadata = request.metadata.command_metadata()?;
    let correlation_id = metadata.correlation_id;
    let command = UserCommand::DeactivateUser {
        user_id: UserId::new(request.user_id).map_err(ApiError::invalid_request)?,
    };
    submit_command::<User, _, _>(
        state.user_gateway,
        command,
        metadata,
        request.metadata.idempotency_key,
        UserReplyDto::from,
    )
    .await
    .map(|success| Json(success.with_correlation(correlation_id)))
}

async fn submit_command<A, F, R>(
    gateway: CommandGateway<A>,
    command: A::Command,
    metadata: CommandMetadata,
    idempotency_key: String,
    map_reply: F,
) -> Result<CommandSuccess<R>, ApiError>
where
    A: es_runtime::Aggregate,
    F: FnOnce(A::Reply) -> R,
{
    let started_at = std::time::Instant::now();
    let (reply, receiver) = oneshot::channel();
    let envelope = CommandEnvelope::<A>::new(command, metadata, idempotency_key, reply)?;
    let stream_id = envelope.stream_id.as_str().to_owned();
    let aggregate = aggregate_label::<A>();
    let span = info_span!(
        "http.command",
        command_id = %envelope.metadata.command_id,
        correlation_id = %envelope.metadata.correlation_id,
        causation_id = ?envelope.metadata.causation_id,
        tenant_id = %envelope.metadata.tenant_id.as_str(),
        stream_id = %envelope.stream_id.as_str(),
        global_position = tracing::field::Empty,
    );

    let instrument_span = span.clone();
    async move {
        gateway.try_submit(envelope)?;
        let outcome = receiver.await.map_err(|_| ApiError::ReplyDropped)??;
        if let Some(global_position) = outcome.append.global_positions.last() {
            span.record("global_position", global_position);
        }
        histogram!(
            "es_command_latency_seconds",
            "aggregate" => aggregate,
            "outcome" => "success",
        )
        .record(started_at.elapsed().as_secs_f64());
        Ok(CommandSuccess::from_outcome(stream_id, outcome, map_reply))
    }
    .instrument(instrument_span)
    .await
}

impl CommandRequestMetadata {
    fn command_metadata(&self) -> Result<CommandMetadata, ApiError> {
        let command_id = self.command_id.unwrap_or_else(Uuid::now_v7);
        Ok(CommandMetadata {
            command_id,
            correlation_id: self.correlation_id.unwrap_or(command_id),
            causation_id: self.causation_id,
            tenant_id: TenantId::new(self.tenant_id.clone()).map_err(ApiError::invalid_request)?,
            requested_at: OffsetDateTime::now_utc(),
        })
    }
}

impl OrderLineRequest {
    fn into_domain(self) -> Result<OrderLine, ApiError> {
        Ok(OrderLine {
            product_id: ProductId::new(self.product_id).map_err(ApiError::invalid_request)?,
            sku: Sku::new(self.sku).map_err(ApiError::invalid_request)?,
            quantity: Quantity::new(self.quantity).map_err(ApiError::invalid_request)?,
            product_available: self.product_available,
        })
    }
}

impl<R> CommandSuccess<R> {
    fn from_outcome<A, F>(stream_id: String, outcome: CommandOutcome<A>, map_reply: F) -> Self
    where
        F: FnOnce(A) -> R,
    {
        let stream_revision = outcome.append.last_revision.value();
        Self {
            correlation_id: Uuid::nil(),
            stream_id,
            stream_revision,
            first_revision: outcome.append.first_revision.value(),
            last_revision: stream_revision,
            global_positions: outcome.append.global_positions,
            event_ids: outcome.append.event_ids,
            reply: map_reply(outcome.reply),
        }
    }

    fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }
}

impl From<OrderReply> for OrderReplyDto {
    fn from(reply: OrderReply) -> Self {
        match reply {
            OrderReply::Placed { order_id } => Self::Placed {
                order_id: order_id.into_inner(),
            },
            OrderReply::Confirmed { order_id } => Self::Confirmed {
                order_id: order_id.into_inner(),
            },
            OrderReply::Rejected { order_id } => Self::Rejected {
                order_id: order_id.into_inner(),
            },
            OrderReply::Cancelled { order_id } => Self::Cancelled {
                order_id: order_id.into_inner(),
            },
        }
    }
}

impl From<ProductReply> for ProductReplyDto {
    fn from(reply: ProductReply) -> Self {
        match reply {
            ProductReply::Created { product_id } => Self::Created {
                product_id: product_id.into_inner(),
            },
            ProductReply::InventoryAdjusted { product_id } => Self::InventoryAdjusted {
                product_id: product_id.into_inner(),
            },
            ProductReply::InventoryReserved { product_id } => Self::InventoryReserved {
                product_id: product_id.into_inner(),
            },
            ProductReply::InventoryReleased { product_id } => Self::InventoryReleased {
                product_id: product_id.into_inner(),
            },
        }
    }
}

impl From<UserReply> for UserReplyDto {
    fn from(reply: UserReply) -> Self {
        match reply {
            UserReply::Registered { user_id } => Self::Registered {
                user_id: user_id.into_inner(),
            },
            UserReply::Activated { user_id } => Self::Activated {
                user_id: user_id.into_inner(),
            },
            UserReply::Deactivated { user_id } => Self::Deactivated {
                user_id: user_id.into_inner(),
            },
        }
    }
}

fn aggregate_label<A>() -> &'static str {
    let type_name = std::any::type_name::<A>();
    if type_name.ends_with("::Order") {
        "order"
    } else if type_name.ends_with("::Product") {
        "product"
    } else if type_name.ends_with("::User") {
        "user"
    } else {
        "aggregate"
    }
}

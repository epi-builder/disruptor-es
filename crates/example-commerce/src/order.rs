use crate::{OrderId, ProductId, Quantity, Sku, UserId};

/// Order aggregate marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order;

/// Order lifecycle status.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum OrderStatus {
    /// Order has not been placed.
    #[default]
    NotPlaced,
    /// Order has been placed and is awaiting outcome.
    Placed,
    /// Order was confirmed.
    Confirmed,
    /// Order was rejected.
    Rejected,
    /// Order was cancelled.
    Cancelled,
}

/// Product line captured by an order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderLine {
    /// Product identity referenced by the order.
    pub product_id: ProductId,
    /// SKU referenced by the order.
    pub sku: Sku,
    /// Quantity requested for the line.
    pub quantity: Quantity,
}

/// Order aggregate state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OrderState {
    /// Order identity, if placed.
    pub order_id: Option<OrderId>,
    /// User identity that owns the order, if placed.
    pub user_id: Option<UserId>,
    /// Product lines captured by the order.
    pub lines: Vec<OrderLine>,
    /// Current order lifecycle status.
    pub status: OrderStatus,
}

/// Commands accepted by the order aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderCommand {}

/// Events emitted by the order aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderEvent {}

/// Replies returned by order commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderReply {}

/// Order command validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderError {}

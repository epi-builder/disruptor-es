use crate::{OrderId, ProductId, Quantity, Sku, UserId};
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};
use serde::{Deserialize, Serialize};

/// Order aggregate marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order;

/// Order lifecycle status.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum OrderStatus {
    /// Order has not been placed.
    #[default]
    Draft,
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OrderLine {
    /// Product identity referenced by the order.
    pub product_id: ProductId,
    /// SKU referenced by the order.
    pub sku: Sku,
    /// Quantity requested for the line.
    pub quantity: Quantity,
    /// Whether the product is available when the order command is decided.
    pub product_available: bool,
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
    /// Reason captured when the order is rejected.
    pub rejection_reason: Option<String>,
}

/// Commands accepted by the order aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum OrderCommand {
    /// Places a new order.
    PlaceOrder { order_id: OrderId, user_id: UserId, user_active: bool, lines: Vec<OrderLine> },
    /// Confirms a placed order.
    ConfirmOrder { order_id: OrderId },
    /// Rejects a placed order.
    RejectOrder { order_id: OrderId, reason: String },
    /// Cancels a placed order.
    CancelOrder { order_id: OrderId },
}

/// Events emitted by the order aggregate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum OrderEvent {
    /// Order was placed.
    OrderPlaced { order_id: OrderId, user_id: UserId, lines: Vec<OrderLine> },
    /// Order was confirmed.
    OrderConfirmed { order_id: OrderId },
    /// Order was rejected.
    OrderRejected { order_id: OrderId, reason: String },
    /// Order was cancelled.
    OrderCancelled { order_id: OrderId },
}

/// Replies returned by order commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderReply {
    /// Order placement succeeded.
    Placed {
        /// Placed order.
        order_id: OrderId,
    },
    /// Order confirmation succeeded.
    Confirmed {
        /// Confirmed order.
        order_id: OrderId,
    },
    /// Order rejection succeeded.
    Rejected {
        /// Rejected order.
        order_id: OrderId,
    },
    /// Order cancellation succeeded.
    Cancelled {
        /// Cancelled order.
        order_id: OrderId,
    },
}

/// Order command validation errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[allow(missing_docs)]
pub enum OrderError {
    /// Order lines must not be empty.
    #[error("order must contain at least one line")]
    EmptyOrder,
    /// User must be active before placing an order.
    #[error("user {user_id:?} is inactive")]
    InactiveUser { user_id: UserId },
    /// Product must be available before being included in an order.
    #[error("product {product_id:?} is unavailable")]
    UnavailableProduct { product_id: ProductId },
    /// Order has already been placed.
    #[error("order is already placed")]
    AlreadyPlaced,
    /// Order must be placed before lifecycle transitions.
    #[error("order has not been placed")]
    NotPlaced,
    /// Order has already reached a terminal state.
    #[error("order is already terminal")]
    AlreadyTerminal,
    /// Rejection reason must not be empty.
    #[error("order rejection reason cannot be empty")]
    EmptyRejectionReason,
}

impl Aggregate for Order {
    type State = OrderState;
    type Command = OrderCommand;
    type Event = OrderEvent;
    type Reply = OrderReply;
    type Error = OrderError;

    fn stream_id(command: &Self::Command) -> StreamId {
        StreamId::new(format!("order-{}", command.order_id().as_str()))
            .expect("order id creates a valid stream id")
    }

    fn partition_key(command: &Self::Command) -> PartitionKey {
        PartitionKey::new(format!("order-{}", command.order_id().as_str()))
            .expect("order id creates a valid partition key")
    }

    fn expected_revision(command: &Self::Command) -> ExpectedRevision {
        match command {
            OrderCommand::PlaceOrder { .. } => ExpectedRevision::NoStream,
            OrderCommand::ConfirmOrder { .. }
            | OrderCommand::RejectOrder { .. }
            | OrderCommand::CancelOrder { .. } => ExpectedRevision::Any,
        }
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        match command {
            OrderCommand::PlaceOrder {
                order_id,
                user_id,
                user_active,
                lines,
            } => {
                if state.status != OrderStatus::Draft {
                    return Err(OrderError::AlreadyPlaced);
                }
                if lines.is_empty() {
                    return Err(OrderError::EmptyOrder);
                }
                if !user_active {
                    return Err(OrderError::InactiveUser { user_id });
                }
                if let Some(line) = lines.iter().find(|line| !line.product_available) {
                    return Err(OrderError::UnavailableProduct {
                        product_id: line.product_id.clone(),
                    });
                }

                Ok(Decision::new(
                    vec![OrderEvent::OrderPlaced {
                        order_id: order_id.clone(),
                        user_id,
                        lines,
                    }],
                    OrderReply::Placed { order_id },
                ))
            }
            OrderCommand::ConfirmOrder { order_id } => {
                ensure_placed(state)?;
                Ok(Decision::new(
                    vec![OrderEvent::OrderConfirmed {
                        order_id: order_id.clone(),
                    }],
                    OrderReply::Confirmed { order_id },
                ))
            }
            OrderCommand::RejectOrder { order_id, reason } => {
                ensure_placed(state)?;
                if reason.is_empty() {
                    return Err(OrderError::EmptyRejectionReason);
                }

                Ok(Decision::new(
                    vec![OrderEvent::OrderRejected {
                        order_id: order_id.clone(),
                        reason,
                    }],
                    OrderReply::Rejected { order_id },
                ))
            }
            OrderCommand::CancelOrder { order_id } => {
                ensure_placed(state)?;
                Ok(Decision::new(
                    vec![OrderEvent::OrderCancelled {
                        order_id: order_id.clone(),
                    }],
                    OrderReply::Cancelled { order_id },
                ))
            }
        }
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        match event {
            OrderEvent::OrderPlaced {
                order_id,
                user_id,
                lines,
            } => {
                state.order_id = Some(order_id.clone());
                state.user_id = Some(user_id.clone());
                state.lines = lines.clone();
                state.status = OrderStatus::Placed;
                state.rejection_reason = None;
            }
            OrderEvent::OrderConfirmed { order_id } => {
                state.order_id = Some(order_id.clone());
                state.status = OrderStatus::Confirmed;
            }
            OrderEvent::OrderRejected { order_id, reason } => {
                state.order_id = Some(order_id.clone());
                state.status = OrderStatus::Rejected;
                state.rejection_reason = Some(reason.clone());
            }
            OrderEvent::OrderCancelled { order_id } => {
                state.order_id = Some(order_id.clone());
                state.status = OrderStatus::Cancelled;
            }
        }
    }
}

impl OrderCommand {
    fn order_id(&self) -> &OrderId {
        match self {
            OrderCommand::PlaceOrder { order_id, .. }
            | OrderCommand::ConfirmOrder { order_id }
            | OrderCommand::RejectOrder { order_id, .. }
            | OrderCommand::CancelOrder { order_id } => order_id,
        }
    }
}

fn ensure_placed(state: &OrderState) -> Result<(), OrderError> {
    match state.status {
        OrderStatus::Draft => Err(OrderError::NotPlaced),
        OrderStatus::Placed => Ok(()),
        OrderStatus::Confirmed | OrderStatus::Rejected | OrderStatus::Cancelled => {
            Err(OrderError::AlreadyTerminal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_core::{CommandMetadata, TenantId};
    use es_kernel::Aggregate;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(21),
            correlation_id: Uuid::from_u128(22),
            causation_id: None,
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        }
    }

    fn order_id() -> OrderId {
        OrderId::new("order-1").expect("order id")
    }

    fn user_id() -> UserId {
        UserId::new("user-1").expect("user id")
    }

    fn product_id() -> ProductId {
        ProductId::new("product-1").expect("product id")
    }

    fn sku() -> Sku {
        Sku::new("SKU-1").expect("sku")
    }

    fn available_line() -> OrderLine {
        OrderLine {
            product_id: product_id(),
            sku: sku(),
            quantity: Quantity::new(2).expect("quantity"),
            product_available: true,
        }
    }

    #[test]
    fn order_projection_payload_roundtrips_order_placed() {
        let event = OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: user_id(),
            lines: vec![available_line()],
        };

        let value = serde_json::to_value(&event).expect("serialize order event");
        let decoded = serde_json::from_value::<OrderEvent>(value).expect("deserialize order event");

        assert_eq!(event, decoded);
        let OrderEvent::OrderPlaced {
            order_id,
            user_id,
            lines,
        } = decoded
        else {
            panic!("expected OrderPlaced");
        };
        assert_eq!(order_id, self::order_id());
        assert_eq!(user_id, self::user_id());
        assert_eq!(1, lines.len());
        assert_eq!(sku(), lines[0].sku);
        assert_eq!(Quantity::new(2).expect("quantity"), lines[0].quantity);
    }

    #[test]
    fn order_projection_payload_roundtrips_order_rejected() {
        let event = OrderEvent::OrderRejected {
            order_id: order_id(),
            reason: "out of stock".to_owned(),
        };

        let value = serde_json::to_value(&event).expect("serialize order event");
        let decoded = serde_json::from_value::<OrderEvent>(value).expect("deserialize order event");

        assert_eq!(event, decoded);
        let OrderEvent::OrderRejected { reason, .. } = decoded else {
            panic!("expected OrderRejected");
        };
        assert_eq!("out of stock", reason);
    }

    fn place_command(lines: Vec<OrderLine>) -> OrderCommand {
        OrderCommand::PlaceOrder {
            order_id: order_id(),
            user_id: user_id(),
            user_active: true,
            lines,
        }
    }

    fn placed_state() -> OrderState {
        let decision = Order::decide(
            &OrderState::default(),
            place_command(vec![available_line()]),
            &metadata(),
        )
        .expect("placed");
        es_kernel::replay::<Order>(decision.events)
    }

    #[test]
    fn place_order_emits_placed_event() {
        let line = available_line();
        let decision = Order::decide(
            &OrderState::default(),
            place_command(vec![line.clone()]),
            &metadata(),
        )
        .expect("place order");

        assert_eq!(
            vec![OrderEvent::OrderPlaced {
                order_id: order_id(),
                user_id: user_id(),
                lines: vec![line],
            }],
            decision.events
        );
        assert_eq!(
            OrderReply::Placed {
                order_id: order_id(),
            },
            decision.reply
        );
    }

    #[test]
    fn order_rejects_invalid_placement_assumptions() {
        assert_eq!(
            OrderError::EmptyOrder,
            Order::decide(
                &OrderState::default(),
                place_command(Vec::new()),
                &metadata()
            )
            .expect_err("empty order")
        );

        assert_eq!(
            OrderError::InactiveUser { user_id: user_id() },
            Order::decide(
                &OrderState::default(),
                OrderCommand::PlaceOrder {
                    order_id: order_id(),
                    user_id: user_id(),
                    user_active: false,
                    lines: vec![available_line()],
                },
                &metadata()
            )
            .expect_err("inactive user")
        );

        let unavailable = OrderLine {
            product_available: false,
            ..available_line()
        };
        assert_eq!(
            OrderError::UnavailableProduct {
                product_id: product_id(),
            },
            Order::decide(
                &OrderState::default(),
                place_command(vec![unavailable]),
                &metadata()
            )
            .expect_err("unavailable product")
        );

        assert_eq!(
            OrderError::AlreadyPlaced,
            Order::decide(
                &placed_state(),
                place_command(vec![available_line()]),
                &metadata()
            )
            .expect_err("duplicate placement")
        );
    }

    #[test]
    fn order_terminal_transitions_are_replayable_and_final() {
        let placed = placed_state();

        let confirmed = Order::decide(
            &placed,
            OrderCommand::ConfirmOrder {
                order_id: order_id(),
            },
            &metadata(),
        )
        .expect("confirm order");
        assert_eq!(
            vec![OrderEvent::OrderConfirmed {
                order_id: order_id(),
            }],
            confirmed.events
        );

        let confirmed_state = es_kernel::replay::<Order>([
            OrderEvent::OrderPlaced {
                order_id: order_id(),
                user_id: user_id(),
                lines: vec![available_line()],
            },
            confirmed.events[0].clone(),
        ]);
        assert_eq!(OrderStatus::Confirmed, confirmed_state.status);
        assert_eq!(
            OrderError::AlreadyTerminal,
            Order::decide(
                &confirmed_state,
                OrderCommand::CancelOrder {
                    order_id: order_id(),
                },
                &metadata()
            )
            .expect_err("terminal transition")
        );

        let rejected = Order::decide(
            &placed,
            OrderCommand::RejectOrder {
                order_id: order_id(),
                reason: "out of stock".to_owned(),
            },
            &metadata(),
        )
        .expect("reject order");
        let rejected_state = es_kernel::replay::<Order>([
            OrderEvent::OrderPlaced {
                order_id: order_id(),
                user_id: user_id(),
                lines: vec![available_line()],
            },
            rejected.events[0].clone(),
        ]);
        assert_eq!(OrderStatus::Rejected, rejected_state.status);
        assert_eq!(
            Some("out of stock".to_owned()),
            rejected_state.rejection_reason
        );

        let cancelled = Order::decide(
            &placed,
            OrderCommand::CancelOrder {
                order_id: order_id(),
            },
            &metadata(),
        )
        .expect("cancel order");
        let cancelled_state = es_kernel::replay::<Order>([
            OrderEvent::OrderPlaced {
                order_id: order_id(),
                user_id: user_id(),
                lines: vec![available_line()],
            },
            cancelled.events[0].clone(),
        ]);
        assert_eq!(OrderStatus::Cancelled, cancelled_state.status);
    }

    #[test]
    fn lifecycle_commands_require_placed_state_and_rejection_reason() {
        assert_eq!(
            OrderError::NotPlaced,
            Order::decide(
                &OrderState::default(),
                OrderCommand::ConfirmOrder {
                    order_id: order_id(),
                },
                &metadata()
            )
            .expect_err("not placed")
        );

        assert_eq!(
            OrderError::EmptyRejectionReason,
            Order::decide(
                &placed_state(),
                OrderCommand::RejectOrder {
                    order_id: order_id(),
                    reason: String::new(),
                },
                &metadata()
            )
            .expect_err("empty rejection reason")
        );
    }
}

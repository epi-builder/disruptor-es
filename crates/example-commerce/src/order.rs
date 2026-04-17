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
            Order::decide(&OrderState::default(), place_command(Vec::new()), &metadata())
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
            Order::decide(&placed_state(), place_command(vec![available_line()]), &metadata())
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
        assert_eq!(Some("out of stock".to_owned()), rejected_state.rejection_reason);

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

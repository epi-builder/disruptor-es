use super::{
    Order, OrderCommand, OrderError, OrderEvent, OrderId, OrderLine, OrderState, OrderStatus,
    Product, ProductCommand, ProductEvent, ProductId, ProductState, Quantity, Sku, User,
    UserCommand, UserEvent, UserId, UserState, UserStatus,
};
use es_core::{CommandMetadata, TenantId};
use es_kernel::Aggregate;
use proptest::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug)]
enum UserStep {
    Register { email: String, display_name: String },
    Activate,
    Deactivate,
}

#[derive(Clone, Debug)]
enum ProductStep {
    Create { name: String, initial_quantity: u32 },
    Adjust(i32),
    Reserve(u32),
    Release(u32),
}

#[derive(Clone, Debug)]
enum OrderStep {
    Place {
        user_active: bool,
        available_lines: Vec<bool>,
    },
    Confirm,
    Reject(String),
    Cancel,
}

fn metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(21),
        correlation_id: Uuid::from_u128(22),
        causation_id: None,
        tenant_id: TenantId::new("tenant-a").expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn user_id() -> UserId {
    UserId::new("user-1").expect("user id")
}

fn product_id() -> ProductId {
    ProductId::new("product-1").expect("product id")
}

fn order_id() -> OrderId {
    OrderId::new("order-1").expect("order id")
}

fn sku() -> Sku {
    Sku::new("SKU-1").expect("sku")
}

fn quantity(value: u32) -> Quantity {
    Quantity::new(value.max(1)).expect("quantity")
}

fn order_line(product_available: bool, suffix: usize) -> OrderLine {
    OrderLine {
        product_id: ProductId::new(format!("product-{suffix}")).expect("product id"),
        sku: Sku::new(format!("SKU-{suffix}")).expect("sku"),
        quantity: quantity((suffix as u32 % 4) + 1),
        product_available,
    }
}

fn user_step_strategy() -> impl Strategy<Value = UserStep> {
    prop_oneof![
        (
            prop_oneof![Just(String::new()), Just("a@example.test".to_owned())],
            prop_oneof![Just(String::new()), Just("Ada".to_owned())],
        )
            .prop_map(|(email, display_name)| UserStep::Register {
                email,
                display_name,
            }),
        Just(UserStep::Activate),
        Just(UserStep::Deactivate),
    ]
}

fn product_step_strategy() -> impl Strategy<Value = ProductStep> {
    prop_oneof![
        (
            prop_oneof![Just(String::new()), Just("Keyboard".to_owned())],
            1u32..=12,
        )
            .prop_map(|(name, initial_quantity)| ProductStep::Create {
                name,
                initial_quantity,
            }),
        (-8i32..=8).prop_map(ProductStep::Adjust),
        (1u32..=8).prop_map(ProductStep::Reserve),
        (1u32..=8).prop_map(ProductStep::Release),
    ]
}

fn order_step_strategy() -> impl Strategy<Value = OrderStep> {
    prop_oneof![
        (any::<bool>(), prop::collection::vec(any::<bool>(), 0..5)).prop_map(
            |(user_active, available_lines)| OrderStep::Place {
                user_active,
                available_lines,
            },
        ),
        Just(OrderStep::Confirm),
        prop_oneof![Just(String::new()), Just("manual review".to_owned())]
            .prop_map(OrderStep::Reject),
        Just(OrderStep::Cancel),
    ]
}

fn user_command(step: UserStep) -> UserCommand {
    match step {
        UserStep::Register {
            email,
            display_name,
        } => UserCommand::RegisterUser {
            user_id: user_id(),
            email,
            display_name,
        },
        UserStep::Activate => UserCommand::ActivateUser { user_id: user_id() },
        UserStep::Deactivate => UserCommand::DeactivateUser { user_id: user_id() },
    }
}

fn product_command(step: ProductStep) -> ProductCommand {
    match step {
        ProductStep::Create {
            name,
            initial_quantity,
        } => ProductCommand::CreateProduct {
            product_id: product_id(),
            sku: sku(),
            name,
            initial_quantity: quantity(initial_quantity),
        },
        ProductStep::Adjust(delta) => ProductCommand::AdjustInventory {
            product_id: product_id(),
            delta,
        },
        ProductStep::Reserve(value) => ProductCommand::ReserveInventory {
            product_id: product_id(),
            quantity: quantity(value),
        },
        ProductStep::Release(value) => ProductCommand::ReleaseInventory {
            product_id: product_id(),
            quantity: quantity(value),
        },
    }
}

fn order_command(step: OrderStep) -> OrderCommand {
    match step {
        OrderStep::Place {
            user_active,
            available_lines,
        } => OrderCommand::PlaceOrder {
            order_id: order_id(),
            user_id: user_id(),
            user_active,
            lines: available_lines
                .into_iter()
                .enumerate()
                .map(|(index, product_available)| order_line(product_available, index + 1))
                .collect(),
        },
        OrderStep::Confirm => OrderCommand::ConfirmOrder {
            order_id: order_id(),
        },
        OrderStep::Reject(reason) => OrderCommand::RejectOrder {
            order_id: order_id(),
            reason,
        },
        OrderStep::Cancel => OrderCommand::CancelOrder {
            order_id: order_id(),
        },
    }
}

#[test]
fn commerce_value_objects_construct_from_valid_inputs() {
    let user_id = UserId::new("user-1").expect("user id");
    let product_id = ProductId::new("product-1").expect("product id");
    let order_id = OrderId::new("order-1").expect("order id");
    let sku = Sku::new("SKU-1").expect("sku");
    let quantity = Quantity::new(1).expect("quantity");

    assert_eq!("user-1", user_id.as_str());
    assert_eq!("product-1", product_id.as_str());
    assert_eq!("order-1", order_id.as_str());
    assert_eq!("SKU-1", sku.as_str());
    assert_eq!(1, quantity.value());
}

proptest! {
    #[test]
    fn user_command_sequence_is_replayable(steps in prop::collection::vec(user_step_strategy(), 0..64)) {
        let mut state = UserState::default();
        let mut events = Vec::new();

        for step in steps {
            let command = user_command(step);
            if let Ok(decision) = User::decide(&state, command, &metadata()) {
                for event in &decision.events {
                    User::apply(&mut state, event);
                    events.push(event.clone());
                }

                match state.status {
                    UserStatus::Unregistered => prop_assert!(state.user_id.is_none()),
                    UserStatus::Active | UserStatus::Inactive => {
                        prop_assert!(state.user_id.is_some());
                    }
                }
            }
        }

        prop_assert_eq!(state, es_kernel::replay::<User>(events.clone()));
        for event in events {
            match event {
                UserEvent::UserRegistered { email, display_name, .. } => {
                    prop_assert!(!email.is_empty());
                    prop_assert!(!display_name.is_empty());
                }
                UserEvent::UserActivated { .. } | UserEvent::UserDeactivated { .. } => {}
            }
        }
    }

    #[test]
    fn product_generated_sequences_keep_inventory_nonnegative(steps in prop::collection::vec(product_step_strategy(), 0..64)) {
        let mut state = ProductState::default();
        let mut events = Vec::new();

        for step in steps {
            let command = product_command(step);
            if let Ok(decision) = Product::decide(&state, command, &metadata()) {
                for event in &decision.events {
                    Product::apply(&mut state, event);
                    events.push(event.clone());
                }

                prop_assert!(state.available_quantity >= 0);
                prop_assert!(state.reserved_quantity >= 0);
            }
        }

        prop_assert_eq!(state, es_kernel::replay::<Product>(events.clone()));
        for event in events {
            match event {
                ProductEvent::ProductCreated { name, .. } => prop_assert!(!name.is_empty()),
                ProductEvent::InventoryAdjusted { .. }
                | ProductEvent::InventoryReserved { .. }
                | ProductEvent::InventoryReleased { .. } => {}
            }
        }
    }

    #[test]
    fn order_command_sequence_is_replayable(steps in prop::collection::vec(order_step_strategy(), 0..64)) {
        let mut state = OrderState::default();
        let mut events = Vec::new();
        let unavailable_probe = OrderCommand::PlaceOrder {
            order_id: order_id(),
            user_id: user_id(),
            user_active: true,
            lines: vec![order_line(false, 99)],
        };

        prop_assert_eq!(
            OrderError::UnavailableProduct {
                product_id: ProductId::new("product-99").expect("product id"),
            },
            Order::decide(&state, unavailable_probe, &metadata()).expect_err("unavailable product")
        );

        for step in steps {
            let command = order_command(step);
            if let Ok(decision) = Order::decide(&state, command, &metadata()) {
                for event in &decision.events {
                    Order::apply(&mut state, event);
                    events.push(event.clone());
                }

                match state.status {
                    OrderStatus::Draft => prop_assert!(state.order_id.is_none()),
                    OrderStatus::Placed
                    | OrderStatus::Confirmed
                    | OrderStatus::Rejected
                    | OrderStatus::Cancelled => {
                        prop_assert!(state.order_id.is_some());
                        prop_assert!(state.user_id.is_some());
                    }
                }
            }
        }

        prop_assert_eq!(state, es_kernel::replay::<Order>(events.clone()));
        for event in events {
            match event {
                OrderEvent::OrderPlaced { lines, .. } => {
                    prop_assert!(!lines.is_empty());
                    prop_assert!(lines.iter().all(|line| line.product_available));
                }
                OrderEvent::OrderRejected { reason, .. } => prop_assert!(!reason.is_empty()),
                OrderEvent::OrderConfirmed { .. } | OrderEvent::OrderCancelled { .. } => {}
            }
        }
    }
}

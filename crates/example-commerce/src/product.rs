use crate::{ProductId, Quantity, Sku};

/// Product aggregate marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product;

/// Product aggregate state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductState {
    /// Product identity, if any.
    pub product_id: Option<ProductId>,
    /// Product stock-keeping unit, if any.
    pub sku: Option<Sku>,
    /// Available quantity tracked by later aggregate behavior.
    pub available_quantity: u32,
}

/// Commands accepted by the product aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductCommand {}

/// Events emitted by the product aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {}

/// Replies returned by product commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductReply {}

/// Product command validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductError {}

impl ProductState {
    /// Returns the available quantity as a typed value when positive.
    pub fn quantity(&self) -> Option<Quantity> {
        Quantity::new(self.available_quantity).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_core::{CommandMetadata, TenantId};
    use es_kernel::Aggregate;
    use proptest::prelude::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    #[derive(Clone, Debug)]
    enum InventoryStep {
        Adjust(i32),
        Reserve(u32),
        Release(u32),
    }

    fn metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: None,
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        }
    }

    fn product_id() -> ProductId {
        ProductId::new("product-1").expect("product id")
    }

    fn sku() -> Sku {
        Sku::new("SKU-1").expect("sku")
    }

    fn create_command(initial_quantity: u32) -> ProductCommand {
        ProductCommand::CreateProduct {
            product_id: product_id(),
            sku: sku(),
            name: "Keyboard".to_owned(),
            initial_quantity: Quantity::new(initial_quantity).expect("quantity"),
        }
    }

    fn created_state(initial_quantity: u32) -> ProductState {
        let command = create_command(initial_quantity);
        let decision =
            Product::decide(&ProductState::default(), command, &metadata()).expect("created");
        let mut state = ProductState::default();
        for event in &decision.events {
            Product::apply(&mut state, event);
        }
        state
    }

    fn inventory_step_strategy() -> impl Strategy<Value = InventoryStep> {
        prop_oneof![
            (-8i32..=8).prop_map(InventoryStep::Adjust),
            (1u32..=8).prop_map(InventoryStep::Reserve),
            (1u32..=8).prop_map(InventoryStep::Release),
        ]
    }

    #[test]
    fn create_product_emits_created_event() {
        let command = create_command(5);
        let decision =
            Product::decide(&ProductState::default(), command, &metadata()).expect("decision");

        assert_eq!(
            vec![ProductEvent::ProductCreated {
                product_id: product_id(),
                sku: sku(),
                name: "Keyboard".to_owned(),
                initial_quantity: Quantity::new(5).expect("quantity"),
            }],
            decision.events
        );
        assert_eq!(
            ProductReply::Created {
                product_id: product_id(),
            },
            decision.reply
        );
    }

    #[test]
    fn reserve_and_release_inventory_updates_replayable_state() {
        let mut state = created_state(5);
        let mut events = vec![ProductEvent::ProductCreated {
            product_id: product_id(),
            sku: sku(),
            name: "Keyboard".to_owned(),
            initial_quantity: Quantity::new(5).expect("quantity"),
        }];

        let reserve = ProductCommand::ReserveInventory {
            product_id: product_id(),
            quantity: Quantity::new(2).expect("quantity"),
        };
        let decision = Product::decide(&state, reserve, &metadata()).expect("reserved");
        assert_eq!(
            vec![ProductEvent::InventoryReserved {
                product_id: product_id(),
                quantity: Quantity::new(2).expect("quantity"),
            }],
            decision.events
        );
        for event in &decision.events {
            Product::apply(&mut state, event);
            events.push(event.clone());
        }
        assert_eq!(3, state.available_quantity);
        assert_eq!(2, state.reserved_quantity);

        let release = ProductCommand::ReleaseInventory {
            product_id: product_id(),
            quantity: Quantity::new(1).expect("quantity"),
        };
        let decision = Product::decide(&state, release, &metadata()).expect("released");
        assert_eq!(
            vec![ProductEvent::InventoryReleased {
                product_id: product_id(),
                quantity: Quantity::new(1).expect("quantity"),
            }],
            decision.events
        );
        for event in &decision.events {
            Product::apply(&mut state, event);
            events.push(event.clone());
        }

        assert_eq!(4, state.available_quantity);
        assert_eq!(1, state.reserved_quantity);
        assert_eq!(state, es_kernel::replay::<Product>(events));
    }

    #[test]
    fn product_rejects_negative_inventory_paths() {
        let state = created_state(5);

        assert_eq!(
            ProductError::InsufficientInventory {
                available: 5,
                requested: 6,
            },
            Product::decide(
                &state,
                ProductCommand::ReserveInventory {
                    product_id: product_id(),
                    quantity: Quantity::new(6).expect("quantity"),
                },
                &metadata()
            )
            .expect_err("insufficient available")
        );

        assert_eq!(
            ProductError::InsufficientReservedInventory {
                reserved: 0,
                requested: 1,
            },
            Product::decide(
                &state,
                ProductCommand::ReleaseInventory {
                    product_id: product_id(),
                    quantity: Quantity::new(1).expect("quantity"),
                },
                &metadata()
            )
            .expect_err("insufficient reserved")
        );

        assert_eq!(
            ProductError::InventoryWouldBeNegative {
                available: 5,
                delta: -6,
            },
            Product::decide(
                &state,
                ProductCommand::AdjustInventory {
                    product_id: product_id(),
                    delta: -6,
                },
                &metadata()
            )
            .expect_err("negative available")
        );
    }

    proptest! {
        #[test]
        fn product_inventory_sequence_never_goes_negative(steps in prop::collection::vec(inventory_step_strategy(), 0..64)) {
            let create = create_command(5);
            let created = Product::decide(&ProductState::default(), create, &metadata()).expect("created");
            let mut state = ProductState::default();
            let mut events = Vec::new();

            for event in &created.events {
                Product::apply(&mut state, event);
                events.push(event.clone());
            }

            for step in steps {
                let command = match step {
                    InventoryStep::Adjust(delta) => ProductCommand::AdjustInventory {
                        product_id: product_id(),
                        delta,
                    },
                    InventoryStep::Reserve(quantity) => ProductCommand::ReserveInventory {
                        product_id: product_id(),
                        quantity: Quantity::new(quantity).expect("quantity"),
                    },
                    InventoryStep::Release(quantity) => ProductCommand::ReleaseInventory {
                        product_id: product_id(),
                        quantity: Quantity::new(quantity).expect("quantity"),
                    },
                };

                if let Ok(decision) = Product::decide(&state, command, &metadata()) {
                    for event in &decision.events {
                        Product::apply(&mut state, event);
                        events.push(event.clone());
                    }
                    prop_assert!(state.available_quantity >= 0);
                    prop_assert!(state.reserved_quantity >= 0);
                }
            }

            prop_assert_eq!(state, es_kernel::replay::<Product>(events));
        }
    }
}

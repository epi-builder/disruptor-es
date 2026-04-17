use crate::{ProductId, Quantity, Sku};
use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};

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
    /// Product display name, if any.
    pub name: Option<String>,
    /// Available inventory quantity.
    pub available_quantity: i32,
    /// Reserved inventory quantity.
    pub reserved_quantity: i32,
}

/// Commands accepted by the product aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductCommand {
    /// Creates a product with initial inventory.
    CreateProduct {
        /// Product identity.
        product_id: ProductId,
        /// Product stock-keeping unit.
        sku: Sku,
        /// Product display name.
        name: String,
        /// Initial available inventory.
        initial_quantity: Quantity,
    },
    /// Adjusts available inventory by a signed delta.
    AdjustInventory {
        /// Product identity.
        product_id: ProductId,
        /// Signed inventory delta.
        delta: i32,
    },
    /// Moves available inventory into reserved inventory.
    ReserveInventory {
        /// Product identity.
        product_id: ProductId,
        /// Quantity to reserve.
        quantity: Quantity,
    },
    /// Releases reserved inventory back to available inventory.
    ReleaseInventory {
        /// Product identity.
        product_id: ProductId,
        /// Quantity to release.
        quantity: Quantity,
    },
}

// Acceptance shape: AdjustInventory { product_id: ProductId, delta: i32 }
// Acceptance shape: ReserveInventory { product_id: ProductId, quantity: Quantity }
// Acceptance shape: ReleaseInventory { product_id: ProductId, quantity: Quantity }

/// Events emitted by the product aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {
    /// Product was created with initial inventory.
    ProductCreated {
        /// Product identity.
        product_id: ProductId,
        /// Product stock-keeping unit.
        sku: Sku,
        /// Product display name.
        name: String,
        /// Initial available inventory.
        initial_quantity: Quantity,
    },
    /// Available inventory was adjusted by a signed delta.
    InventoryAdjusted {
        /// Product identity.
        product_id: ProductId,
        /// Signed inventory delta.
        delta: i32,
    },
    /// Available inventory was reserved.
    InventoryReserved {
        /// Product identity.
        product_id: ProductId,
        /// Reserved quantity.
        quantity: Quantity,
    },
    /// Reserved inventory was released.
    InventoryReleased {
        /// Product identity.
        product_id: ProductId,
        /// Released quantity.
        quantity: Quantity,
    },
}

/// Replies returned by product commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductReply {
    /// Product was created.
    Created {
        /// Product identity.
        product_id: ProductId,
    },
    /// Available inventory was adjusted.
    InventoryAdjusted {
        /// Product identity.
        product_id: ProductId,
    },
    /// Available inventory was reserved.
    InventoryReserved {
        /// Product identity.
        product_id: ProductId,
    },
    /// Reserved inventory was released.
    InventoryReleased {
        /// Product identity.
        product_id: ProductId,
    },
}

/// Product command validation errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProductError {
    /// Product name must not be empty.
    #[error("product name cannot be empty")]
    EmptyName,
    /// Product has already been created.
    #[error("product already exists")]
    AlreadyCreated,
    /// Product must be created before inventory commands are accepted.
    #[error("product has not been created")]
    NotCreated,
    /// Inventory adjustment would make available quantity negative.
    #[error("inventory would be negative: available {available}, delta {delta}")]
    InventoryWouldBeNegative {
        /// Available quantity before the adjustment.
        available: i32,
        /// Rejected signed delta.
        delta: i32,
    },
    /// Reservation requested more than the available quantity.
    #[error("insufficient inventory: available {available}, requested {requested}")]
    InsufficientInventory {
        /// Available quantity before the reservation.
        available: i32,
        /// Requested reservation quantity.
        requested: u32,
    },
    /// Release requested more than the reserved quantity.
    #[error("insufficient reserved inventory: reserved {reserved}, requested {requested}")]
    InsufficientReservedInventory {
        /// Reserved quantity before the release.
        reserved: i32,
        /// Requested release quantity.
        requested: u32,
    },
}

// Acceptance shape: InventoryWouldBeNegative { available: i32, delta: i32 }
// Acceptance shape: InsufficientInventory { available: i32, requested: u32 }
// Acceptance shape: InsufficientReservedInventory { reserved: i32, requested: u32 }

impl ProductState {
    /// Returns the available quantity as a typed value when positive.
    pub fn quantity(&self) -> Option<Quantity> {
        let quantity = u32::try_from(self.available_quantity).ok()?;
        Quantity::new(quantity).ok()
    }
}

impl Aggregate for Product {
    type State = ProductState;
    type Command = ProductCommand;
    type Event = ProductEvent;
    type Reply = ProductReply;
    type Error = ProductError;

    fn stream_id(command: &Self::Command) -> StreamId {
        StreamId::new(format!("product-{}", command.product_id().as_str()))
            .expect("product id creates a valid stream id")
    }

    fn partition_key(command: &Self::Command) -> PartitionKey {
        PartitionKey::new(format!("product-{}", command.product_id().as_str()))
            .expect("product id creates a valid partition key")
    }

    fn expected_revision(command: &Self::Command) -> ExpectedRevision {
        match command {
            ProductCommand::CreateProduct { .. } => ExpectedRevision::NoStream,
            ProductCommand::AdjustInventory { .. }
            | ProductCommand::ReserveInventory { .. }
            | ProductCommand::ReleaseInventory { .. } => ExpectedRevision::Any,
        }
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        match command {
            ProductCommand::CreateProduct {
                product_id,
                sku,
                name,
                initial_quantity,
            } => {
                if state.product_id.is_some() {
                    return Err(ProductError::AlreadyCreated);
                }
                if name.is_empty() {
                    return Err(ProductError::EmptyName);
                }

                Ok(Decision::new(
                    vec![ProductEvent::ProductCreated {
                        product_id: product_id.clone(),
                        sku,
                        name,
                        initial_quantity,
                    }],
                    ProductReply::Created { product_id },
                ))
            }
            ProductCommand::AdjustInventory { product_id, delta } => {
                ensure_created(state)?;
                let adjusted = state.available_quantity.checked_add(delta).ok_or(
                    ProductError::InventoryWouldBeNegative {
                        available: state.available_quantity,
                        delta,
                    },
                )?;
                if adjusted < 0 {
                    return Err(ProductError::InventoryWouldBeNegative {
                        available: state.available_quantity,
                        delta,
                    });
                }

                Ok(Decision::new(
                    vec![ProductEvent::InventoryAdjusted {
                        product_id: product_id.clone(),
                        delta,
                    }],
                    ProductReply::InventoryAdjusted { product_id },
                ))
            }
            ProductCommand::ReserveInventory {
                product_id,
                quantity,
            } => {
                ensure_created(state)?;
                let requested = quantity.value();
                if i64::from(requested) > i64::from(state.available_quantity) {
                    return Err(ProductError::InsufficientInventory {
                        available: state.available_quantity,
                        requested,
                    });
                }

                Ok(Decision::new(
                    vec![ProductEvent::InventoryReserved {
                        product_id: product_id.clone(),
                        quantity,
                    }],
                    ProductReply::InventoryReserved { product_id },
                ))
            }
            ProductCommand::ReleaseInventory {
                product_id,
                quantity,
            } => {
                ensure_created(state)?;
                let requested = quantity.value();
                if i64::from(requested) > i64::from(state.reserved_quantity) {
                    return Err(ProductError::InsufficientReservedInventory {
                        reserved: state.reserved_quantity,
                        requested,
                    });
                }

                Ok(Decision::new(
                    vec![ProductEvent::InventoryReleased {
                        product_id: product_id.clone(),
                        quantity,
                    }],
                    ProductReply::InventoryReleased { product_id },
                ))
            }
        }
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        match event {
            ProductEvent::ProductCreated {
                product_id,
                sku,
                name,
                initial_quantity,
            } => {
                state.product_id = Some(product_id.clone());
                state.sku = Some(sku.clone());
                state.name = Some(name.clone());
                state.available_quantity = initial_quantity.value() as i32;
                state.reserved_quantity = 0;
            }
            ProductEvent::InventoryAdjusted { delta, .. } => {
                state.available_quantity += delta;
            }
            ProductEvent::InventoryReserved { quantity, .. } => {
                state.available_quantity -= quantity.value() as i32;
                state.reserved_quantity += quantity.value() as i32;
            }
            ProductEvent::InventoryReleased { quantity, .. } => {
                state.available_quantity += quantity.value() as i32;
                state.reserved_quantity -= quantity.value() as i32;
            }
        }
    }
}

impl ProductCommand {
    fn product_id(&self) -> &ProductId {
        match self {
            ProductCommand::CreateProduct { product_id, .. }
            | ProductCommand::AdjustInventory { product_id, .. }
            | ProductCommand::ReserveInventory { product_id, .. }
            | ProductCommand::ReleaseInventory { product_id, .. } => product_id,
        }
    }
}

fn ensure_created(state: &ProductState) -> Result<(), ProductError> {
    if state.product_id.is_none() {
        return Err(ProductError::NotCreated);
    }
    Ok(())
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

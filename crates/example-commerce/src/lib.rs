//! Minimal commerce aggregate fixture for the typed event-sourcing kernel.

mod ids;

pub use ids::{OrderId, ProductId, Quantity, Sku, UserId};

use es_core::{CommandMetadata, ExpectedRevision, PartitionKey, StreamId};
use es_kernel::{Aggregate, Decision};

/// Product draft aggregate marker.
pub struct ProductDraft;

/// Product draft state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductState {
    /// Created product SKU, if any.
    pub sku: Option<String>,
    /// Created product name, if any.
    pub name: Option<String>,
}

/// Commands accepted by the product draft aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductCommand {
    /// Creates a new product draft.
    CreateProduct {
        /// Stream that owns the product draft.
        stream_id: StreamId,
        /// Product SKU.
        sku: String,
        /// Product display name.
        name: String,
    },
}

/// Events emitted by the product draft aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {
    /// Product draft was created.
    ProductCreated {
        /// Product SKU.
        sku: String,
        /// Product display name.
        name: String,
    },
}

/// Replies returned by product commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductReply {
    /// Product draft creation succeeded.
    Created {
        /// Stream that owns the created product draft.
        stream_id: StreamId,
    },
}

/// Product command validation errors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProductError {
    /// SKU must not be empty.
    #[error("product SKU cannot be empty")]
    EmptySku,
    /// Name must not be empty.
    #[error("product name cannot be empty")]
    EmptyName,
    /// Product draft has already been created.
    #[error("product draft already exists")]
    AlreadyCreated,
}

impl Aggregate for ProductDraft {
    type State = ProductState;
    type Command = ProductCommand;
    type Event = ProductEvent;
    type Reply = ProductReply;
    type Error = ProductError;

    fn stream_id(command: &Self::Command) -> StreamId {
        match command {
            ProductCommand::CreateProduct { stream_id, .. } => stream_id.clone(),
        }
    }

    fn partition_key(command: &Self::Command) -> PartitionKey {
        match command {
            ProductCommand::CreateProduct { stream_id, .. } => {
                PartitionKey::new(stream_id.as_str()).expect("stream id is a valid partition key")
            }
        }
    }

    fn expected_revision(_command: &Self::Command) -> ExpectedRevision {
        ExpectedRevision::NoStream
    }

    fn decide(
        state: &Self::State,
        command: Self::Command,
        _metadata: &CommandMetadata,
    ) -> Result<Decision<Self::Event, Self::Reply>, Self::Error> {
        if state.sku.is_some() {
            return Err(ProductError::AlreadyCreated);
        }

        match command {
            ProductCommand::CreateProduct {
                stream_id,
                sku,
                name,
            } => {
                if sku.is_empty() {
                    return Err(ProductError::EmptySku);
                }
                if name.is_empty() {
                    return Err(ProductError::EmptyName);
                }

                Ok(Decision::new(
                    vec![ProductEvent::ProductCreated { sku, name }],
                    ProductReply::Created { stream_id },
                ))
            }
        }
    }

    fn apply(state: &mut Self::State, event: &Self::Event) {
        match event {
            ProductEvent::ProductCreated { sku, name } => {
                state.sku = Some(sku.clone());
                state.name = Some(name.clone());
            }
        }
    }
}

#[cfg(test)]
mod aggregate_contract {
    use super::*;
    use es_core::TenantId;
    use proptest::prelude::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn metadata() -> CommandMetadata {
        CommandMetadata {
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: None,
            tenant_id: TenantId::new("tenant-a").expect("tenant id"),
            requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
        }
    }

    fn create_command(sku: impl Into<String>, name: impl Into<String>) -> ProductCommand {
        ProductCommand::CreateProduct {
            stream_id: StreamId::new("product-1").expect("stream id"),
            sku: sku.into(),
            name: name.into(),
        }
    }

    #[test]
    fn decide_valid_create_returns_event_and_reply() {
        let command = create_command("SKU-1", "Keyboard");
        let decision =
            ProductDraft::decide(&ProductState::default(), command, &metadata()).expect("decision");

        assert_eq!(
            vec![ProductEvent::ProductCreated {
                sku: "SKU-1".to_owned(),
                name: "Keyboard".to_owned(),
            }],
            decision.events
        );
        assert_eq!(
            ProductReply::Created {
                stream_id: StreamId::new("product-1").expect("stream id"),
            },
            decision.reply
        );
    }

    #[test]
    fn decide_rejects_empty_sku_and_name() {
        assert_eq!(
            ProductError::EmptySku,
            ProductDraft::decide(
                &ProductState::default(),
                create_command("", "Keyboard"),
                &metadata()
            )
            .expect_err("empty sku")
        );

        assert_eq!(
            ProductError::EmptyName,
            ProductDraft::decide(
                &ProductState::default(),
                create_command("SKU-1", ""),
                &metadata()
            )
            .expect_err("empty name")
        );
    }

    #[test]
    fn decide_rejects_second_create_after_apply() {
        let mut state = ProductState::default();
        ProductDraft::apply(
            &mut state,
            &ProductEvent::ProductCreated {
                sku: "SKU-1".to_owned(),
                name: "Keyboard".to_owned(),
            },
        );

        assert_eq!(
            ProductError::AlreadyCreated,
            ProductDraft::decide(&state, create_command("SKU-2", "Mouse"), &metadata())
                .expect_err("already created")
        );
    }

    proptest! {
        #[test]
        fn replay_matches_manual_application(events in prop::collection::vec(("[A-Z0-9]{1,8}", "[A-Za-z0-9 ]{1,24}"), 1..16)) {
            let events: Vec<ProductEvent> = events
                .into_iter()
                .map(|(sku, name)| ProductEvent::ProductCreated { sku, name })
                .collect();

            let replayed = es_kernel::replay::<ProductDraft>(events.clone());
            let mut manually_applied = ProductState::default();
            for event in &events {
                ProductDraft::apply(&mut manually_applied, event);
            }

            prop_assert_eq!(manually_applied, replayed);
        }
    }
}

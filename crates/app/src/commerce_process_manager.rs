use std::future::Future;
use std::pin::Pin;

use es_core::CommandMetadata;
use es_outbox::{
    OutboxError, OutboxResult, ProcessEvent, ProcessManager, ProcessManagerName, ProcessOutcome,
};
use es_runtime::{CommandEnvelope, CommandGateway};
use example_commerce::{Order, OrderCommand, OrderEvent, Product, ProductCommand};
use uuid::Uuid;

/// Process manager coordinating order placement with product inventory reservation.
pub struct CommerceOrderProcessManager {
    name: ProcessManagerName,
    product_gateway: CommandGateway<Product>,
    order_gateway: CommandGateway<Order>,
}

impl CommerceOrderProcessManager {
    /// Creates a commerce order process manager.
    pub fn new(
        name: ProcessManagerName,
        product_gateway: CommandGateway<Product>,
        order_gateway: CommandGateway<Order>,
    ) -> Self {
        Self {
            name,
            product_gateway,
            order_gateway,
        }
    }
}

impl ProcessManager for CommerceOrderProcessManager {
    fn name(&self) -> &ProcessManagerName {
        &self.name
    }

    fn handles(&self, event_type: &str, schema_version: i32) -> bool {
        event_type == "OrderPlaced" && schema_version == 1
    }

    fn process<'a>(
        &'a self,
        event: &'a ProcessEvent,
    ) -> Pin<Box<dyn Future<Output = OutboxResult<ProcessOutcome>> + Send + 'a>> {
        Box::pin(async move {
            if !self.handles(&event.event_type, event.schema_version) {
                return Ok(ProcessOutcome::Skipped {
                    global_position: event.global_position,
                });
            }

            let OrderEvent::OrderPlaced {
                order_id,
                user_id: _,
                lines,
            } = decode_order_placed(event)?
            else {
                return Ok(ProcessOutcome::Skipped {
                    global_position: event.global_position,
                });
            };

            let mut command_count = 0;
            let mut inventory_reserved = true;
            for line in lines {
                let (reply, receiver) = tokio::sync::oneshot::channel();
                let product_id = line.product_id.clone();
                let envelope = CommandEnvelope::<Product>::new(
                    ProductCommand::ReserveInventory {
                        product_id: product_id.clone(),
                        quantity: line.quantity,
                    },
                    follow_up_metadata(event),
                    format!(
                        "pm:{}:{}:reserve:{}",
                        self.name.as_str(),
                        event.event_id,
                        product_id.as_str()
                    ),
                    reply,
                )
                .map_err(command_submit_error)?;
                self.product_gateway
                    .try_submit(envelope)
                    .map_err(command_submit_error)?;
                command_count += 1;

                match receiver
                    .await
                    .map_err(|_| OutboxError::CommandReplyDropped)?
                {
                    Ok(_) => {}
                    Err(_) => {
                        inventory_reserved = false;
                        break;
                    }
                }
            }

            let (reply, receiver) = tokio::sync::oneshot::channel();
            let (command, idempotency_key) = if inventory_reserved {
                (
                    OrderCommand::ConfirmOrder {
                        order_id: order_id.clone(),
                    },
                    format!(
                        "pm:{}:{}:confirm:{}",
                        self.name.as_str(),
                        event.event_id,
                        order_id.as_str()
                    ),
                )
            } else {
                (
                    OrderCommand::RejectOrder {
                        order_id: order_id.clone(),
                        reason: "inventory reservation failed".to_owned(),
                    },
                    format!(
                        "pm:{}:{}:reject:{}",
                        self.name.as_str(),
                        event.event_id,
                        order_id.as_str()
                    ),
                )
            };
            let envelope = CommandEnvelope::<Order>::new(
                command,
                follow_up_metadata(event),
                idempotency_key,
                reply,
            )
            .map_err(command_submit_error)?;
            self.order_gateway
                .try_submit(envelope)
                .map_err(command_submit_error)?;
            command_count += 1;
            receiver
                .await
                .map_err(|_| OutboxError::CommandReplyDropped)?
                .map_err(command_submit_error)?;

            Ok(ProcessOutcome::CommandsSubmitted {
                global_position: event.global_position,
                command_count,
            })
        })
    }
}

fn decode_order_placed(event: &ProcessEvent) -> OutboxResult<OrderEvent> {
    serde_json::from_value(event.payload.clone()).map_err(|_| OutboxError::PayloadDecode {
        event_type: event.event_type.clone(),
        schema_version: event.schema_version,
    })
}

fn follow_up_metadata(event: &ProcessEvent) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::now_v7(),
        correlation_id: event.correlation_id,
        causation_id: Some(event.event_id),
        tenant_id: event.tenant_id.clone(),
        requested_at: time::OffsetDateTime::now_utc(),
    }
}

fn command_submit_error(error: impl std::fmt::Display) -> OutboxError {
    OutboxError::CommandSubmit {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_core::TenantId;
    use es_runtime::{
        CommandGateway, CommandOutcome, PartitionRouter, RoutedCommand, RuntimeError,
    };
    use example_commerce::{
        OrderId, OrderLine, OrderReply, ProductId, ProductReply, Quantity, Sku, UserId,
    };
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio::time::{Duration, timeout};

    fn tenant() -> TenantId {
        TenantId::new("tenant-a").expect("tenant id")
    }

    fn process_manager_name() -> ProcessManagerName {
        ProcessManagerName::new("commerce-order").expect("process manager name")
    }

    fn product_id(value: &str) -> ProductId {
        ProductId::new(value).expect("product id")
    }

    fn order_id() -> OrderId {
        OrderId::new("order-1").expect("order id")
    }

    fn line(product: ProductId) -> OrderLine {
        OrderLine {
            product_id: product,
            sku: Sku::new("SKU-1").expect("sku"),
            quantity: Quantity::new(2).expect("quantity"),
            product_available: true,
        }
    }

    fn process_event(event: OrderEvent) -> ProcessEvent {
        ProcessEvent {
            global_position: 42,
            event_id: Uuid::from_u128(42),
            event_type: "OrderPlaced".to_owned(),
            schema_version: 1,
            payload: serde_json::to_value(event).expect("order event payload"),
            metadata: json!({ "source": "commerce-process-manager-test" }),
            tenant_id: tenant(),
            command_id: Uuid::from_u128(100),
            correlation_id: Uuid::from_u128(101),
            causation_id: Some(Uuid::from_u128(102)),
        }
    }

    fn committed_append(
        global_position: i64,
        event_id: Uuid,
    ) -> es_store_postgres::CommittedAppend {
        es_store_postgres::CommittedAppend {
            stream_id: es_core::StreamId::new(format!("stream-{global_position}"))
                .expect("stream id"),
            first_revision: es_core::StreamRevision::new(1),
            last_revision: es_core::StreamRevision::new(1),
            global_positions: vec![global_position],
            event_ids: vec![event_id],
        }
    }

    fn gateways() -> (
        CommandGateway<Product>,
        mpsc::Receiver<RoutedCommand<Product>>,
        CommandGateway<Order>,
        mpsc::Receiver<RoutedCommand<Order>>,
    ) {
        let router = PartitionRouter::new(4).expect("router");
        let (product_gateway, product_rx) =
            CommandGateway::<Product>::new(router.clone(), 8).expect("product gateway");
        let (order_gateway, order_rx) =
            CommandGateway::<Order>::new(router, 8).expect("order gateway");
        (product_gateway, product_rx, order_gateway, order_rx)
    }

    async fn receive_product(
        product_rx: &mut mpsc::Receiver<RoutedCommand<Product>>,
    ) -> RoutedCommand<Product> {
        timeout(Duration::from_millis(100), product_rx.recv())
            .await
            .expect("product command received before timeout")
            .expect("product command")
    }

    async fn receive_order(
        order_rx: &mut mpsc::Receiver<RoutedCommand<Order>>,
    ) -> RoutedCommand<Order> {
        timeout(Duration::from_millis(100), order_rx.recv())
            .await
            .expect("order command received before timeout")
            .expect("order command")
    }

    #[tokio::test]
    async fn process_manager_skips_unhandled_events() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let mut event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(product_id("product-1"))],
        });
        event.event_type = "OrderConfirmed".to_owned();

        let outcome = manager.process(&event).await?;

        assert_eq!(
            ProcessOutcome::Skipped {
                global_position: 42
            },
            outcome
        );
        assert!(product_rx.try_recv().is_err());
        assert!(order_rx.try_recv().is_err());

        Ok(())
    }

    #[tokio::test]
    async fn order_placed_submits_reserve_then_confirm_commands() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let product = product_id("product-1");
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(product.clone())],
        });

        let process_event = event.clone();
        let task = tokio::spawn(async move { manager.process(&process_event).await });

        let reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            reserve.envelope.command
        );
        assert_eq!(tenant(), reserve.envelope.metadata.tenant_id);
        assert_eq!(
            event.correlation_id,
            reserve.envelope.metadata.correlation_id
        );
        assert_eq!(Some(event.event_id), reserve.envelope.metadata.causation_id);
        assert!(
            reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: product,
                    },
                    committed_append(43, Uuid::from_u128(43)),
                )))
                .is_ok()
        );

        let confirm = receive_order(&mut order_rx).await;
        assert_eq!(
            OrderCommand::ConfirmOrder {
                order_id: order_id()
            },
            confirm.envelope.command
        );
        assert!(
            confirm
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    OrderReply::Confirmed {
                        order_id: order_id(),
                    },
                    committed_append(44, Uuid::from_u128(44)),
                )))
                .is_ok()
        );

        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: 42,
                command_count: 2
            },
            task.await.expect("process task")?
        );

        Ok(())
    }

    #[tokio::test]
    async fn reserve_failure_submits_reject_command() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(product_id("product-1"))],
        });

        let process_event = event.clone();
        let task = tokio::spawn(async move { manager.process(&process_event).await });

        let reserve = receive_product(&mut product_rx).await;
        assert!(
            reserve
                .envelope
                .reply
                .send(Err(RuntimeError::Domain {
                    message: "insufficient inventory".to_owned(),
                }))
                .is_ok()
        );

        let reject = receive_order(&mut order_rx).await;
        assert_eq!(
            OrderCommand::RejectOrder {
                order_id: order_id(),
                reason: "inventory reservation failed".to_owned()
            },
            reject.envelope.command
        );
        assert!(
            reject
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    OrderReply::Rejected {
                        order_id: order_id(),
                    },
                    committed_append(45, Uuid::from_u128(45)),
                )))
                .is_ok()
        );

        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: 42,
                command_count: 2
            },
            task.await.expect("process task")?
        );

        Ok(())
    }

    #[tokio::test]
    async fn process_manager_uses_deterministic_idempotency_keys() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let product = product_id("product-1");
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(product.clone())],
        });

        let process_event = event.clone();
        let task = tokio::spawn(async move { manager.process(&process_event).await });

        let reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            format!(
                "pm:{}:{}:reserve:{}",
                process_manager_name().as_str(),
                event.event_id,
                product.as_str()
            ),
            reserve.envelope.idempotency_key
        );
        assert!(
            reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: product,
                    },
                    committed_append(43, Uuid::from_u128(43)),
                )))
                .is_ok()
        );

        let confirm = receive_order(&mut order_rx).await;
        assert_eq!(
            format!(
                "pm:{}:{}:confirm:{}",
                process_manager_name().as_str(),
                event.event_id,
                order_id().as_str()
            ),
            confirm.envelope.idempotency_key
        );
        assert!(
            confirm
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    OrderReply::Confirmed {
                        order_id: order_id(),
                    },
                    committed_append(44, Uuid::from_u128(44)),
                )))
                .is_ok()
        );

        task.await.expect("process task")?;

        Ok(())
    }

    #[tokio::test]
    async fn process_manager_waits_for_replies_before_success() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let product = product_id("product-1");
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(product.clone())],
        });

        let process_event = event.clone();
        let mut task = tokio::spawn(async move { manager.process(&process_event).await });
        let reserve = receive_product(&mut product_rx).await;

        assert!(
            timeout(Duration::from_millis(10), &mut task).await.is_err(),
            "process should wait for reserve reply"
        );

        assert!(
            reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: product,
                    },
                    committed_append(43, Uuid::from_u128(43)),
                )))
                .is_ok()
        );

        let confirm = receive_order(&mut order_rx).await;
        assert!(
            timeout(Duration::from_millis(10), &mut task).await.is_err(),
            "process should wait for confirm reply"
        );
        assert!(
            confirm
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    OrderReply::Confirmed {
                        order_id: order_id(),
                    },
                    committed_append(44, Uuid::from_u128(44)),
                )))
                .is_ok()
        );
        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: 42,
                command_count: 2
            },
            task.await.expect("process task")?
        );

        Ok(())
    }
}

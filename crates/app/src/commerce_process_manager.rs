use es_core::{CommandMetadata, TenantId};
use es_outbox::{OutboxResult, ProcessEvent, ProcessManager, ProcessManagerName, ProcessOutcome};
use es_runtime::{
    CommandGateway, CommandOutcome, PartitionRouter, RoutedCommand, RuntimeError, RuntimeResult,
};
use example_commerce::{
    Order, OrderCommand, OrderEvent, OrderId, OrderLine, OrderReply, Product, ProductCommand,
    ProductId, ProductReply, Quantity, Sku, UserId,
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::OffsetDateTime;
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

        let process = manager.process(&event);
        tokio::pin!(process);

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
        reserve
            .envelope
            .reply
            .send(Ok(CommandOutcome::new(
                ProductReply::InventoryReserved {
                    product_id: product,
                },
                committed_append(43, Uuid::from_u128(43)),
            )))
            .expect("send reserve reply");

        let confirm = receive_order(&mut order_rx).await;
        assert_eq!(
            OrderCommand::ConfirmOrder {
                order_id: order_id()
            },
            confirm.envelope.command
        );
        confirm
            .envelope
            .reply
            .send(Ok(CommandOutcome::new(
                OrderReply::Confirmed {
                    order_id: order_id(),
                },
                committed_append(44, Uuid::from_u128(44)),
            )))
            .expect("send confirm reply");

        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: 42,
                command_count: 2
            },
            process.await?
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

        let process = manager.process(&event);
        tokio::pin!(process);

        let reserve = receive_product(&mut product_rx).await;
        reserve
            .envelope
            .reply
            .send(Err(RuntimeError::Domain {
                message: "insufficient inventory".to_owned(),
            }))
            .expect("send failed reserve reply");

        let reject = receive_order(&mut order_rx).await;
        assert_eq!(
            OrderCommand::RejectOrder {
                order_id: order_id(),
                reason: "inventory reservation failed".to_owned()
            },
            reject.envelope.command
        );
        reject
            .envelope
            .reply
            .send(Ok(CommandOutcome::new(
                OrderReply::Rejected {
                    order_id: order_id(),
                },
                committed_append(45, Uuid::from_u128(45)),
            )))
            .expect("send reject reply");

        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: 42,
                command_count: 2
            },
            process.await?
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

        let process = manager.process(&event);
        tokio::pin!(process);

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
        reserve
            .envelope
            .reply
            .send(Ok(CommandOutcome::new(
                ProductReply::InventoryReserved {
                    product_id: product,
                },
                committed_append(43, Uuid::from_u128(43)),
            )))
            .expect("send reserve reply");

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
        confirm
            .envelope
            .reply
            .send(Ok(CommandOutcome::new(
                OrderReply::Confirmed {
                    order_id: order_id(),
                },
                committed_append(44, Uuid::from_u128(44)),
            )))
            .expect("send confirm reply");

        process.await?;

        Ok(())
    }

    #[tokio::test]
    async fn process_manager_waits_for_replies_before_success() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, _order_rx) = gateways();
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

        let process = manager.process(&event);
        tokio::pin!(process);
        let reserve = receive_product(&mut product_rx).await;

        assert!(
            timeout(Duration::from_millis(10), &mut process)
                .await
                .is_err(),
            "process should wait for reserve reply"
        );

        reserve
            .envelope
            .reply
            .send(Ok(CommandOutcome::new(
                ProductReply::InventoryReserved {
                    product_id: product,
                },
                committed_append(43, Uuid::from_u128(43)),
            )))
            .expect("send reserve reply");

        Ok(())
    }
}

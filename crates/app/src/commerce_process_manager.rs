use std::future::Future;
use std::pin::Pin;

use es_core::CommandMetadata;
use es_outbox::{
    OutboxError, OutboxResult, ProcessEvent, ProcessManager, ProcessManagerName, ProcessOutcome,
};
use es_runtime::{CommandEnvelope, CommandGateway};
use example_commerce::{Order, OrderCommand, OrderEvent, Product, ProductCommand, ProductId};
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
            let mut reserved_lines = Vec::new();
            for (line_index, line) in lines.into_iter().enumerate() {
                let (reply, receiver) = tokio::sync::oneshot::channel();
                let product_id = line.product_id.clone();
                let quantity = line.quantity;
                let envelope = CommandEnvelope::<Product>::new(
                    ProductCommand::ReserveInventory {
                        product_id: product_id.clone(),
                        quantity,
                    },
                    follow_up_metadata(event),
                    follow_up_line_key(
                        &self.name,
                        event.event_id,
                        "reserve",
                        line_index,
                        &product_id,
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
                    Ok(_) => {
                        reserved_lines.push((line_index, product_id, quantity));
                    }
                    Err(_) => {
                        inventory_reserved = false;
                        break;
                    }
                }
            }

            if !inventory_reserved {
                for (line_index, product_id, quantity) in reserved_lines {
                    let (reply, receiver) = tokio::sync::oneshot::channel();
                    let envelope = CommandEnvelope::<Product>::new(
                        ProductCommand::ReleaseInventory {
                            product_id: product_id.clone(),
                            quantity,
                        },
                        follow_up_metadata(event),
                        follow_up_line_key(
                            &self.name,
                            event.event_id,
                            "release",
                            line_index,
                            &product_id,
                        ),
                        reply,
                    )
                    .map_err(command_submit_error)?;
                    self.product_gateway
                        .try_submit(envelope)
                        .map_err(command_submit_error)?;
                    command_count += 1;
                    receiver
                        .await
                        .map_err(|_| OutboxError::CommandReplyDropped)?
                        .map_err(command_submit_error)?;
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

fn follow_up_line_key(
    manager: &ProcessManagerName,
    source_event_id: Uuid,
    action: &str,
    line_index: usize,
    product_id: &ProductId,
) -> String {
    format!(
        "pm:{}:{}:{}:{}:{}",
        manager.as_str(),
        source_event_id,
        action,
        line_index,
        product_id.as_str()
    )
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
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use super::*;
    use es_core::{StreamId, StreamRevision, TenantId};
    use es_runtime::{
        CommandEngine, CommandEngineConfig, CommandGateway, CommandOutcome, PartitionRouter,
        RoutedCommand, RuntimeError, RuntimeEventCodec, RuntimeEventStore,
    };
    use es_store_postgres::{
        AppendOutcome, AppendRequest, CommandReplayRecord, CommandReplyPayload, NewEvent,
        RehydrationBatch, SnapshotRecord, StoreResult, StoredEvent,
    };
    use example_commerce::{
        OrderId, OrderLine, OrderReply, OrderState, ProductEvent, ProductId, ProductReply,
        ProductState, Quantity, Sku, UserId,
    };
    use futures::future::BoxFuture;
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

    #[derive(Clone, Copy, Debug)]
    struct TestProductCodec;

    impl RuntimeEventCodec<Product> for TestProductCodec {
        fn encode(
            &self,
            event: &ProductEvent,
            _metadata: &CommandMetadata,
        ) -> es_runtime::RuntimeResult<NewEvent> {
            let event_type = match event {
                ProductEvent::ProductCreated { .. } => "ProductCreated",
                ProductEvent::InventoryAdjusted { .. } => "InventoryAdjusted",
                ProductEvent::InventoryReserved { .. } => "InventoryReserved",
                ProductEvent::InventoryReleased { .. } => "InventoryReleased",
            };
            NewEvent::new(
                Uuid::from_u128(20),
                event_type,
                1,
                serde_json::to_value(event).map_err(|error| RuntimeError::Codec {
                    message: error.to_string(),
                })?,
                json!({ "codec": "test-product" }),
            )
            .map_err(RuntimeError::from_store_error)
        }

        fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<ProductEvent> {
            serde_json::from_value(stored.payload.clone()).map_err(|error| RuntimeError::Codec {
                message: error.to_string(),
            })
        }

        fn decode_snapshot(
            &self,
            _snapshot: &SnapshotRecord,
        ) -> es_runtime::RuntimeResult<ProductState> {
            Ok(ProductState::default())
        }

        fn encode_reply(
            &self,
            reply: &ProductReply,
        ) -> es_runtime::RuntimeResult<CommandReplyPayload> {
            let payload = serde_json::to_value(reply).map_err(|error| RuntimeError::Codec {
                message: error.to_string(),
            })?;
            CommandReplyPayload::new("product_reply", 1, payload)
                .map_err(RuntimeError::from_store_error)
        }

        fn decode_reply(
            &self,
            payload: &CommandReplyPayload,
        ) -> es_runtime::RuntimeResult<ProductReply> {
            if payload.reply_type != "product_reply" {
                return Err(RuntimeError::Codec {
                    message: format!("unexpected reply type {}", payload.reply_type),
                });
            }
            if payload.schema_version != 1 {
                return Err(RuntimeError::Codec {
                    message: format!("unexpected reply schema version {}", payload.schema_version),
                });
            }

            serde_json::from_value::<ProductReply>(payload.payload.clone()).map_err(|error| {
                RuntimeError::Codec {
                    message: error.to_string(),
                }
            })
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct TestOrderCodec;

    impl RuntimeEventCodec<Order> for TestOrderCodec {
        fn encode(
            &self,
            event: &OrderEvent,
            _metadata: &CommandMetadata,
        ) -> es_runtime::RuntimeResult<NewEvent> {
            let event_type = match event {
                OrderEvent::OrderPlaced { .. } => "OrderPlaced",
                OrderEvent::OrderConfirmed { .. } => "OrderConfirmed",
                OrderEvent::OrderRejected { .. } => "OrderRejected",
                OrderEvent::OrderCancelled { .. } => "OrderCancelled",
            };
            NewEvent::new(
                Uuid::from_u128(21),
                event_type,
                1,
                serde_json::to_value(event).map_err(|error| RuntimeError::Codec {
                    message: error.to_string(),
                })?,
                json!({ "codec": "test-order" }),
            )
            .map_err(RuntimeError::from_store_error)
        }

        fn decode(&self, stored: &StoredEvent) -> es_runtime::RuntimeResult<OrderEvent> {
            serde_json::from_value(stored.payload.clone()).map_err(|error| RuntimeError::Codec {
                message: error.to_string(),
            })
        }

        fn decode_snapshot(
            &self,
            _snapshot: &SnapshotRecord,
        ) -> es_runtime::RuntimeResult<OrderState> {
            Ok(OrderState::default())
        }

        fn encode_reply(
            &self,
            reply: &OrderReply,
        ) -> es_runtime::RuntimeResult<CommandReplyPayload> {
            let payload = serde_json::to_value(reply).map_err(|error| RuntimeError::Codec {
                message: error.to_string(),
            })?;
            CommandReplyPayload::new("order_reply", 1, payload)
                .map_err(RuntimeError::from_store_error)
        }

        fn decode_reply(
            &self,
            payload: &CommandReplyPayload,
        ) -> es_runtime::RuntimeResult<OrderReply> {
            if payload.reply_type != "order_reply" {
                return Err(RuntimeError::Codec {
                    message: format!("unexpected reply type {}", payload.reply_type),
                });
            }
            if payload.schema_version != 1 {
                return Err(RuntimeError::Codec {
                    message: format!("unexpected reply schema version {}", payload.schema_version),
                });
            }

            serde_json::from_value::<OrderReply>(payload.payload.clone()).map_err(|error| {
                RuntimeError::Codec {
                    message: error.to_string(),
                }
            })
        }
    }

    #[derive(Clone)]
    struct ReplayAwareProductStore {
        inner: Arc<ReplayAwareStoreInner>,
    }

    #[derive(Clone)]
    struct ReplayAwareOrderStore {
        inner: Arc<ReplayAwareStoreInner>,
    }

    struct ReplayAwareStoreInner {
        global_position: i64,
        event_id: Uuid,
        rehydration_events: Vec<StoredEvent>,
        append_requests: Mutex<Vec<AppendRequest>>,
        replay_records: Mutex<BTreeMap<String, CommandReplayRecord>>,
        lookup_count: Mutex<usize>,
    }

    impl ReplayAwareProductStore {
        fn new(product: ProductId) -> Self {
            Self {
                inner: Arc::new(ReplayAwareStoreInner {
                    global_position: 20,
                    event_id: Uuid::from_u128(20),
                    rehydration_events: vec![stored_event(
                        "product-product-1",
                        "ProductCreated",
                        ProductEvent::ProductCreated {
                            product_id: product,
                            sku: Sku::new("SKU-1").expect("sku"),
                            name: "Keyboard".to_owned(),
                            initial_quantity: Quantity::new(10).expect("quantity"),
                        },
                    )],
                    append_requests: Mutex::new(Vec::new()),
                    replay_records: Mutex::new(BTreeMap::new()),
                    lookup_count: Mutex::new(0),
                }),
            }
        }

        fn append_count(&self) -> usize {
            self.inner.append_count()
        }

        fn idempotency_keys(&self) -> Vec<String> {
            self.inner.idempotency_keys()
        }

        fn replay_global_positions(&self) -> Vec<i64> {
            self.inner.replay_global_positions()
        }
    }

    impl ReplayAwareOrderStore {
        fn new(source_event: &OrderEvent) -> Self {
            Self {
                inner: Arc::new(ReplayAwareStoreInner {
                    global_position: 21,
                    event_id: Uuid::from_u128(21),
                    rehydration_events: vec![stored_event(
                        "order-order-1",
                        "OrderPlaced",
                        source_event.clone(),
                    )],
                    append_requests: Mutex::new(Vec::new()),
                    replay_records: Mutex::new(BTreeMap::new()),
                    lookup_count: Mutex::new(0),
                }),
            }
        }

        fn append_count(&self) -> usize {
            self.inner.append_count()
        }

        fn idempotency_keys(&self) -> Vec<String> {
            self.inner.idempotency_keys()
        }

        fn replay_global_positions(&self) -> Vec<i64> {
            self.inner.replay_global_positions()
        }
    }

    impl ReplayAwareStoreInner {
        fn append_count(&self) -> usize {
            self.append_requests.lock().expect("append requests").len()
        }

        fn idempotency_keys(&self) -> Vec<String> {
            self.append_requests
                .lock()
                .expect("append requests")
                .iter()
                .map(|request| request.idempotency_key.clone())
                .collect()
        }

        fn replay_global_positions(&self) -> Vec<i64> {
            self.replay_records
                .lock()
                .expect("replay records")
                .values()
                .flat_map(|record| record.append.global_positions.iter().copied())
                .collect()
        }

        fn append(&self, request: AppendRequest) -> StoreResult<AppendOutcome> {
            let committed = es_store_postgres::CommittedAppend {
                stream_id: request.stream_id.clone(),
                first_revision: StreamRevision::new(1),
                last_revision: StreamRevision::new(1),
                global_positions: vec![self.global_position],
                event_ids: vec![self.event_id],
            };
            if let Some(reply) = request.command_reply_payload.clone() {
                self.replay_records.lock().expect("replay records").insert(
                    request.idempotency_key.clone(),
                    CommandReplayRecord {
                        append: committed.clone(),
                        reply,
                    },
                );
            }
            self.append_requests
                .lock()
                .expect("append requests")
                .push(request);
            Ok(AppendOutcome::Committed(committed))
        }

        fn load_rehydration(&self) -> StoreResult<RehydrationBatch> {
            Ok(RehydrationBatch {
                snapshot: None,
                events: self.rehydration_events.clone(),
            })
        }

        fn lookup_command_replay(
            &self,
            idempotency_key: &str,
        ) -> StoreResult<Option<CommandReplayRecord>> {
            *self.lookup_count.lock().expect("lookup count") += 1;
            Ok(self
                .replay_records
                .lock()
                .expect("replay records")
                .get(idempotency_key)
                .cloned())
        }
    }

    impl RuntimeEventStore for ReplayAwareProductStore {
        fn append(&self, request: AppendRequest) -> BoxFuture<'_, StoreResult<AppendOutcome>> {
            let result = self.inner.append(request);
            Box::pin(async move { result })
        }

        fn load_rehydration(
            &self,
            _tenant_id: &TenantId,
            _stream_id: &StreamId,
        ) -> BoxFuture<'_, StoreResult<RehydrationBatch>> {
            let result = self.inner.load_rehydration();
            Box::pin(async move { result })
        }

        fn lookup_command_replay(
            &self,
            _tenant_id: &TenantId,
            idempotency_key: &str,
        ) -> BoxFuture<'_, StoreResult<Option<CommandReplayRecord>>> {
            let result = self.inner.lookup_command_replay(idempotency_key);
            Box::pin(async move { result })
        }
    }

    impl RuntimeEventStore for ReplayAwareOrderStore {
        fn append(&self, request: AppendRequest) -> BoxFuture<'_, StoreResult<AppendOutcome>> {
            let result = self.inner.append(request);
            Box::pin(async move { result })
        }

        fn load_rehydration(
            &self,
            _tenant_id: &TenantId,
            _stream_id: &StreamId,
        ) -> BoxFuture<'_, StoreResult<RehydrationBatch>> {
            let result = self.inner.load_rehydration();
            Box::pin(async move { result })
        }

        fn lookup_command_replay(
            &self,
            _tenant_id: &TenantId,
            idempotency_key: &str,
        ) -> BoxFuture<'_, StoreResult<Option<CommandReplayRecord>>> {
            let result = self.inner.lookup_command_replay(idempotency_key);
            Box::pin(async move { result })
        }
    }

    fn stored_event<E: serde::Serialize>(
        stream_id: &str,
        event_type: &str,
        event: E,
    ) -> StoredEvent {
        StoredEvent {
            global_position: 1,
            stream_id: StreamId::new(stream_id).expect("stream id"),
            stream_revision: StreamRevision::new(1),
            event_id: Uuid::from_u128(1),
            event_type: event_type.to_owned(),
            schema_version: 1,
            payload: serde_json::to_value(event).expect("event payload"),
            metadata: json!({ "source": "process-manager-replay-test" }),
            tenant_id: tenant(),
            command_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            causation_id: None,
            recorded_at: time::OffsetDateTime::from_unix_timestamp(1_700_000_000)
                .expect("timestamp"),
        }
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
    async fn multi_line_reserve_failure_releases_prior_reservations_before_rejecting()
    -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let first_product = product_id("product-1");
        let second_product = product_id("product-2");
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(first_product.clone()), line(second_product.clone())],
        });

        let process_event = event.clone();
        let task = tokio::spawn(async move { manager.process(&process_event).await });

        let first_reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: first_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            first_reserve.envelope.command
        );
        assert!(
            first_reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: first_product.clone(),
                    },
                    committed_append(43, Uuid::from_u128(43)),
                )))
                .is_ok()
        );

        let second_reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: second_product,
                quantity: Quantity::new(2).expect("quantity")
            },
            second_reserve.envelope.command
        );
        assert!(
            second_reserve
                .envelope
                .reply
                .send(Err(RuntimeError::Domain {
                    message: "insufficient inventory".to_owned(),
                }))
                .is_ok()
        );

        let release = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReleaseInventory {
                product_id: first_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            release.envelope.command
        );
        assert_eq!(
            format!(
                "pm:{}:{}:release:0:{}",
                process_manager_name().as_str(),
                event.event_id,
                first_product.as_str()
            ),
            release.envelope.idempotency_key
        );
        assert!(
            release
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReleased {
                        product_id: first_product,
                    },
                    committed_append(44, Uuid::from_u128(44)),
                )))
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
                command_count: 4
            },
            task.await.expect("process task")?
        );

        Ok(())
    }

    #[tokio::test]
    async fn duplicate_product_lines_emit_distinct_reserve_keys() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let same_product = product_id("product-1");
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(same_product.clone()), line(same_product.clone())],
        });

        let process_event = event.clone();
        let task = tokio::spawn(async move { manager.process(&process_event).await });

        let first_reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: same_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            first_reserve.envelope.command
        );
        assert_eq!(
            format!(
                "pm:{}:{}:reserve:0:{}",
                process_manager_name().as_str(),
                event.event_id,
                same_product.as_str()
            ),
            first_reserve.envelope.idempotency_key
        );
        assert!(
            first_reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: same_product.clone(),
                    },
                    committed_append(43, Uuid::from_u128(43)),
                )))
                .is_ok()
        );

        let second_reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: same_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            second_reserve.envelope.command
        );
        assert_eq!(
            format!(
                "pm:{}:{}:reserve:1:{}",
                process_manager_name().as_str(),
                event.event_id,
                same_product.as_str()
            ),
            second_reserve.envelope.idempotency_key
        );
        assert!(
            second_reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: same_product.clone(),
                    },
                    committed_append(44, Uuid::from_u128(44)),
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
                    committed_append(45, Uuid::from_u128(45)),
                )))
                .is_ok()
        );

        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: 42,
                command_count: 3
            },
            task.await.expect("process task")?
        );

        Ok(())
    }

    #[tokio::test]
    async fn duplicate_product_line_failure_releases_distinct_prior_lines() -> OutboxResult<()> {
        let (product_gateway, mut product_rx, order_gateway, mut order_rx) = gateways();
        let manager = CommerceOrderProcessManager::new(
            process_manager_name(),
            product_gateway,
            order_gateway,
        );
        let same_product = product_id("product-1");
        let event = process_event(OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(same_product.clone()), line(same_product.clone())],
        });

        let process_event = event.clone();
        let task = tokio::spawn(async move { manager.process(&process_event).await });

        let first_reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: same_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            first_reserve.envelope.command
        );
        assert!(
            first_reserve
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReserved {
                        product_id: same_product.clone(),
                    },
                    committed_append(43, Uuid::from_u128(43)),
                )))
                .is_ok()
        );

        let second_reserve = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReserveInventory {
                product_id: same_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            second_reserve.envelope.command
        );
        assert!(
            second_reserve
                .envelope
                .reply
                .send(Err(RuntimeError::Domain {
                    message: "insufficient inventory".to_owned(),
                }))
                .is_ok()
        );

        let release = receive_product(&mut product_rx).await;
        assert_eq!(
            ProductCommand::ReleaseInventory {
                product_id: same_product.clone(),
                quantity: Quantity::new(2).expect("quantity")
            },
            release.envelope.command
        );
        assert_eq!(
            format!(
                "pm:{}:{}:release:0:{}",
                process_manager_name().as_str(),
                event.event_id,
                same_product.as_str()
            ),
            release.envelope.idempotency_key
        );
        assert!(
            release
                .envelope
                .reply
                .send(Ok(CommandOutcome::new(
                    ProductReply::InventoryReleased {
                        product_id: same_product.clone(),
                    },
                    committed_append(44, Uuid::from_u128(44)),
                )))
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
                command_count: 4
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
                "pm:{}:{}:reserve:0:{}",
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
    async fn process_manager_replayed_followups_return_original_outcomes() -> OutboxResult<()> {
        let process_manager_name =
            ProcessManagerName::new("commerce-order-pm").expect("process manager name");
        let product = product_id("product-1");
        let source_order_event = OrderEvent::OrderPlaced {
            order_id: order_id(),
            user_id: UserId::new("user-1").expect("user id"),
            lines: vec![line(product.clone()), line(product.clone())],
        };
        let event = process_event(source_order_event.clone());
        let product_store = ReplayAwareProductStore::new(product.clone());
        let order_store = ReplayAwareOrderStore::new(&source_order_event);
        let mut product_engine: CommandEngine<Product, _, _> = CommandEngine::new(
            CommandEngineConfig::new(1, 4, 4).expect("product config"),
            product_store.clone(),
            TestProductCodec,
        )
        .expect("product engine");
        let mut order_engine: CommandEngine<Order, _, _> = CommandEngine::new(
            CommandEngineConfig::new(1, 4, 4).expect("order config"),
            order_store.clone(),
            TestOrderCodec,
        )
        .expect("order engine");
        let manager = Arc::new(CommerceOrderProcessManager::new(
            process_manager_name.clone(),
            product_engine.gateway(),
            order_engine.gateway(),
        ));
        let expected_reserve_key_0 = format!(
            "pm:{}:{}:reserve:0:{}",
            process_manager_name.as_str(),
            event.event_id,
            product.as_str()
        );
        let expected_reserve_key_1 = format!(
            "pm:{}:{}:reserve:1:{}",
            process_manager_name.as_str(),
            event.event_id,
            product.as_str()
        );
        let expected_confirm_key = format!(
            "pm:{}:{}:confirm:{}",
            process_manager_name.as_str(),
            event.event_id,
            order_id().as_str()
        );

        let first_event = event.clone();
        let first_manager = manager.clone();
        let first_task = tokio::spawn(async move { first_manager.process(&first_event).await });
        assert!(product_engine.process_one().await.expect("first reserve 0"));
        assert!(product_engine.process_one().await.expect("first reserve 1"));
        assert!(order_engine.process_one().await.expect("first confirm"));
        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: event.global_position,
                command_count: 3
            },
            first_task.await.expect("first process")?
        );
        assert_eq!(
            vec![
                expected_reserve_key_0.clone(),
                expected_reserve_key_1.clone()
            ],
            product_store.idempotency_keys()
        );
        assert_eq!(
            vec![expected_confirm_key.clone()],
            order_store.idempotency_keys()
        );

        let second_event = event.clone();
        let second_manager = manager.clone();
        let second_task = tokio::spawn(async move { second_manager.process(&second_event).await });
        assert!(
            product_engine
                .process_one()
                .await
                .expect("second reserve 0")
        );
        assert!(
            product_engine
                .process_one()
                .await
                .expect("second reserve 1")
        );
        assert!(order_engine.process_one().await.expect("second confirm"));
        assert_eq!(
            ProcessOutcome::CommandsSubmitted {
                global_position: event.global_position,
                command_count: 3
            },
            second_task.await.expect("second process")?
        );

        assert_eq!(2, product_store.append_count());
        assert_eq!(1, order_store.append_count());
        assert_eq!(
            vec![expected_reserve_key_0, expected_reserve_key_1],
            product_store.idempotency_keys()
        );
        assert_eq!(vec![expected_confirm_key], order_store.idempotency_keys());
        assert_eq!(product_store.replay_global_positions(), vec![20, 20]);
        assert_eq!(order_store.replay_global_positions(), vec![21]);

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

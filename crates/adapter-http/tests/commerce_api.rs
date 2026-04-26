//! HTTP adapter commerce contract tests.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use adapter_http::{HttpState, router};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use es_core::{CommandMetadata, StreamId, StreamRevision, TenantId};
use es_runtime::{
    AppendOutcome, AppendRequest, CommandEngine, CommandEngineConfig, CommandEnvelope,
    CommandGateway, CommandOutcome, CommandReplayRecord, CommandReplyPayload, CommittedAppend,
    NewEvent, PartitionRouter, RehydrationBatch, RuntimeError, RuntimeEventCodec,
    RuntimeEventStore, SnapshotRecord, StoreResult, StoredEvent,
};
use example_commerce::{
    Order, OrderCommand, OrderEvent, OrderId, OrderLine, OrderReply, OrderState, ProductId,
    Quantity, Sku, UserId,
};
use futures::future::BoxFuture;
use serde_json::json;
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn healthz_returns_ok() {
    let (order_gateway, _order_rx) =
        CommandGateway::<Order>::new(PartitionRouter::new(1).expect("router"), 1)
            .expect("order gateway");
    let (product_gateway, _product_rx) =
        CommandGateway::new(PartitionRouter::new(1).expect("router"), 1).expect("product gateway");
    let (user_gateway, _user_rx) =
        CommandGateway::new(PartitionRouter::new(1).expect("router"), 1).expect("user gateway");

    let app = router(HttpState {
        order_gateway,
        product_gateway,
        user_gateway,
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("health response");
    assert_eq!(StatusCode::OK, response.status());
    assert_eq!("ok", body_string(response.into_body()).await);
}

#[tokio::test]
async fn commerce_api_place_order_submits_command_and_returns_response_contract() {
    let (order_gateway, mut order_rx) =
        CommandGateway::<Order>::new(PartitionRouter::new(4).expect("router"), 8)
            .expect("order gateway");
    let (product_gateway, _product_rx) =
        CommandGateway::new(PartitionRouter::new(4).expect("router"), 8).expect("product gateway");
    let (user_gateway, _user_rx) =
        CommandGateway::new(PartitionRouter::new(4).expect("router"), 8).expect("user gateway");

    let app = router(HttpState {
        order_gateway,
        product_gateway,
        user_gateway,
    });

    let request = Request::builder()
        .method("POST")
        .uri("/commands/orders/place")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{
                "tenant_id": "tenant-a",
                "idempotency_key": "idem-place-1",
                "command_id": "018f3212-9299-7a4b-8bd3-3f3cc48c0f45",
                "correlation_id": "018f3212-9299-7a4b-8bd3-3f3cc48c0f46",
                "order_id": "order-1",
                "user_id": "user-1",
                "user_active": true,
                "lines": [
                    {
                        "product_id": "product-1",
                        "sku": "SKU-1",
                        "quantity": 2,
                        "product_available": true
                    }
                ]
            }"#,
        ))
        .expect("request");

    let response_task = tokio::spawn(async move {
        let routed = order_rx.recv().await.expect("routed command");
        assert_eq!("tenant-a", routed.envelope.metadata.tenant_id.as_str());
        assert_eq!("idem-place-1", routed.envelope.idempotency_key);
        assert_eq!("order-order-1", routed.envelope.stream_id.as_str());
        assert_eq!("order-order-1", routed.envelope.partition_key.as_str());
        assert!(matches!(
            routed.envelope.command,
            OrderCommand::PlaceOrder { user_active: true, ref lines, .. } if lines.len() == 1
        ));

        let sent = routed.envelope.reply.send(Ok(CommandOutcome::new(
            OrderReply::Placed {
                order_id: OrderId::new("order-1").expect("order id"),
            },
            committed_append("order-order-1", 10),
        )));
        assert!(sent.is_ok(), "send reply");
    });

    let response = app.oneshot(request).await.expect("response");
    assert_eq!(StatusCode::OK, response.status());
    let body = body_string(response.into_body()).await;

    assert!(body.contains(r#""correlation_id":"018f3212-9299-7a4b-8bd3-3f3cc48c0f46""#));
    assert!(body.contains(r#""stream_id":"order-order-1""#));
    assert!(body.contains(r#""first_revision":1"#));
    assert!(body.contains(r#""last_revision":1"#));
    assert!(body.contains(r#""global_positions":[10]"#));
    assert!(body.contains(r#""event_ids":["00000000-0000-0000-0000-00000000000a"]"#));
    assert!(body.contains(r#""reply":{"type":"placed","order_id":"order-1"}"#));
    response_task.await.expect("reply task");
}

#[tokio::test]
async fn commerce_api_response_contract_maps_overload_to_json_429() {
    let (order_gateway, _order_rx) =
        CommandGateway::<Order>::new(PartitionRouter::new(1).expect("router"), 1)
            .expect("order gateway");
    let fill_gateway = order_gateway.clone();
    let (product_gateway, _product_rx) =
        CommandGateway::new(PartitionRouter::new(1).expect("router"), 1).expect("product gateway");
    let (user_gateway, _user_rx) =
        CommandGateway::new(PartitionRouter::new(1).expect("router"), 1).expect("user gateway");

    let app = router(HttpState {
        order_gateway,
        product_gateway,
        user_gateway,
    });

    let (reply, _receiver) = tokio::sync::oneshot::channel();
    let envelope = CommandEnvelope::<Order>::new(
        place_order_command(),
        command_metadata(),
        "idem-already-queued",
        reply,
    )
    .expect("envelope");
    fill_gateway
        .try_submit(envelope)
        .expect("fill gateway queue");

    let overloaded = app
        .oneshot(place_order_request("idem-overload-2"))
        .await
        .expect("second response");
    assert_eq!(StatusCode::TOO_MANY_REQUESTS, overloaded.status());
    let body = body_string(overloaded.into_body()).await;
    assert!(body.contains(r#""code":"overloaded""#));
    assert!(body.contains(r#""message":"runtime is overloaded""#));
}

#[tokio::test]
async fn commerce_api_response_contract_maps_conflict_to_json_409() {
    let error = adapter_http::ApiError::from(RuntimeError::Conflict {
        stream_id: "order-order-1".to_owned(),
        expected: "no stream".to_owned(),
        actual: Some(3),
    });

    let response = axum::response::IntoResponse::into_response(error);
    assert_eq!(StatusCode::CONFLICT, response.status());
    let body = body_string(response.into_body()).await;
    assert!(body.contains(r#""code":"conflict""#));
    assert!(body.contains(r#""stream_id":"order-order-1""#));
    assert!(body.contains(r#""expected":"no stream""#));
    assert!(body.contains(r#""actual":3"#));
}

#[tokio::test]
async fn duplicate_place_order_retry_returns_original_response() {
    let order_store = ReplayAwareOrderStore::new(committed_append("order-order-1", 10));
    let mut order_engine: CommandEngine<Order, _, _> = CommandEngine::new(
        CommandEngineConfig::new(1, 4, 4).expect("config"),
        order_store.clone(),
        TestOrderCodec,
    )
    .expect("order engine");
    let (product_gateway, _product_rx) =
        CommandGateway::new(PartitionRouter::new(1).expect("router"), 4).expect("product gateway");
    let (user_gateway, _user_rx) =
        CommandGateway::new(PartitionRouter::new(1).expect("router"), 4).expect("user gateway");

    let app = router(HttpState {
        order_gateway: order_engine.gateway(),
        product_gateway,
        user_gateway,
    });

    let first_response_task = tokio::spawn(
        app.clone()
            .oneshot(place_order_request("idem-place-duplicate-1")),
    );
    assert!(order_engine.process_one().await.expect("processed first"));
    let first_response = first_response_task
        .await
        .expect("first response task")
        .expect("first response");
    assert_eq!(StatusCode::OK, first_response.status());
    let first_body = body_string(first_response.into_body()).await;

    let second_response_task =
        tokio::spawn(app.oneshot(place_order_request("idem-place-duplicate-1")));
    assert!(order_engine.process_one().await.expect("processed second"));
    let second_response = second_response_task
        .await
        .expect("second response task")
        .expect("second response");
    assert_eq!(StatusCode::OK, second_response.status());
    let second_body = body_string(second_response.into_body()).await;

    assert_eq!(1, order_store.append_count());
    assert!(order_store.lookup_count() >= 1);
    assert!(second_body.contains(r#""stream_id":"order-order-1""#));
    assert!(second_body.contains(r#""stream_revision":1"#));
    assert!(second_body.contains(r#""first_revision":1"#));
    assert!(second_body.contains(r#""last_revision":1"#));
    assert!(second_body.contains(r#""global_positions":[10]"#));
    assert!(second_body.contains(r#""event_ids":["00000000-0000-0000-0000-00000000000a"]"#));
    assert!(second_body.contains(r#""reply":{"type":"placed","order_id":"order-1"}"#));
    assert_eq!(first_body, second_body);
}

fn place_order_request(idempotency_key: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/commands/orders/place")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{
                "tenant_id": "tenant-a",
                "idempotency_key": "{idempotency_key}",
                "command_id": "018f3212-9299-7a4b-8bd3-3f3cc48c0f45",
                "correlation_id": "018f3212-9299-7a4b-8bd3-3f3cc48c0f46",
                "order_id": "order-1",
                "user_id": "user-1",
                "user_active": true,
                "lines": [
                    {{
                        "product_id": "product-1",
                        "sku": "SKU-1",
                        "quantity": 2,
                        "product_available": true
                    }}
                ]
            }}"#
        )))
        .expect("request")
}

fn place_order_command() -> OrderCommand {
    OrderCommand::PlaceOrder {
        order_id: OrderId::new("order-queued").expect("order id"),
        user_id: UserId::new("user-1").expect("user id"),
        user_active: true,
        lines: vec![OrderLine {
            product_id: ProductId::new("product-1").expect("product id"),
            sku: Sku::new("SKU-1").expect("sku"),
            quantity: Quantity::new(1).expect("quantity"),
            product_available: true,
        }],
    }
}

fn command_metadata() -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::now_v7(),
        correlation_id: Uuid::now_v7(),
        causation_id: None,
        tenant_id: TenantId::new("tenant-a").expect("tenant id"),
        requested_at: OffsetDateTime::now_utc(),
    }
}

fn committed_append(stream_id: &str, global_position: i64) -> CommittedAppend {
    CommittedAppend {
        stream_id: StreamId::new(stream_id).expect("stream id"),
        first_revision: StreamRevision::new(1),
        last_revision: StreamRevision::new(1),
        global_positions: vec![global_position],
        event_ids: vec![Uuid::from_u128(global_position as u128)],
    }
}

async fn body_string(body: Body) -> String {
    let bytes = to_bytes(body, usize::MAX).await.expect("body bytes");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
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
            Uuid::from_u128(10),
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

    fn decode_snapshot(&self, _snapshot: &SnapshotRecord) -> es_runtime::RuntimeResult<OrderState> {
        Ok(OrderState::default())
    }

    fn encode_reply(&self, reply: &OrderReply) -> es_runtime::RuntimeResult<CommandReplyPayload> {
        CommandReplyPayload::new(
            "order_reply",
            1,
            serde_json::to_value(reply).map_err(|error| RuntimeError::Codec {
                message: error.to_string(),
            })?,
        )
        .map_err(RuntimeError::from_store_error)
    }

    fn decode_reply(&self, payload: &CommandReplyPayload) -> es_runtime::RuntimeResult<OrderReply> {
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
struct ReplayAwareOrderStore {
    inner: Arc<ReplayAwareOrderStoreInner>,
}

struct ReplayAwareOrderStoreInner {
    committed: CommittedAppend,
    append_requests: Mutex<Vec<AppendRequest>>,
    replay_records: Mutex<VecDeque<CommandReplayRecord>>,
    lookup_count: Mutex<usize>,
}

impl ReplayAwareOrderStore {
    fn new(committed: CommittedAppend) -> Self {
        Self {
            inner: Arc::new(ReplayAwareOrderStoreInner {
                committed,
                append_requests: Mutex::new(Vec::new()),
                replay_records: Mutex::new(VecDeque::new()),
                lookup_count: Mutex::new(0),
            }),
        }
    }

    fn append_count(&self) -> usize {
        self.inner
            .append_requests
            .lock()
            .expect("append requests")
            .len()
    }

    fn lookup_count(&self) -> usize {
        *self.inner.lookup_count.lock().expect("lookup count")
    }
}

impl RuntimeEventStore for ReplayAwareOrderStore {
    fn append(&self, request: AppendRequest) -> BoxFuture<'_, StoreResult<AppendOutcome>> {
        self.inner
            .append_requests
            .lock()
            .expect("append requests")
            .push(request.clone());
        let committed = self.inner.committed.clone();
        let replay = request
            .command_reply_payload
            .clone()
            .map(|reply| CommandReplayRecord {
                append: committed.clone(),
                reply,
            });
        if let Some(replay) = replay {
            self.inner
                .replay_records
                .lock()
                .expect("replay records")
                .push_back(replay);
        }

        Box::pin(async move { Ok(AppendOutcome::Committed(committed)) })
    }

    fn load_rehydration(
        &self,
        _tenant_id: &TenantId,
        _stream_id: &StreamId,
    ) -> BoxFuture<'_, StoreResult<RehydrationBatch>> {
        Box::pin(async {
            Ok(RehydrationBatch {
                snapshot: None,
                events: Vec::new(),
            })
        })
    }

    fn lookup_command_replay(
        &self,
        _tenant_id: &TenantId,
        _idempotency_key: &str,
    ) -> BoxFuture<'_, StoreResult<Option<CommandReplayRecord>>> {
        *self.inner.lookup_count.lock().expect("lookup count") += 1;
        let replay = self
            .inner
            .replay_records
            .lock()
            .expect("replay records")
            .front()
            .cloned();

        Box::pin(async move { Ok(replay) })
    }
}

//! HTTP adapter commerce contract tests.

use adapter_http::{HttpState, router};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use es_core::{CommandMetadata, StreamId, StreamRevision, TenantId};
use es_runtime::{
    CommandEnvelope, CommandGateway, CommandOutcome, CommittedAppend, PartitionRouter, RuntimeError,
};
use example_commerce::{
    Order, OrderCommand, OrderId, OrderLine, OrderReply, ProductId, Quantity, Sku, UserId,
};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

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

fn place_order_request(idempotency_key: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/commands/orders/place")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(format!(
            r#"{{
                "tenant_id": "tenant-a",
                "idempotency_key": "{idempotency_key}",
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

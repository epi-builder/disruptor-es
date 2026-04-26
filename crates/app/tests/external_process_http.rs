//! Canonical external-process HTTP E2E coverage for `app serve`.

mod support;

use app::http_stress::canonical_place_order_request;
use reqwest::{Method, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;

use support::http_process::spawn_app;

#[tokio::test]
async fn external_process_http_success_path() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let request = canonical_place_order_request("external-success", 1);

    let response = app
        .request(Method::POST, "/commands/orders/place", Some(&request))
        .await?;
    assert_eq!(
        StatusCode::OK,
        response.status,
        "unexpected response: {}",
        response.body
    );

    let payload: Value = serde_json::from_str(&response.body)?;
    assert_eq!(
        request.correlation_id.to_string(),
        payload["correlation_id"]
    );
    assert_eq!("order-external-success-order-1", payload["stream_id"]);
    assert_eq!(1, payload["first_revision"]);
    assert_eq!(1, payload["last_revision"]);
    assert_eq!(
        1,
        payload["global_positions"]
            .as_array()
            .unwrap_or(&Vec::new())
            .len()
    );
    assert_eq!("placed", payload["reply"]["type"]);
    assert_eq!("external-success-order-1", payload["reply"]["order_id"]);

    Ok(())
}

#[tokio::test]
async fn external_process_http_metadata_contract() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let request = canonical_place_order_request("external-metadata", 2);

    let response = app
        .request(Method::POST, "/commands/orders/place", Some(&request))
        .await?;
    assert_eq!(
        StatusCode::OK,
        response.status,
        "unexpected response: {}",
        response.body
    );
    assert_eq!(
        Some("application/json"),
        response
            .headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value))
    );

    let payload: Value = serde_json::from_str(&response.body)?;
    assert_eq!(
        request.correlation_id.to_string(),
        payload["correlation_id"]
    );
    assert_eq!("order-external-metadata-order-2", payload["stream_id"]);
    assert_eq!(1, payload["stream_revision"]);
    assert_eq!(1, payload["first_revision"]);
    assert_eq!(1, payload["last_revision"]);
    assert_eq!(
        1,
        payload["global_positions"]
            .as_array()
            .unwrap_or(&Vec::new())
            .len()
    );
    assert_eq!(
        1,
        payload["event_ids"].as_array().unwrap_or(&Vec::new()).len()
    );
    assert_eq!("placed", payload["reply"]["type"]);
    assert_eq!("external-metadata-order-2", payload["reply"]["order_id"]);

    Ok(())
}

#[tokio::test]
async fn external_process_http_error_contracts() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let malformed = serde_json::json!({
        "tenant_id": "tenant-a",
        "idempotency_key": "external-error-idem",
        "order_id": "external-error-order",
        "user_id": "external-error-user",
        "user_active": true,
        "lines": [{
            "product_id": "external-error-product",
            "sku": "SKU-ERROR",
            "quantity": 0,
            "product_available": true
        }]
    });

    let response = app
        .request(Method::POST, "/commands/orders/place", Some(&malformed))
        .await?;
    assert_eq!(
        StatusCode::BAD_REQUEST,
        response.status,
        "unexpected response: {}",
        response.body
    );

    let payload: Value = serde_json::from_str(&response.body)?;
    assert_eq!("invalid_request", payload["error"]["code"]);
    assert!(
        payload["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("quantity")
    );

    Ok(())
}

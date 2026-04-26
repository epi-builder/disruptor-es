//! External-process smoke test for the official `app serve` entrypoint.

mod support;

use reqwest::{Method, StatusCode};

use support::http_process::spawn_app;

#[tokio::test]
async fn serve_smoke_boots_service_and_accepts_http_requests() -> anyhow::Result<()> {
    let app = spawn_app().await?;
    let health = app
        .request(Method::GET, "/healthz", Option::<&()>::None)
        .await?;
    assert_eq!(StatusCode::OK, health.status);
    assert_eq!("ok", health.body.trim());

    let request = app::http_stress::canonical_place_order_request("serve-smoke", 1);
    let response = app
        .request(Method::POST, "/commands/orders/place", Some(&request))
        .await?;
    assert_eq!(
        StatusCode::OK,
        response.status,
        "unexpected response: {}",
        response.body
    );
    assert!(
        response
            .body
            .contains("\"reply\":{\"type\":\"placed\",\"order_id\":\"serve-smoke-order-1\"}")
    );
    assert!(
        response
            .body
            .contains("\"stream_id\":\"order-serve-smoke-order-1\"")
    );
    Ok(())
}

//! HTTP request decoding boundary for commerce command ingress.

mod commerce;
mod error;

use axum::{Router, routing::get};

pub use commerce::{CommandSuccess, HttpState, PlaceOrderRequest, commerce_routes};
pub use error::{ApiError, ApiErrorBody};

/// Builds the HTTP command router.
pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .merge(commerce_routes(state))
}

async fn health() -> &'static str {
    "ok"
}

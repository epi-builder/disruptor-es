//! HTTP request decoding boundary for commerce command ingress.

mod commerce;
mod error;

pub use commerce::{CommandSuccess, HttpState, PlaceOrderRequest, commerce_routes};
pub use error::{ApiError, ApiErrorBody};

/// Builds the HTTP command router.
pub fn router(state: HttpState) -> axum::Router {
    commerce_routes(state)
}

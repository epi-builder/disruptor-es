//! Commerce fixture aggregates for the typed event-sourcing kernel.

mod ids;
mod order;
mod product;
mod user;

pub use ids::{OrderId, ProductId, Quantity, Sku, UserId};
pub use order::{
    Order, OrderCommand, OrderError, OrderEvent, OrderLine, OrderReply, OrderState, OrderStatus,
};
pub use product::{
    Product, ProductCommand, ProductError, ProductEvent, ProductReply, ProductState,
};
pub use user::{User, UserCommand, UserError, UserEvent, UserReply, UserState, UserStatus};

#[cfg(test)]
mod tests;

use crate::{ProductId, Quantity, Sku};

/// Product aggregate marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Product;

/// Product aggregate state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProductState {
    /// Product identity, if any.
    pub product_id: Option<ProductId>,
    /// Product stock-keeping unit, if any.
    pub sku: Option<Sku>,
    /// Available quantity tracked by later aggregate behavior.
    pub available_quantity: u32,
}

/// Commands accepted by the product aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductCommand {}

/// Events emitted by the product aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductEvent {}

/// Replies returned by product commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductReply {}

/// Product command validation errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductError {}

impl ProductState {
    /// Returns the available quantity as a typed value when positive.
    pub fn quantity(&self) -> Option<Quantity> {
        Quantity::new(self.available_quantity).ok()
    }
}

use serde::{Deserialize, Serialize};

/// Errors returned by commerce domain identity constructors.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CommerceIdError {
    /// A required string-backed value was empty.
    #[error("{type_name} cannot be empty")]
    EmptyValue { type_name: &'static str },
    /// Quantity values must be greater than zero.
    #[error("quantity must be greater than zero")]
    InvalidQuantity,
}

/// User aggregate identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct UserId(String);

impl UserId {
    /// Creates a user identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CommerceIdError> {
        string_value(value, "UserId").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Product aggregate identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ProductId(String);

impl ProductId {
    /// Creates a product identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CommerceIdError> {
        string_value(value, "ProductId").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Order aggregate identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct OrderId(String);

impl OrderId {
    /// Creates an order identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, CommerceIdError> {
        string_value(value, "OrderId").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Product stock-keeping unit.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Sku(String);

impl Sku {
    /// Creates a stock-keeping unit.
    pub fn new(value: impl Into<String>) -> Result<Self, CommerceIdError> {
        string_value(value, "Sku").map(Self)
    }

    /// Returns the borrowed string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the value and returns the owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Positive item quantity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Quantity(u32);

impl Quantity {
    /// Creates a positive quantity.
    pub fn new(value: u32) -> Result<Self, CommerceIdError> {
        if value == 0 {
            return Err(CommerceIdError::InvalidQuantity);
        }
        Ok(Self(value))
    }

    /// Returns the numeric quantity value.
    pub fn value(self) -> u32 {
        self.0
    }
}

fn string_value(
    value: impl Into<String>,
    type_name: &'static str,
) -> Result<String, CommerceIdError> {
    let value = value.into();
    if value.is_empty() {
        return Err(CommerceIdError::EmptyValue { type_name });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_ids_reject_empty_values_with_type_name() {
        assert_eq!(
            CommerceIdError::EmptyValue {
                type_name: "UserId",
            },
            UserId::new("").expect_err("empty user id")
        );
        assert_eq!(
            CommerceIdError::EmptyValue {
                type_name: "ProductId",
            },
            ProductId::new("").expect_err("empty product id")
        );
        assert_eq!(
            CommerceIdError::EmptyValue {
                type_name: "OrderId",
            },
            OrderId::new("").expect_err("empty order id")
        );
        assert_eq!(
            CommerceIdError::EmptyValue { type_name: "Sku" },
            Sku::new("").expect_err("empty sku")
        );
    }

    #[test]
    fn quantity_rejects_zero_and_preserves_valid_value() {
        assert_eq!(
            CommerceIdError::InvalidQuantity,
            Quantity::new(0).expect_err("zero quantity")
        );

        assert_eq!(1, Quantity::new(1).expect("quantity").value());
    }
}

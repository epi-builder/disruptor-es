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

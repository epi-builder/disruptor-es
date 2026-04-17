use super::{OrderId, ProductId, Quantity, Sku, UserId};

#[test]
fn commerce_value_objects_construct_from_valid_inputs() {
    let user_id = UserId::new("user-1").expect("user id");
    let product_id = ProductId::new("product-1").expect("product id");
    let order_id = OrderId::new("order-1").expect("order id");
    let sku = Sku::new("SKU-1").expect("sku");
    let quantity = Quantity::new(1).expect("quantity");

    assert_eq!("user-1", user_id.as_str());
    assert_eq!("product-1", product_id.as_str());
    assert_eq!("order-1", order_id.as_str());
    assert_eq!("SKU-1", sku.as_str());
    assert_eq!(1, quantity.value());
}

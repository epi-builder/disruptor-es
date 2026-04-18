//! Domain-only microbenchmarks.
//!
//! These scenarios run synchronous commerce aggregate `decide`/`apply` logic
//! in memory. They do not exercise adapters, runtime gateways, rings, or
//! PostgreSQL storage.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use es_core::{CommandMetadata, TenantId};
use es_kernel::Aggregate;
use example_commerce::{
    Order, OrderCommand, OrderId, OrderLine, OrderState, Product, ProductCommand, ProductId,
    ProductState, Quantity, Sku, UserId,
};
use time::OffsetDateTime;
use uuid::Uuid;

fn metadata(seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: None,
        tenant_id: TenantId::new("tenant-bench").expect("tenant id"),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn product_id() -> ProductId {
    ProductId::new("product-bench").expect("product id")
}

fn sku() -> Sku {
    Sku::new("SKU-BENCH").expect("sku")
}

fn quantity(value: u32) -> Quantity {
    Quantity::new(value).expect("quantity")
}

fn order_line() -> OrderLine {
    OrderLine {
        product_id: product_id(),
        sku: sku(),
        quantity: quantity(2),
        product_available: true,
    }
}

fn domain_only_product_decide_apply(criterion: &mut Criterion) {
    criterion.bench_function("domain_only_product_decide_apply", |bench| {
        bench.iter(|| {
            let mut state = ProductState::default();
            let create = ProductCommand::CreateProduct {
                product_id: product_id(),
                sku: sku(),
                name: "Keyboard".to_owned(),
                initial_quantity: quantity(100),
            };
            let decision = Product::decide(&state, create, &metadata(10)).expect("create product");
            for event in &decision.events {
                Product::apply(&mut state, event);
            }

            let reserve = ProductCommand::ReserveInventory {
                product_id: product_id(),
                quantity: quantity(3),
            };
            let decision =
                Product::decide(&state, reserve, &metadata(20)).expect("reserve inventory");
            for event in &decision.events {
                Product::apply(&mut state, event);
            }

            black_box(state);
        });
    });
}

fn domain_only_order_lifecycle_decide_apply(criterion: &mut Criterion) {
    criterion.bench_function("domain_only_order_lifecycle_decide_apply", |bench| {
        bench.iter(|| {
            let order_id = OrderId::new("order-bench").expect("order id");
            let mut state = OrderState::default();
            let place = OrderCommand::PlaceOrder {
                order_id: order_id.clone(),
                user_id: UserId::new("user-bench").expect("user id"),
                user_active: true,
                lines: vec![order_line()],
            };
            let decision = Order::decide(&state, place, &metadata(30)).expect("place order");
            for event in &decision.events {
                Order::apply(&mut state, event);
            }

            let confirm = OrderCommand::ConfirmOrder { order_id };
            let decision = Order::decide(&state, confirm, &metadata(40)).expect("confirm order");
            for event in &decision.events {
                Order::apply(&mut state, event);
            }

            black_box(state);
        });
    });
}

criterion_group!(
    domain_only,
    domain_only_product_decide_apply,
    domain_only_order_lifecycle_decide_apply
);
criterion_main!(domain_only);

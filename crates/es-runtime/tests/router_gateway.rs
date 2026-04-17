use es_core::{PartitionKey, TenantId};
use es_runtime::{PartitionRouter, RuntimeError};

#[test]
fn partition_router_rejects_zero_shards() {
    let error = PartitionRouter::new(0).expect_err("zero shards rejected");

    assert!(matches!(error, RuntimeError::InvalidShardCount));
}

#[test]
fn same_tenant_and_key_route_to_same_shard() {
    let router = PartitionRouter::new(8).expect("router");
    let tenant = TenantId::new("tenant-a").expect("tenant");
    let partition_key = PartitionKey::new("order-123").expect("partition key");

    let first = router.route(&tenant, &partition_key);
    let second = router.route(&tenant, &partition_key);

    assert_eq!(first, second);
    assert_eq!(7, first.value());
}

#[test]
fn tenant_is_part_of_route_input() {
    let router = PartitionRouter::new(8).expect("router");
    let tenant_a = TenantId::new("tenant-a").expect("tenant a");
    let tenant_b = TenantId::new("tenant-b").expect("tenant b");
    let partition_key = PartitionKey::new("order-123").expect("partition key");

    let tenant_a_shard = router.route(&tenant_a, &partition_key);
    let tenant_b_shard = router.route(&tenant_b, &partition_key);

    assert_ne!(tenant_a_shard, tenant_b_shard);
    assert_eq!(7, tenant_a_shard.value());
    assert_eq!(4, tenant_b_shard.value());
}

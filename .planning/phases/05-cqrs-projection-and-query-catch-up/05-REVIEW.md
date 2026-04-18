---
phase: 05-cqrs-projection-and-query-catch-up
reviewed: 2026-04-18T00:48:43Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - crates/es-projection/Cargo.toml
  - crates/es-projection/src/checkpoint.rs
  - crates/es-projection/src/error.rs
  - crates/es-projection/src/lib.rs
  - crates/es-projection/src/projector.rs
  - crates/es-projection/src/query.rs
  - crates/es-projection/tests/minimum_position.rs
  - crates/es-store-postgres/Cargo.toml
  - crates/es-store-postgres/migrations/20260418000000_projection_read_models.sql
  - crates/es-store-postgres/src/lib.rs
  - crates/es-store-postgres/src/projection.rs
  - crates/es-store-postgres/tests/projections.rs
  - crates/example-commerce/Cargo.toml
  - crates/example-commerce/src/ids.rs
  - crates/example-commerce/src/order.rs
  - crates/example-commerce/src/product.rs
finding_counts:
  critical: 0
  warning: 2
  info: 1
  total: 3
findings:
  critical: 0
  warning: 2
  info: 1
  total: 3
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-04-18T00:48:43Z
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

Phase 05의 CQRS projection/query catch-up 변경분을 표준 깊이로 검토했습니다. 테넌트 조건은 read-model 조회, 갱신, offset 조회 전반에 포함되어 있고, malformed payload가 offset을 advance하지 않는 통합 테스트도 있습니다.

다만 projection offset의 동시 catch-up 정합성 문제와 commerce quantity 경계값 문제가 남아 있습니다. 두 항목 모두 데이터 손실보다는 재처리, 잘못된 재고 상태, projection/domain 불일치를 유발할 수 있는 correctness 리스크입니다.

## Warnings

### WR-01: concurrent catch-up can move projector offset backward

**File:** `crates/es-store-postgres/src/projection.rs:532`

**Issue:** `upsert_projector_offset`가 conflict update에서 `last_global_position = EXCLUDED.last_global_position`를 그대로 저장합니다. 같은 `(tenant_id, projector_name)`에 대해 두 catch-up 작업이 동시에 실행되고 서로 다른 batch limit 또는 stale offset으로 시작하면, 더 큰 position을 먼저 commit한 작업 뒤에 더 작은 batch가 나중에 commit하면서 durable offset을 낮출 수 있습니다. read-model row writes는 `last_applied_global_position` 조건으로 대체로 idempotent하지만, offset이 후퇴하면 같은 이벤트를 반복 스캔하고 query freshness 판단도 실제 진행 상황보다 낮게 보일 수 있습니다.

**Fix:**

```rust
sqlx::query(
    r#"
    INSERT INTO projector_offsets (
        tenant_id,
        projector_name,
        last_global_position
    )
    VALUES ($1, $2, $3)
    ON CONFLICT (tenant_id, projector_name) DO UPDATE
    SET last_global_position = GREATEST(
            projector_offsets.last_global_position,
            EXCLUDED.last_global_position
        ),
        updated_at = now()
    "#,
)
```

동시 catch-up을 지원하지 않는 설계라면 `catch_up` 진입점에서 per-projector advisory lock 또는 `SELECT ... FOR UPDATE` 기반 serialization을 명시적으로 걸어 offset과 read-model write ordering을 보장하는 편이 안전합니다.

### WR-02: Quantity accepts values that overflow product inventory state

**File:** `crates/example-commerce/src/product.rs:312`

**Issue:** `Quantity`는 `u32`를 보관하고 0만 거부하지만, `ProductState`와 read model은 inventory를 `i32`로 저장합니다. `Product::apply`는 `initial_quantity.value() as i32`, reserve/release에서도 `quantity.value() as i32`를 사용하므로 `Quantity::new(2_147_483_648)` 같은 값이 음수로 wrap됩니다. 반면 projection 쪽은 `i32::try_from(...).unwrap_or(i32::MAX)`로 clamp하기 때문에 동일한 이벤트를 replay한 domain state와 read model이 서로 다른 값을 가질 수 있습니다.

**Fix:** `Quantity::new`에서 `i32::MAX` 초과 값을 거부하거나 inventory state/read model 타입을 `i64`/`u64`로 일관되게 확장하십시오. 현재 `i32` schema를 유지한다면 생성자에서 경계를 닫는 방식이 가장 작습니다.

```rust
pub fn new(value: u32) -> Result<Self, CommerceIdError> {
    if value == 0 || value > i32::MAX as u32 {
        return Err(CommerceIdError::InvalidQuantity);
    }
    Ok(Self(value))
}
```

이후 `as i32` 변환은 `i32::try_from(quantity.value()).expect("Quantity fits i32")`처럼 불변식을 드러내는 형태로 바꾸는 것을 권장합니다.

## Info

### IN-01: regression tests do not cover the highest-risk boundaries

**File:** `crates/es-store-postgres/tests/projections.rs:330`

**Issue:** 현재 projection tests는 정상 catch-up, restart idempotence, tenant scoping, query minimum position, malformed payload failure를 커버합니다. 하지만 위 두 warning의 회귀 조건인 concurrent/stale offset update와 `Quantity`의 `i32::MAX` 초과 경계값은 테스트되지 않습니다.

**Fix:** projection store에는 더 높은 offset row가 이미 있는 상태에서 낮은 offset upsert가 후퇴시키지 않는 테스트를 추가하고, commerce domain에는 `Quantity::new(i32::MAX as u32 + 1)` 또는 equivalent boundary test를 추가하십시오. 동시성까지 검증하려면 서로 다른 limit의 두 catch-up 작업을 barrier로 맞춰 commit 순서를 뒤집는 통합 테스트가 가장 직접적입니다.

---

_Reviewed: 2026-04-18T00:48:43Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

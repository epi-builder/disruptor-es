use es_core::{StreamId, TenantId};
use sqlx::PgPool;

use crate::{RehydrationBatch, StoreResult, sql};

pub(crate) async fn load_rehydration(
    pool: &PgPool,
    tenant_id: &TenantId,
    stream_id: &StreamId,
) -> StoreResult<RehydrationBatch> {
    let snapshot = sql::load_latest_snapshot(pool, tenant_id, stream_id).await?;
    let after_revision = snapshot
        .as_ref()
        .map(|record| record.stream_revision.value())
        .unwrap_or(0);
    let after_revision = i64::try_from(after_revision)
        .map_err(|_| crate::StoreError::InvalidStoredRevision { value: i64::MAX })?;

    let events =
        sql::read_stream_after(pool, tenant_id, stream_id, after_revision, i64::MAX).await?;

    Ok(RehydrationBatch { snapshot, events })
}

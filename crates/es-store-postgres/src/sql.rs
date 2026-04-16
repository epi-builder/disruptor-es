use es_core::{ExpectedRevision, StreamId, StreamRevision, TenantId};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AppendOutcome, AppendRequest, CommittedAppend, NewEvent, SaveSnapshotRequest, SnapshotRecord,
    StoreError, StoreResult, StoredEvent,
};

pub(crate) async fn append(pool: &PgPool, request: AppendRequest) -> StoreResult<AppendOutcome> {
    let mut tx = pool.begin().await?;

    acquire_dedupe_lock(&mut tx, &request).await?;

    if let Some(committed) = select_dedupe_result(&mut tx, &request).await? {
        tx.commit().await?;
        return Ok(AppendOutcome::Duplicate(committed));
    }

    let current_revision = select_stream_revision_for_update(&mut tx, &request).await?;
    validate_expected_revision(&request, current_revision)?;

    let first_revision = current_revision.unwrap_or(0) + 1;
    let last_revision =
        first_revision + i64::try_from(request.events.len()).unwrap_or(i64::MAX) - 1;

    upsert_stream_revision(&mut tx, &request, last_revision).await?;

    let mut global_positions = Vec::with_capacity(request.events.len());
    let mut event_ids = Vec::with_capacity(request.events.len());

    for (index, event) in request.events.iter().enumerate() {
        let stream_revision = first_revision + i64::try_from(index).unwrap_or(i64::MAX);
        let inserted = insert_event(&mut tx, &request, event, stream_revision).await?;
        global_positions.push(inserted.global_position);
        event_ids.push(inserted.event_id);
    }

    let committed = committed_append(
        request.stream_id.clone(),
        first_revision,
        last_revision,
        global_positions,
        event_ids,
    )?;

    let dedupe_inserted = insert_dedupe_result(&mut tx, &request, &committed).await?;
    if !dedupe_inserted {
        tx.rollback().await?;
        return select_duplicate_after_late_conflict(pool, &request).await;
    }

    tx.commit().await?;

    Ok(AppendOutcome::Committed(committed))
}

async fn acquire_dedupe_lock(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
) -> StoreResult<()> {
    sqlx::query(
        r#"
        SELECT pg_advisory_xact_lock(
            hashtextextended($1 || ':' || $2, 0)
        )
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(&request.idempotency_key)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn select_dedupe_result(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
) -> StoreResult<Option<CommittedAppend>> {
    let response_payload = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT response_payload
        FROM command_dedup
        WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(&request.idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;

    response_payload
        .map(|payload| {
            serde_json::from_value(payload)
                .map_err(|source| StoreError::DedupeResultDecode { source })
        })
        .transpose()
}

async fn select_dedupe_result_from_pool(
    pool: &PgPool,
    request: &AppendRequest,
) -> StoreResult<Option<CommittedAppend>> {
    let response_payload = sqlx::query_scalar::<_, serde_json::Value>(
        r#"
        SELECT response_payload
        FROM command_dedup
        WHERE tenant_id = $1 AND idempotency_key = $2
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(&request.idempotency_key)
    .fetch_optional(pool)
    .await?;

    response_payload
        .map(|payload| {
            serde_json::from_value(payload)
                .map_err(|source| StoreError::DedupeResultDecode { source })
        })
        .transpose()
}

async fn select_stream_revision_for_update(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
) -> StoreResult<Option<i64>> {
    let revision = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT revision FROM streams
        WHERE tenant_id = $1 AND stream_id = $2
        FOR UPDATE
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(request.stream_id.as_str())
    .fetch_optional(&mut **tx)
    .await?;

    Ok(revision)
}

fn validate_expected_revision(
    request: &AppendRequest,
    current_revision: Option<i64>,
) -> StoreResult<()> {
    match request.expected_revision {
        ExpectedRevision::Any => Ok(()),
        ExpectedRevision::NoStream if current_revision.is_none() => Ok(()),
        ExpectedRevision::NoStream => Err(stream_conflict(
            request,
            "no stream".to_owned(),
            current_revision,
        )),
        ExpectedRevision::Exact(expected) => {
            let expected_value = i64::try_from(expected.value())
                .map_err(|_| StoreError::InvalidStoredRevision { value: i64::MAX })?;
            if current_revision == Some(expected_value) {
                Ok(())
            } else {
                Err(stream_conflict(
                    request,
                    expected.value().to_string(),
                    current_revision,
                ))
            }
        }
    }
}

fn stream_conflict(request: &AppendRequest, expected: String, actual: Option<i64>) -> StoreError {
    StoreError::StreamConflict {
        stream_id: request.stream_id.as_str().to_owned(),
        expected,
        actual: actual.and_then(|revision| u64::try_from(revision).ok()),
    }
}

async fn upsert_stream_revision(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
    last_revision: i64,
) -> StoreResult<()> {
    let result = sqlx::query(
        r#"
        INSERT INTO streams (tenant_id, stream_id, revision)
        VALUES ($1, $2, $3)
        ON CONFLICT (tenant_id, stream_id)
        DO UPDATE SET revision = EXCLUDED.revision, updated_at = now()
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(request.stream_id.as_str())
    .bind(last_revision)
    .execute(&mut **tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(stream_conflict(request, "stream update".to_owned(), None));
    }

    Ok(())
}

struct InsertedEvent {
    event_id: Uuid,
    global_position: i64,
}

async fn insert_event(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
    event: &NewEvent,
    stream_revision: i64,
) -> StoreResult<InsertedEvent> {
    let (event_id, global_position, _stream_revision, _recorded_at) =
        sqlx::query_as::<_, (Uuid, i64, i64, time::OffsetDateTime)>(
            r#"
            INSERT INTO events (
                event_id,
                tenant_id,
                stream_id,
                stream_revision,
                command_id,
                correlation_id,
                causation_id,
                event_type,
                schema_version,
                payload,
                metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING event_id, global_position, stream_revision, recorded_at
            "#,
        )
        .bind(event.event_id)
        .bind(request.command_metadata.tenant_id.as_str())
        .bind(request.stream_id.as_str())
        .bind(stream_revision)
        .bind(request.command_metadata.command_id)
        .bind(request.command_metadata.correlation_id)
        .bind(request.command_metadata.causation_id)
        .bind(&event.event_type)
        .bind(event.schema_version)
        .bind(&event.payload)
        .bind(&event.metadata)
        .fetch_one(&mut **tx)
        .await?;

    Ok(InsertedEvent {
        event_id,
        global_position,
    })
}

async fn insert_dedupe_result(
    tx: &mut Transaction<'_, Postgres>,
    request: &AppendRequest,
    committed: &CommittedAppend,
) -> StoreResult<bool> {
    let first_global_position = committed
        .global_positions
        .first()
        .copied()
        .ok_or(StoreError::InvalidGlobalPosition { value: 0 })?;
    let last_global_position = committed
        .global_positions
        .last()
        .copied()
        .ok_or(StoreError::InvalidGlobalPosition { value: 0 })?;
    let response_payload = serde_json::to_value(committed)
        .map_err(|source| StoreError::DedupeResultDecode { source })?;

    let inserted = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO command_dedup (
            tenant_id,
            idempotency_key,
            stream_id,
            first_revision,
            last_revision,
            first_global_position,
            last_global_position,
            event_ids,
            response_payload
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
        RETURNING 1::bigint
        "#,
    )
    .bind(request.command_metadata.tenant_id.as_str())
    .bind(&request.idempotency_key)
    .bind(request.stream_id.as_str())
    .bind(
        i64::try_from(committed.first_revision.value())
            .map_err(|_| StoreError::InvalidStoredRevision { value: i64::MAX })?,
    )
    .bind(
        i64::try_from(committed.last_revision.value())
            .map_err(|_| StoreError::InvalidStoredRevision { value: i64::MAX })?,
    )
    .bind(first_global_position)
    .bind(last_global_position)
    .bind(&committed.event_ids)
    .bind(response_payload)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(inserted.is_some())
}

async fn select_duplicate_after_late_conflict(
    pool: &PgPool,
    request: &AppendRequest,
) -> StoreResult<AppendOutcome> {
    let committed = select_dedupe_result_from_pool(pool, request)
        .await?
        .ok_or_else(|| StoreError::DedupeConflict {
            tenant_id: request.command_metadata.tenant_id.as_str().to_owned(),
            idempotency_key: request.idempotency_key.clone(),
        })?;

    Ok(AppendOutcome::Duplicate(committed))
}

fn committed_append(
    stream_id: StreamId,
    first_revision: i64,
    last_revision: i64,
    global_positions: Vec<i64>,
    event_ids: Vec<Uuid>,
) -> StoreResult<CommittedAppend> {
    Ok(CommittedAppend {
        stream_id,
        first_revision: revision_from_i64(first_revision)?,
        last_revision: revision_from_i64(last_revision)?,
        global_positions,
        event_ids,
    })
}

fn revision_from_i64(value: i64) -> StoreResult<StreamRevision> {
    let revision = u64::try_from(value).map_err(|_| StoreError::InvalidStoredRevision { value })?;
    if revision == 0 {
        return Err(StoreError::InvalidStoredRevision { value });
    }

    Ok(StreamRevision::new(revision))
}

pub(crate) async fn read_stream_after(
    pool: &PgPool,
    tenant_id: &TenantId,
    stream_id: &StreamId,
    after_revision: i64,
    limit: i64,
) -> StoreResult<Vec<StoredEvent>> {
    if after_revision < 0 {
        return Err(StoreError::InvalidStoredRevision {
            value: after_revision,
        });
    }
    validate_limit(limit)?;

    let rows = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT
            global_position,
            stream_id,
            stream_revision,
            event_id,
            event_type,
            schema_version,
            payload,
            metadata,
            tenant_id,
            command_id,
            correlation_id,
            causation_id,
            recorded_at
        FROM events
        WHERE tenant_id = $1 AND stream_id = $2 AND stream_revision > $3
        ORDER BY stream_revision ASC
        LIMIT $4
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(stream_id.as_str())
    .bind(after_revision)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(EventRow::try_into).collect()
}

pub(crate) async fn read_global(
    pool: &PgPool,
    tenant_id: &TenantId,
    after_global_position: i64,
    limit: i64,
) -> StoreResult<Vec<StoredEvent>> {
    if after_global_position < 0 {
        return Err(StoreError::InvalidGlobalPosition {
            value: after_global_position,
        });
    }
    validate_limit(limit)?;

    let rows = sqlx::query_as::<_, EventRow>(
        r#"
        SELECT
            global_position,
            stream_id,
            stream_revision,
            event_id,
            event_type,
            schema_version,
            payload,
            metadata,
            tenant_id,
            command_id,
            correlation_id,
            causation_id,
            recorded_at
        FROM events
        WHERE tenant_id = $1 AND global_position > $2
        ORDER BY global_position ASC
        LIMIT $3
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(after_global_position)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(EventRow::try_into).collect()
}

fn validate_limit(limit: i64) -> StoreResult<()> {
    if limit < 0 {
        return Err(StoreError::InvalidReadLimit { value: limit });
    }

    Ok(())
}

pub(crate) async fn save_snapshot(
    pool: &PgPool,
    request: SaveSnapshotRequest,
) -> StoreResult<SnapshotRecord> {
    let stream_revision = i64::try_from(request.stream_revision.value())
        .map_err(|_| StoreError::InvalidStoredRevision { value: i64::MAX })?;

    let row = sqlx::query_as::<_, SnapshotRow>(
        r#"
        INSERT INTO snapshots (
            tenant_id,
            stream_id,
            stream_revision,
            state_payload,
            metadata
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (tenant_id, stream_id, stream_revision)
        DO UPDATE SET
            state_payload = EXCLUDED.state_payload,
            metadata = EXCLUDED.metadata,
            recorded_at = now()
        RETURNING tenant_id, stream_id, stream_revision, state_payload, metadata, recorded_at
        "#,
    )
    .bind(request.tenant_id.as_str())
    .bind(request.stream_id.as_str())
    .bind(stream_revision)
    .bind(&request.state_payload)
    .bind(&request.metadata)
    .fetch_one(pool)
    .await?;

    row.try_into()
}

pub(crate) async fn load_latest_snapshot(
    pool: &PgPool,
    tenant_id: &TenantId,
    stream_id: &StreamId,
) -> StoreResult<Option<SnapshotRecord>> {
    let row = sqlx::query_as::<_, SnapshotRow>(
        r#"
        SELECT tenant_id, stream_id, stream_revision, state_payload, metadata, recorded_at
        FROM snapshots
        WHERE tenant_id = $1 AND stream_id = $2
        ORDER BY stream_revision DESC
        LIMIT 1
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(stream_id.as_str())
    .fetch_optional(pool)
    .await?;

    row.map(TryInto::try_into).transpose()
}

#[derive(sqlx::FromRow)]
struct EventRow {
    global_position: i64,
    stream_id: String,
    stream_revision: i64,
    event_id: Uuid,
    event_type: String,
    schema_version: i32,
    payload: serde_json::Value,
    metadata: serde_json::Value,
    tenant_id: String,
    command_id: Uuid,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
    recorded_at: time::OffsetDateTime,
}

impl TryFrom<EventRow> for StoredEvent {
    type Error = StoreError;

    fn try_from(row: EventRow) -> StoreResult<Self> {
        if row.global_position < 1 {
            return Err(StoreError::InvalidGlobalPosition {
                value: row.global_position,
            });
        }

        let stream_id = StreamId::new(row.stream_id.clone()).map_err(|_| {
            StoreError::InvalidStoredStreamId {
                value: row.stream_id,
            }
        })?;
        let tenant_id = TenantId::new(row.tenant_id.clone()).map_err(|_| {
            StoreError::InvalidStoredTenantId {
                value: row.tenant_id,
            }
        })?;

        Ok(Self {
            global_position: row.global_position,
            stream_id,
            stream_revision: revision_from_i64(row.stream_revision)?,
            event_id: row.event_id,
            event_type: row.event_type,
            schema_version: row.schema_version,
            payload: row.payload,
            metadata: row.metadata,
            tenant_id,
            command_id: row.command_id,
            correlation_id: row.correlation_id,
            causation_id: row.causation_id,
            recorded_at: row.recorded_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct SnapshotRow {
    tenant_id: String,
    stream_id: String,
    stream_revision: i64,
    state_payload: serde_json::Value,
    metadata: serde_json::Value,
    recorded_at: time::OffsetDateTime,
}

impl TryFrom<SnapshotRow> for SnapshotRecord {
    type Error = StoreError;

    fn try_from(row: SnapshotRow) -> StoreResult<Self> {
        let tenant_id = TenantId::new(row.tenant_id.clone()).map_err(|_| {
            StoreError::InvalidStoredTenantId {
                value: row.tenant_id,
            }
        })?;
        let stream_id = StreamId::new(row.stream_id.clone()).map_err(|_| {
            StoreError::InvalidStoredStreamId {
                value: row.stream_id,
            }
        })?;

        Ok(Self {
            tenant_id,
            stream_id,
            stream_revision: revision_from_i64(row.stream_revision)?,
            state_payload: row.state_payload,
            metadata: row.metadata,
            recorded_at: row.recorded_at,
        })
    }
}

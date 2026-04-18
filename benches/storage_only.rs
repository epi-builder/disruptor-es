//! Storage-only PostgreSQL microbenchmarks.
//!
//! These scenarios measure explicit event-store operations against the
//! developer-provided `DATABASE_URL`. They never fall back to in-memory storage
//! or report ring/runtime throughput.

#![allow(missing_docs)]

use std::{env, hint::black_box};

use criterion::{Criterion, criterion_group, criterion_main};
use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_store_postgres::{AppendOutcome, AppendRequest, NewEvent, PostgresEventStore};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;
use tokio::runtime::Runtime;
use uuid::Uuid;

fn database_url() -> Option<String> {
    match env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => {
            eprintln!("storage_only requires DATABASE_URL");
            None
        }
    }
}

async fn connect_storage_pool() -> anyhow::Result<PgPool> {
    let Some(database_url) = database_url() else {
        anyhow::bail!("storage_only requires DATABASE_URL");
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn tenant_id() -> TenantId {
    TenantId::new("tenant-storage-bench").expect("tenant id")
}

fn stream_id(value: impl Into<String>) -> StreamId {
    StreamId::new(value).expect("stream id")
}

fn command_metadata(seed: u128) -> CommandMetadata {
    CommandMetadata {
        command_id: Uuid::from_u128(seed),
        correlation_id: Uuid::from_u128(seed + 1),
        causation_id: Some(Uuid::from_u128(seed + 2)),
        tenant_id: tenant_id(),
        requested_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
    }
}

fn new_event(seed: u128, event_type: &str) -> NewEvent {
    NewEvent::new(
        Uuid::from_u128(seed),
        event_type,
        1,
        json!({ "seed": seed }),
        json!({ "source": "storage_only" }),
    )
    .expect("event")
}

fn append_request(
    stream: StreamId,
    expected_revision: ExpectedRevision,
    idempotency_key: String,
    command_seed: u128,
    event_seed: u128,
    event_type: &str,
) -> AppendRequest {
    AppendRequest::new(
        stream,
        expected_revision,
        command_metadata(command_seed),
        idempotency_key,
        vec![new_event(event_seed, event_type)],
    )
    .expect("append request")
}

fn storage_only_append(criterion: &mut Criterion) {
    let Some(_) = database_url() else {
        return;
    };
    let runtime = Runtime::new().expect("tokio runtime");
    let store = runtime
        .block_on(async { connect_storage_pool().await.map(PostgresEventStore::new) })
        .expect("storage bench pool");
    let mut seed = 1_000_u128;

    criterion.bench_function("storage_only_append", |bench| {
        bench.iter(|| {
            seed = seed.wrapping_add(1);
            let outcome = runtime
                .block_on(store.append(append_request(
                    stream_id(format!("storage-append-{seed}")),
                    ExpectedRevision::NoStream,
                    format!("storage-append-{seed}"),
                    seed,
                    seed + 10_000,
                    "StorageOnlyAppended",
                )))
                .expect("append");
            black_box(outcome);
        });
    });
}

fn storage_only_occ_conflict(criterion: &mut Criterion) {
    let Some(_) = database_url() else {
        return;
    };
    let runtime = Runtime::new().expect("tokio runtime");
    let store = runtime
        .block_on(async { connect_storage_pool().await.map(PostgresEventStore::new) })
        .expect("storage bench pool");
    let stream = stream_id("storage-occ-conflict");
    runtime
        .block_on(store.append(append_request(
            stream.clone(),
            ExpectedRevision::NoStream,
            "storage-occ-seed".to_owned(),
            2_000,
            12_000,
            "StorageOnlySeeded",
        )))
        .expect("seed stream");
    let mut seed = 2_100_u128;

    criterion.bench_function("storage_only_occ_conflict", |bench| {
        bench.iter(|| {
            seed = seed.wrapping_add(1);
            let error = runtime
                .block_on(store.append(append_request(
                    stream.clone(),
                    ExpectedRevision::Exact(StreamRevision::new(99)),
                    format!("storage-occ-{seed}"),
                    seed,
                    seed + 10_000,
                    "StorageOnlyConflict",
                )))
                .expect_err("expected OCC conflict");
            black_box(error);
        });
    });
}

fn storage_only_dedupe(criterion: &mut Criterion) {
    let Some(_) = database_url() else {
        return;
    };
    let runtime = Runtime::new().expect("tokio runtime");
    let store = runtime
        .block_on(async { connect_storage_pool().await.map(PostgresEventStore::new) })
        .expect("storage bench pool");
    let stream = stream_id("storage-dedupe");
    runtime
        .block_on(store.append(append_request(
            stream.clone(),
            ExpectedRevision::NoStream,
            "storage-dedupe-key".to_owned(),
            3_000,
            13_000,
            "StorageOnlyDeduped",
        )))
        .expect("seed dedupe");

    criterion.bench_function("storage_only_dedupe", |bench| {
        bench.iter(|| {
            let outcome = runtime
                .block_on(store.append(append_request(
                    stream.clone(),
                    ExpectedRevision::NoStream,
                    "storage-dedupe-key".to_owned(),
                    3_100,
                    13_100,
                    "StorageOnlyDedupedAgain",
                )))
                .expect("duplicate append");
            assert!(matches!(outcome, AppendOutcome::Duplicate(_)));
            black_box(outcome);
        });
    });
}

fn storage_only_global_read(criterion: &mut Criterion) {
    let Some(_) = database_url() else {
        return;
    };
    let runtime = Runtime::new().expect("tokio runtime");
    let store = runtime
        .block_on(async { connect_storage_pool().await.map(PostgresEventStore::new) })
        .expect("storage bench pool");
    runtime
        .block_on(async {
            for seed in 4_000_u128..4_020 {
                store
                    .append(append_request(
                        stream_id(format!("storage-read-{seed}")),
                        ExpectedRevision::NoStream,
                        format!("storage-read-{seed}"),
                        seed,
                        seed + 10_000,
                        "StorageOnlyReadable",
                    ))
                    .await?;
            }
            anyhow::Ok(())
        })
        .expect("seed global reads");

    criterion.bench_function("storage_only_global_read", |bench| {
        bench.iter(|| {
            let events = runtime
                .block_on(store.read_global(&tenant_id(), 0, 20))
                .expect("read global");
            black_box(events);
        });
    });
}

criterion_group!(
    storage_only,
    storage_only_append,
    storage_only_occ_conflict,
    storage_only_dedupe,
    storage_only_global_read
);
criterion_main!(storage_only);

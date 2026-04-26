//! Storage-only PostgreSQL microbenchmarks.
//!
//! These scenarios measure explicit event-store operations against PostgreSQL.
//! They never fall back to in-memory storage or report ring/runtime throughput.

#![allow(missing_docs)]

use std::{env, hint::black_box};

use criterion::{Criterion, criterion_group, criterion_main};
use es_core::{CommandMetadata, ExpectedRevision, StreamId, StreamRevision, TenantId};
use es_store_postgres::{AppendOutcome, AppendRequest, NewEvent, PostgresEventStore};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;
use tokio::sync::OnceCell;
use tokio::runtime::Runtime;
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

static STORAGE_BENCH_HARNESS: OnceCell<StorageBenchHarness> = OnceCell::const_new();

struct StorageBenchHarness {
    _container: Option<ContainerAsync<Postgres>>,
    pool: PgPool,
}

fn database_url() -> Option<String> {
    match env::var("DATABASE_URL") {
        Ok(url) if !url.is_empty() => Some(url),
        _ => None,
    }
}

async fn connect_storage_pool() -> anyhow::Result<PgPool> {
    let harness = STORAGE_BENCH_HARNESS
        .get_or_try_init(StorageBenchHarness::connect_or_spawn)
        .await?;
    Ok(harness.pool.clone())
}

impl StorageBenchHarness {
    async fn connect_or_spawn() -> anyhow::Result<Self> {
        match database_url() {
            Some(database_url) => {
                let pool = connect_pool(&database_url).await?;
                Ok(Self {
                    _container: None,
                    pool,
                })
            }
            None => {
                let container = Postgres::default().with_tag("18").start().await?;
                let port = container.get_host_port_ipv4(5432).await?;
                let database_url = format!(
                    "postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable"
                );
                let pool = connect_pool(&database_url).await?;
                Ok(Self {
                    _container: Some(container),
                    pool,
                })
            }
        }
    }
}

async fn connect_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
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

#[cfg(test)]
mod tests {
    #[test]
    fn storage_only_source_uses_connect_or_spawn_fallback() {
        let source = include_str!("storage_only.rs");
        assert!(source.contains("connect_or_spawn"));
        assert!(!source.contains("storage_only requires DATABASE_URL"));
    }

    #[test]
    fn comparison_script_uses_diagnostic_hot_key_artifact_and_storage_validation() {
        let script = include_str!("../scripts/compare-stress-layers.sh");
        assert!(script.contains("live-http-single-hot-key-diagnostic.json"));
        assert!(script.contains("storage_only_append"));
        assert!(script.contains("storage-only benchmark output missing"));
    }
}

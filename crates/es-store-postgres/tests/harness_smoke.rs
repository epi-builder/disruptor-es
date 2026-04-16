//! Smoke coverage for the PostgreSQL integration-test harness.

mod common;

#[tokio::test]
async fn postgres_harness_starts_and_applies_migrations() -> anyhow::Result<()> {
    let harness = common::start_postgres().await?;

    for table in [
        "public.events",
        "public.streams",
        "public.command_dedup",
        "public.snapshots",
    ] {
        let exists = sqlx::query_scalar::<_, Option<String>>("SELECT to_regclass($1)::text")
            .bind(table)
            .fetch_one(&harness.pool)
            .await?;

        assert!(exists.is_some(), "{table} should exist after migrations");
    }

    Ok(())
}

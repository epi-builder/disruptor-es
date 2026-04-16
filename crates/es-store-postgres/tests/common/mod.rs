use sqlx::{postgres::PgPoolOptions, PgPool};
use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;

pub struct PostgresHarness {
    _container: ContainerAsync<Postgres>,
    pub pool: PgPool,
}

pub async fn start_postgres() -> anyhow::Result<PostgresHarness> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");

    let pool = PgPoolOptions::new().max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(PostgresHarness {
        _container: container,
        pool,
    })
}

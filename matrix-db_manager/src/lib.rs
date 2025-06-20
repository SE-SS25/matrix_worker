use anyhow::{Context, Result};
use matrix_macros::get_env;
use sqlx::postgres::PgPoolOptions;
use sqlx::postgres::types::PgInterval;
use sqlx::types::chrono;
use sqlx::{Postgres, migrate, query};
use std::time::Duration;
use tracing::{debug, info, instrument};
use uuid::Uuid;

pub type DbType = Postgres;
pub type DbPool = sqlx::Pool<DbType>;

const DB_POOL_MAX_SIZE: u32 = 100;
const DB_POOL_MIN_IDLE: u32 = 5;
const DB_POOL_TIMEOUT: Duration = Duration::from_secs(10);

#[instrument(name = "db init")]
pub async fn init() -> Result<DbPool> {
    let db_url = get_env!("DATABASE_URL");

    debug!("Connecting to database"); // URL not shown because of credentials

    let pool = PgPoolOptions::new()
        .max_connections(DB_POOL_MAX_SIZE)
        .min_connections(DB_POOL_MIN_IDLE)
        .acquire_timeout(DB_POOL_TIMEOUT)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    info!("Connected to database");

    Ok(pool)
}

#[instrument(skip_all)]
pub async fn worker_metric_example(db_pool: &DbPool) -> Result<()> {
    let now = chrono::Utc::now();
    let id = Uuid::new_v4();
    let uptime = PgInterval {
        months: 0,
        days: 0,
        microseconds: 300,
    };
    query!(
        r#"
        DELETE FROM worker_metric
            WHERE TRUE;
        "#
    )
    .execute(db_pool)
    .await
    .context("Can't delete")?;
    query!(
        r#"
        INSERT INTO worker_metric (id, last_heartbeat, uptime)
            VALUES ($1, $2, $3);
        "#,
        id,
        now,
        uptime,
    )
    .execute(db_pool)
    .await
    .context("Can't insert")?;

    let res = query!(
        r#"
        SELECT * FROM worker_metric
            LIMIT 1;
        "#
    )
    .fetch_one(db_pool)
    .await
    .context("Can't get worker_metrics")?;

    info!("{res:#?}");

    Ok(())
}

#[instrument(skip_all)]
pub async fn migrate(pool: &DbPool) -> Result<()> {
    info!("Migrating");
    migrate!("../migrations")
        .run(pool)
        .await
        .context("Failed to run migrations")?;
    Ok(())
}

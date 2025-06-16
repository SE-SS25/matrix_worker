use crate::DbPool;
use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use tracing::{debug, instrument};

const DB_POOL_MAX_SIZE: u32 = 100;
const DB_POOL_MIN_IDLE: u32 = 5;

#[instrument(name = "db init")]
pub(crate) async fn init() -> Result<DbPool> {
    let db_url = get_env!("DB_URL");

    debug!(db_url, "Connecting to database");

    let pool = PgPoolOptions::new()
        .max_connections(DB_POOL_MAX_SIZE)
        .min_connections(DB_POOL_MIN_IDLE)
        .connect(&db_url)
        .await
        .with_context(|| format!("Failed to connect to database: {db_url}"))?;

    Ok(pool)
}

#[instrument(skip_all)]
pub async fn migrate(pool: &DbPool) -> Result<()> {
    sqlx::migrate!()
        .run(pool)
        .await
        .context("Failed to run migrations")?;
    Ok(())
}

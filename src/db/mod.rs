use crate::DbPool;
use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::{debug, instrument};

const DB_POOL_MAX_SIZE: u32 = 100;
const DB_POOL_MIN_IDLE: u32 = 5;
const DB_POOL_TIMEOUT: Duration = Duration::from_secs(10);

#[instrument(name = "db init")]
pub(crate) async fn init() -> Result<DbPool> {
    let db_url = get_env!("DB_URL");

    debug!("Connecting to database"); // URL not shown because of credentials

    let pool = PgPoolOptions::new()
        .max_connections(DB_POOL_MAX_SIZE)
        .min_connections(DB_POOL_MIN_IDLE)
        .acquire_timeout(DB_POOL_TIMEOUT)
        .connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    Ok(pool)
}

#[cfg(debug_assertions)]
#[instrument(skip_all)]
pub async fn migrate(pool: &DbPool) -> Result<()> {
    sqlx::migrate!()
        .run(pool)
        .await
        .context("Failed to run migrations")?;
    Ok(())
}

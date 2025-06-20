use anyhow::{Context, Result};
use matrix_macros::get_env;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Postgres, migrate};
use std::time::Duration;
use tracing::{debug, info, instrument};

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
pub async fn migrate(pool: &DbPool) -> Result<()> {
    info!("Migrating");
    migrate!("../migrations")
        .run(pool)
        .await
        .context("Failed to run migrations")?;
    Ok(())
}

#[macro_use]
mod macros;
pub mod guard;
pub mod metrics_manager;
mod mongo_manager;

use anyhow::{Context, Result, anyhow, bail};
use matrix_errors::DbErr::Unreachable;
use matrix_macros::get_env;
use sqlx::postgres::PgPoolOptions;
use sqlx::postgres::types::PgInterval;
use sqlx::types::chrono;
use sqlx::{Postgres, migrate, query};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{debug, info, instrument};
use uuid::Uuid;

pub type DbType = Postgres;
pub type DbPool = sqlx::Pool<DbType>;

const DB_POOL_MAX_SIZE: u32 = 100;
const DB_POOL_MIN_IDLE: u32 = 5;
const DB_POOL_TIMEOUT: Duration = Duration::from_secs(5);

static LOADED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct DbManager {
    db_pool: DbPool,
}

impl DbManager {
    #[instrument(name = "db init")]
    pub async fn new() -> Result<Self> {
        if LOADED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            bail!("Can't create the DbManager more than once! (You can clone it tho)");
        }

        let db_url = get_env!("DATABASE_URL");

        debug!("Connecting to database"); // URL not shown because of credentials

        let db_pool = PgPoolOptions::new()
            .max_connections(DB_POOL_MAX_SIZE)
            .min_connections(DB_POOL_MIN_IDLE)
            .acquire_timeout(DB_POOL_TIMEOUT)
            .connect(&db_url)
            .await
            .context("Can't connect to database")
            .map_err(Unreachable)?;

        info!("Connected to database");

        let manager = DbManager { db_pool };

        Ok(manager)
    }

    #[instrument(skip_all)]
    pub async fn worker_metric_example(&self) -> Result<()> {
        let db_pool = backoff!(self);

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
        .context("Can't delete")
        .map_err(|e| hans!(self, e))?;

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
        .context("Can't insert")
        .map_err(|e| hans!(self, e))?;

        let res = query!(
            r#"
            SELECT * FROM worker_metric
                LIMIT 1;
            "#
        )
        .fetch_one(db_pool)
        .await
        .context("Can't get worker_metrics")
        .map_err(|e| hans!(self, e))?;

        info!("{res:#?}");

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn migrate(&self) -> Result<()> {
        let db_pool = backoff!(self);

        info!("Migrating");
        migrate!("../migrations")
            .run(db_pool)
            .await
            .context("Failed to run migrations")
            .map_err(|e| hans!(self, e))?;
        Ok(())
    }
}

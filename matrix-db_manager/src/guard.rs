use crate::DbPool;
use anyhow::Result;
use sqlx::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::sleep;
use tracing::{debug, info, instrument, warn};

static DB_GUARD_RUNNING: AtomicBool = AtomicBool::new(false);

pub struct DbGuard {
    db_pool: DbPool,
}

impl DbGuard {
    pub fn is_running(ord: Ordering) -> bool {
        DB_GUARD_RUNNING.load(ord)
    }

    #[instrument(skip_all)]
    pub(super) fn init(db_pool: &DbPool) {
        if DB_GUARD_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let guard = Self {
            db_pool: db_pool.clone(),
        };

        tokio::spawn(guard.run());
    }

    #[instrument(skip_all)]
    async fn run(self) {
        let mut backoff = matrix_commons::DEFAULT_BACKOFF;
        loop {
            warn!(
                "DB is down, backing off for {ms}ms",
                ms = backoff.as_millis()
            );
            sleep(backoff).await;
            if self.check_conn().await.is_ok() {
                info!("DB is alive again");
                return;
            };
            backoff = matrix_commons::jitter(backoff);
        }
    }

    #[instrument(skip_all)]
    async fn check_conn(&self) -> Result<()> {
        debug!("Checking");
        let mut conn = self.db_pool.acquire().await?;
        conn.ping().await?;
        Ok(())
    }
}

impl Drop for DbGuard {
    fn drop(&mut self) {
        DB_GUARD_RUNNING.store(false, Ordering::Relaxed)
    }
}

use crate::DbPool;
use anyhow::Result;
use core::ops::RangeInclusive;
use sqlx::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;

const JITTER_RANGE: RangeInclusive<f64> = 0.5..=1.5;
const DEFAULT_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(5 * 60); // Max 5 mins

pub(super) static GUARD_RUNNING: AtomicBool = AtomicBool::new(false);

pub(super) struct DbGuard {
    db_pool: DbPool,
}

impl DbGuard {
    pub(super) fn init(db_pool: &DbPool) {
        match GUARD_RUNNING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => {}
            Err(_) => return,
        }

        let guard = Self {
            db_pool: db_pool.clone(),
        };

        tokio::spawn(guard.run());
    }

    async fn run(self) {
        let mut backoff = DEFAULT_BACKOFF;
        loop {
            sleep(backoff).await;
            if self.check_conn().await.is_ok() {
                return;
            };
            let millis = backoff.as_millis().pow(2) as f64;
            let modifier = rand::random_range(JITTER_RANGE);
            let backoff_millis = (millis * modifier) as u64;
            backoff = Duration::from_millis(backoff_millis).min(MAX_BACKOFF);
        }
    }

    async fn check_conn(&self) -> Result<()> {
        let mut conn = self.db_pool.acquire().await?;
        conn.ping().await?;
        Ok(())
    }
}

impl Drop for DbGuard {
    fn drop(&mut self) {
        GUARD_RUNNING.store(false, Ordering::Relaxed)
    }
}

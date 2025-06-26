use crate::DbManager;
use anyhow::{Context, Result};
use matrix_metrics::MetricsWrapper;
use sqlx::postgres::types::PgInterval;
use sqlx::query;
use sqlx::types::chrono;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, instrument, warn};

const PERSIST_INTERVAL: Duration = Duration::from_secs(10);

impl DbManager {
    #[instrument(name = "manage metrics", skip_all)]
    pub async fn manage(&self, metrics: MetricsWrapper) {
        let startup = Instant::now();
        loop {
            debug!("Persisting metrics");
            if let Err(e) = self.persist(&metrics, startup).await {
                error!(?e, "Persisting metrics failed");
            }
            sleep(PERSIST_INTERVAL).await;
        }
    }

    #[instrument(skip_all)]
    async fn persist(&self, metrics: &MetricsWrapper, running_since: Instant) -> Result<()> {
        let db_pool = backoff!(self);

        let id = metrics.id();
        let last_heartbeat = chrono::Utc::now();
        let uptime = instant_to_interval(running_since);
        let read_per_sec = metrics.read_ps();
        let write_per_sec = metrics.write_ps();
        let req_per_sec = read_per_sec + write_per_sec;
        let req_total = metrics.get_total_requests() as i64;
        let req_failed = metrics.get_total_fails() as i64;
        let db_avail = (req_failed as f32) / (req_total as f32);

        debug!(%id, ?uptime, req_total, req_per_sec, req_failed);

        query!(
            r#"
        INSERT INTO worker_metric
            (
                id,
                last_heartbeat,
                uptime,
                req_per_sec,
                read_per_sec,
                write_per_sec,
                req_total,
                req_failed,
                db_err_rate
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                last_heartbeat = EXCLUDED.last_heartbeat,
                uptime = EXCLUDED.uptime,
                req_per_sec = EXCLUDED.req_per_sec,
                read_per_sec = EXCLUDED.read_per_sec,
                write_per_sec = EXCLUDED.write_per_sec,
                req_total = EXCLUDED.req_total,
                req_failed = EXCLUDED.req_failed,
                db_err_rate = EXCLUDED.db_err_rate;
        "#,
            id,
            last_heartbeat,
            uptime,
            req_per_sec,
            read_per_sec,
            write_per_sec,
            req_total,
            req_failed,
            db_avail,
        )
        .execute(db_pool)
        .await
        .context("Unable to persist metrics")
        .map_err(|e| db_fail!(self, e))?;

        Ok(())
    }
}

#[instrument(skip_all)]
fn instant_to_interval(ts: Instant) -> PgInterval {
    let total_micros = Instant::now().duration_since(ts).as_micros() as i64; // i64 is cringe

    const MICROS_PER_DAY: i64 = 24 * 60 * 60 * 1_000_000;
    const DAYS_PER_MONTH: i32 = 30; // Because I say so, problem?

    let total_days = (total_micros / MICROS_PER_DAY) as i32; // Anybody else with Prog1 flashbacks?
    let months = total_days / DAYS_PER_MONTH;
    let days = total_days % DAYS_PER_MONTH;
    let microseconds = total_micros % MICROS_PER_DAY;

    PgInterval {
        months,
        days,
        microseconds,
    }
}

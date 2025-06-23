use crate::DbPool;
use anyhow::{Context, Result};
use matrix_metrics::MetricsWrapper;
use sqlx::postgres::types::PgInterval;
use sqlx::query;
use sqlx::types::chrono;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, instrument};

const PERSIST_INTERVAL: Duration = Duration::from_secs(10);

#[instrument(skip_all)]
pub async fn manage(metrics: MetricsWrapper, db_pool: DbPool) {
    let startup = Instant::now();
    loop {
        debug!("Persisting metrics");
        if let Err(e) = persist(&metrics, startup, &db_pool).await {
            error!(?e, "Persisting metrics failed");
        }
        sleep(PERSIST_INTERVAL).await;
    }
}

#[instrument(skip_all)]
async fn persist(metrics: &MetricsWrapper, running_since: Instant, db_pool: &DbPool) -> Result<()> {
    let id = metrics.id();
    let last_heartbeat = chrono::Utc::now();
    let uptime = PgInterval {
        months: 0,
        days: 0,
        microseconds: (Instant::now() - running_since).as_micros() as i64,
    };
    let read_per_sec = metrics.read_ps() as i32;
    let write_per_sec = metrics.write_ps() as i32;
    let req_per_sec = read_per_sec + write_per_sec;
    let req_total = metrics.get_total_requests() as i64;
    let req_failed = metrics.get_total_fails() as i64;
    let db_avail = (req_failed as f32) / (req_total as f32);
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
                db_availability
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
                db_availability = EXCLUDED.db_availability;
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
    .context("Unable to persist metrics")?;
    Ok(())
}

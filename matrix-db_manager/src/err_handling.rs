use crate::DbManager;
use anyhow::{Context, Result, bail};
use sqlx::query;
use tokio::sync::mpsc::Receiver;
use tracing::instrument;
use tracing::warn;

impl DbManager {
    #[instrument(skip_all)]
    pub(crate) async fn monitor_errs(self, mut rx: Receiver<String>) {
        while let Some(url) = rx.recv().await {
            if let Err(e) = self.process_err(&url).await {
                warn!(?e, "Failed to process error");
            }
        }
    }

    #[instrument(skip_all)]
    async fn process_err(&self, url: &str) -> Result<()> {
        let db_pool = backoff!(self);

        let affected = query!(
            r#"
                INSERT INTO db_conn_err (worker_id, db_url, fail_time)
                    VALUES ($1, $2, NOW());
                "#,
            self.instance_id,
            url,
        )
        .execute(db_pool)
        .await
        .context("Failed to insert into db_conn_err")
        .map_err(|e| hans!(self, e))?
        .rows_affected();

        if affected != 1 {
            bail!("Failed to insert into db_conn_err, affected {affected} rows");
        }

        Ok(())
    }
}

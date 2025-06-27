use crate::MongoManager;
use anyhow::Result;
use bson::doc;
use mongodb::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::sleep;
use tracing::{debug, info, instrument, warn};

static MONGO_GUARD_RUNNING: AtomicBool = AtomicBool::new(false);

pub struct MongoGuard {
    client: Client,
}

impl MongoGuard {
    #[instrument]
    pub fn is_running(ord: Ordering) -> bool {
        MONGO_GUARD_RUNNING.load(ord)
    }

    #[instrument(skip_all)]
    pub(super) fn init(client: &Client) {
        if MONGO_GUARD_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let guard = Self {
            client: client.clone(),
        };

        tokio::spawn(guard.run());
    }

    #[instrument(skip_all)]
    async fn run(self) {
        let mut backoff = matrix_commons::DEFAULT_BACKOFF;
        loop {
            warn!(
                "Mongo is down, backing off for {ms}ms",
                ms = backoff.as_millis()
            );
            sleep(backoff).await;
            if self.check_conn().await.is_ok() {
                info!("Mongo is alive again");
                return;
            }
            backoff = matrix_commons::jitter(backoff);
        }
    }

    #[instrument(skip_all)]
    async fn check_conn(&self) -> Result<()> {
        debug!("Checking");
        self.client
            .database("admin")
            .run_command(doc! {"ping": 1})
            .await?;
        Ok(())
    }
}

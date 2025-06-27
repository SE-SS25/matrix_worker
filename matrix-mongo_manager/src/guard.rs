use anyhow::Result;
use bson::doc;
use mongodb::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, mpsc};
use tokio::time::sleep;
use tracing::{debug, info, instrument, warn};

#[derive(Debug)]
pub struct MongoGuard {
    client: Client,
    instance_down: Arc<AtomicBool>,
}

impl MongoGuard {
    #[instrument(skip_all)]
    pub(super) fn init(client: &Client, running: &Arc<AtomicBool>) -> Option<Sender<()>> {
        if running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }

        let guard = Self {
            client: client.clone(),
            instance_down: running.clone(),
        };

        let (tx, rx) = mpsc::channel();

        tokio::spawn(guard.run(rx));

        Some(tx)
    }

    #[instrument(skip_all)]
    async fn run(self, rx: Receiver<()>) {
        let mut backoff = matrix_commons::DEFAULT_BACKOFF;
        loop {
            if rx.try_recv().is_ok() {
                debug!("Manager is down, returning");
                return;
            }
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

impl Drop for MongoGuard {
    fn drop(&mut self) {
        self.instance_down.store(false, Ordering::Relaxed);
    }
}

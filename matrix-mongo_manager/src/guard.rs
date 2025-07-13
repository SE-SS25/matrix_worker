use crate::ClientWrapper;
use anyhow::Result;
use bson::doc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver as StdReceiver;
use std::thread;
use std::time::Duration;
use tokio::sync::oneshot::{self, Receiver};
use tokio::time::sleep;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

const SLEEP_DUR: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(super) struct MongoGuard {
    client: ClientWrapper,
    db_id: Uuid,
    db_has_problem: Arc<AtomicBool>,
    rx: Receiver<()>,
}

impl MongoGuard {
    #[instrument(skip_all)]
    pub(super) fn start(
        client: ClientWrapper,
        db_id: Uuid,
        db_has_problem: Arc<AtomicBool>,
        std_rx: StdReceiver<()>,
    ) {
        let (tx, rx) = oneshot::channel();

        thread::spawn(move || {
            while std_rx.try_recv().is_err() {
                thread::sleep(SLEEP_DUR);
            }
            let _ = tx.send(());
        });

        let guard = Self {
            client,
            db_id,
            db_has_problem,
            rx,
        };
        tokio::spawn(guard.run());
    }

    #[instrument(skip_all, fields(id = ?self.db_id))]
    async fn run(mut self) {
        loop {
            if self.db_has_problem.load(Ordering::SeqCst) {
                warn!("Detected faulty Mongo instance");
                // There might be a day when I understand this ordering, but today ain't it
                if self.handle_problem().await {
                    info!("Aborting error handling as instance is down");
                    return;
                }
                info!("Mongo is alive again");
                self.db_has_problem.store(false, Ordering::SeqCst);
            }
            sleep(SLEEP_DUR).await;
            if self.rx.try_recv().is_ok() {
                debug!("Killing thread as instance was shut down");
                return;
            }
        }
    }

    /// Returns true if instance is down, false otherwise
    #[instrument(skip_all)]
    async fn handle_problem(&mut self) -> bool {
        let mut backoff_millis = matrix_commons::DEFAULT_BACKOFF;
        let mut sleep_dur = Duration::from_millis(backoff_millis);
        loop {
            warn!("Mongo is down, backing off for {backoff_millis}ms");
            sleep(sleep_dur).await;
            if self.rx.try_recv().is_ok() {
                debug!("Manager is down, returning");
                return true;
            }
            if self.check_conn().await.is_ok() {
                return false;
            }
            (backoff_millis, sleep_dur) = matrix_commons::jitter(backoff_millis);
        }
    }

    #[instrument(skip_all)]
    async fn check_conn(&self) -> Result<()> {
        debug!("Checking");
        if let Some(client) = &*self.client {
            client
                .database("admin")
                .run_command(doc! { "ping": 1 })
                .await?;
        } else {
            warn!("Mongo seems to be down, but we don't have a client... That's weird");
        };
        Ok(())
    }
}

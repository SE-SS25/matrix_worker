#[macro_use]
mod macros;
pub mod guard;
pub mod mappings;
pub mod user;

use anyhow::Context;
use mongodb::Client;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use tracing::{debug, error, instrument};
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct MongoManager {
    id: Uuid,
    client: Option<Client>,
    guard_running: Arc<AtomicBool>,
    guard_tx: Arc<Mutex<Option<Sender<()>>>>,
}

impl MongoManager {
    #[instrument]
    pub async fn new(url: &str, id: Uuid) -> Self {
        debug!("Connecting to mongo");

        match Client::with_uri_str(&url).await {
            Ok(c) => Self {
                id,
                client: Some(c),
                guard_running: Default::default(),
                guard_tx: Default::default(),
            },
            Err(e) => {
                error!(?e, "Unable to create mongo client");
                // TODO Write to error db
                Self::default()
            }
        }
    }

    /// EXAMPLE FUNCTION TO CHECK MACROS, DO NOT CALL
    async fn x(&self) -> anyhow::Result<()> {
        let client = backoff!(self);
        let c = client.database("").collection("");
        c.insert_one(bson::doc! {})
            .await
            .context("")
            .map_err(|e| fritz!(self, e))?;
        Ok(())
    }
}

impl Drop for MongoManager {
    #[instrument]
    fn drop(&mut self) {
        let mut opt = self.guard_tx.lock();
        if let Some(ref mut tx) = *opt {
            let _ = tx.send(()); // Err = no receiver => Don't care (Also, this will happen if the db was down at least once, as we don't clean this (we would need to lock again))
        };
    }
}

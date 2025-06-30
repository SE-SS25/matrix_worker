#[macro_use]
mod macros;
pub mod guard;
pub mod mappings;
pub mod user;

use anyhow::{Context, Result};
use mongodb::Client;
use mongodb::options::ClientOptions;
use parking_lot::Mutex;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::time::Duration;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

const MONGO_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Default)]
pub struct MongoManager {
    id: Uuid,
    client: Option<Client>,
    guard_running: Arc<AtomicBool>,
    guard_tx: Arc<Mutex<Option<Sender<()>>>>,
}

#[derive(Debug, Deserialize)]
struct User {
    name: String,
}

#[instrument]
pub async fn test() -> Result<()> {
    let db = "foo";
    let col = "bar";
    let manager = mappings::write_manager(&db)
        .await
        .context("Unable to get write manager")?;

    manager
        .write(&db, &col, "Leon")
        .await
        .context("Can't write")?;

    let manager = mappings::read_manager(&db)
        .await
        .context("Can't get read manager")?;

    let Some(manager) = manager.get(0) else {
        error!("No manager");
        return Ok(());
    };

    manager.read(&db, &col).await.context("Can't read")?;

    Ok(())
}

impl MongoManager {
    #[instrument]
    pub async fn new(url: &str, id: Uuid) -> Self {
        debug!("Connecting to mongo");

        let mut opts = match ClientOptions::parse(url).await {
            Ok(opts) => opts,

            Err(e) => {
                error!(?e, "Unable to create mongo client options");
                // TODO Write to error db
                return Self::default();
            }
        };

        opts.connect_timeout = Some(MONGO_TIMEOUT);
        opts.min_pool_size = Some(5); // Why not?

        match Client::with_options(opts) {
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

    async fn write(&self, db: &str, col: &str, name: &str) -> Result<()> {
        return Ok(());
        let client = backoff!(self);
        let col = client.database(&db).collection(&col);
        col.insert_one(bson::doc! {"name": name})
            .await
            .context("Unable to insert")
            .map_err(|e| fritz!(self, e))?;
        info!("Written");

        Ok(())
    }

    async fn read(&self, db: &str, col: &str) -> Result<()> {
        let client = backoff!(self);
        let col = client.database(&db).collection::<User>(&col);
        let mut cursor = col
            .find(bson::doc! {})
            .await
            .context("Can't get docs")
            .map_err(|e| fritz!(self, e))?;

        info!("Got users");

        while let Ok(true) = cursor.advance().await {
            info!(user = ?cursor.deserialize_current());
        }

        Ok(())
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

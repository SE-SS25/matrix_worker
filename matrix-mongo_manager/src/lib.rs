#[macro_use]
mod macros;
pub mod guard;
mod hook;
pub mod mappings;
mod messaging;
pub mod user;

use crate::guard::MongoGuard;
use crate::hook::{MongoHook, MongoHookT};
use anyhow::{Context, Result};
use either::Either;
use mongodb::Client;
use mongodb::options::ClientOptions;
use serde::Deserialize;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

type ClientWrapper = Arc<Option<Client>>;
const MONGO_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub struct MongoManager {
    client: ClientWrapper,
    pub db_id: Uuid,
    db_has_problem: Arc<AtomicBool>,
    _hook: MongoHookT,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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

    let manager = match manager {
        Either::Left(m) => m,
        Either::Right((m, _)) => m,
    };

    manager.read(&db, &col).await.context("Can't read")?;

    Ok(())
}

impl MongoManager {
    #[instrument]
    pub async fn new(url: &str, id: Uuid) -> Self {
        debug!("Connecting to mongo");
        let (tx, rx) = mpsc::channel();
        let db_has_problem = Arc::new(AtomicBool::new(false));
        let mut manager = Self {
            client: Arc::new(None),
            db_id: id,
            db_has_problem: db_has_problem.clone(),
            _hook: Arc::new(MongoHook::new(tx)),
        };

        let mut opts = match ClientOptions::parse(url).await {
            Ok(opts) => opts,
            Err(e) => {
                error!(?e, "Unable to create mongo client options");
                // TODO Write to error db
                return manager;
            }
        };

        opts.connect_timeout = Some(MONGO_TIMEOUT);
        opts.min_pool_size = Some(5); // Why not?

        match Client::with_options(opts) {
            Ok(c) => {
                // Guaranteed to be Some, as we just created the Arc
                if let Some(client_ref) = Arc::get_mut(&mut manager.client) {
                    *client_ref = Some(c);
                } else {
                    error!("How tf can't we get a mut ref, we should be unique???");
                }
                MongoGuard::start(manager.client.clone(), id, db_has_problem.clone(), rx);
                manager
            }
            Err(e) => {
                error!(?e, "Unable to create mongo client");
                // TODO Write to error db
                manager
            }
        }
    }

    #[allow(unreachable_code, unused_variables)]
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
    #[allow(dead_code)]
    async fn x(&self) -> Result<()> {
        let client = backoff!(self);
        let c = client.database("").collection("");
        c.insert_one(bson::doc! {})
            .await
            .context("")
            .map_err(|e| fritz!(self, e))?;
        Ok(())
    }
}

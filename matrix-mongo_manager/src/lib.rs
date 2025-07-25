#[macro_use]
mod macros;
pub mod guard;
mod hook;
pub mod mappings;
pub mod messaging;
pub mod user;

use crate::guard::MongoGuard;
use crate::hook::{MongoHook, MongoHookT};
use mongodb::Client;
use mongodb::options::ClientOptions;
use serde::Deserialize;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tracing::{debug, error, instrument};
use uuid::Uuid;

type ClientWrapper = Arc<Option<Client>>;
const MONGO_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub struct MongoManager {
    client: ClientWrapper,
    pub db_id: Uuid,
    db_has_problem: Arc<AtomicBool>,
    url: String,
    tx: Sender<String>,
    _hook: MongoHookT,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct User {
    name: String,
}

impl MongoManager {
    #[instrument]
    pub async fn new(url: &str, id: Uuid, err_tx: Sender<String>) -> Self {
        debug!("Connecting to mongo");
        let (tx, rx) = mpsc::channel();
        let db_has_problem = Arc::new(AtomicBool::new(false));
        let mut manager = Self {
            client: Arc::new(None),
            db_id: id,
            db_has_problem: db_has_problem.clone(),
            url: url.to_string(),
            tx: err_tx,
            _hook: Arc::new(MongoHook::new(tx)),
        };

        let mut opts = match ClientOptions::parse(url).await {
            Ok(opts) => opts,
            Err(e) => {
                error!(?e, "Unable to create mongo client options");
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
                manager
            }
        }
    }
}

#[macro_use]
mod macros;
mod guard;
mod mappings;
pub mod user;

use anyhow::{Context, Result, bail};
use matrix_macros::get_env;
use mongodb::Client;
use mongodb::options::ClientOptions;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, instrument};

pub type MongoClient = Client;

static LOADED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct MongoManager {
    client: Client,
}

impl MongoManager {
    #[instrument]
    pub async fn new() -> Result<Self> {
        if LOADED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            bail!("Can't create MongoManager more than once! (You can clone it tho)");
        }

        let mongo_url = get_env!("MONGO_URL");

        debug!("Connecting to mongo");

        let options = ClientOptions::parse(&mongo_url)
            .await
            .context("Unable to parse mongo url")?;
        let client = MongoClient::with_options(options).context("Unable to create Mongo client")?;

        // Creating the client doesn't actually connect, so this is needed to establish a connection
        client
            .database("admin")
            .run_command(bson::doc! {"ping": 1})
            .await
            .context("Unable to ping mongo")?;

        info!("Connected to mongo");

        let manager = MongoManager { client };

        Ok(manager)
    }
}

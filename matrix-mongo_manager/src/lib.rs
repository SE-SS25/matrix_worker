use anyhow::{Context, Result};
use matrix_macros::get_env;
use mongodb::options::ClientOptions;
use tracing::{debug, info, instrument};

pub type MongoClient = mongodb::Client;

#[instrument]
pub async fn init() -> Result<MongoClient> {
    let mongo_url = get_env!("MONGO_URL");

    debug!("Connecting to mongo");

    let options = ClientOptions::parse(&mongo_url)
        .await
        .context("Unable to parse mongo url")?;
    let client = MongoClient::with_options(options).context("Unable to connect to mongo")?;

    // Creating the client doesn't actually connect, so this is needed to establish a connection
    client
        .database("admin")
        .run_command(bson::doc! {"ping":1})
        .await
        .context("Unable to ping mongo")?;

    info!("Connected to mongo");

    Ok(client)
}

use anyhow::{Context, Result};
use mongodb::Client;
use mongodb::options::ClientOptions;
use tracing::{debug, info, instrument};

#[instrument]
pub(crate) async fn init() -> Result<Client> {
    let mongo_url = get_env!("MONGO_URL");

    debug!("Connecting to mongo");

    let options = ClientOptions::parse(&mongo_url)
        .await
        .context("Unable to parse mongo url")?;
    let client = Client::with_options(options).context("Unable to connect to mongo")?;

    // Creating the client doesn't actually connect, so this is needed to establish a connection
    client
        .database("admin")
        .run_command(bson::doc! {"ping":1})
        .await
        .context("Unable to ping mongo")?;

    info!("Connected to mongo");

    Ok(client)
}

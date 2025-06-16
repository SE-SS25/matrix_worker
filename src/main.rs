#[macro_use]
mod macros;
mod db;
mod server;

use anyhow::{Context, Result};
use sqlx::Postgres;
use std::env;
use std::process::exit;
use tracing::{Level, info, subscriber};
use tracing_subscriber::FmtSubscriber;

type DbType = Postgres;
type DbPool = sqlx::Pool<DbType>;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    {
        const ENV_KEY: &str = "LOG_LEVEL";
        #[cfg(debug_assertions)]
        const DEFAULT_LEVEL: Level = Level::DEBUG;
        #[cfg(not(debug_assertions))]
        const DEFAULT_LEVEL: Level = Level::INFO;

        let lvl = match env::var(ENV_KEY) {
            Ok(lvl) => match lvl.parse() {
                Ok(lvl) => lvl,
                Err(e) => {
                    eprintln!("WARNING: {ENV_KEY} is set, but the value ({lvl}) is invalid: {e}");
                    exit(1);
                }
            },
            Err(_) => DEFAULT_LEVEL,
        };

        let fmt_sub = FmtSubscriber::builder().with_max_level(lvl).finish();

        subscriber::set_global_default(fmt_sub)
            .with_context(|| format!("Failed to set global default subscriber with lvl {lvl}"))?;
    }

    info!("Starting matrix server v{VERSION}");

    let db_pool = db::init().await.context("Failed to initialize database")?;

    #[cfg(debug_assertions)]
    db::migrate(&db_pool).await.context("Migration failed")?;

    server::start(db_pool).await.context("Failed to serve")?;

    Ok(())
}

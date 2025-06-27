use anyhow::{Context, Result};
use matrix_commons::VERSION;
use matrix_db_manager::DbManager;
use matrix_mongo_manager::MongoManager;
use std::env;
use std::process::exit;
use tracing::{Level, info, subscriber};
use tracing_subscriber::FmtSubscriber;

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

    info!("Starting matrix worker v{VERSION}");

    let (db_manager, mongo_manager) = tokio::try_join!(DbManager::new(), MongoManager::new(),)
        .context("Failed to initialize data stores")?;

    db_manager.migrate().await.context("Migration failed")?;

    let metrics = matrix_metrics::Metrics::new();
    {
        let db_manager = db_manager.clone();
        let metrics = metrics.clone();
        tokio::spawn(async move {
            db_manager.manage(metrics).await;
        });
    }

    matrix_server::start(db_manager, mongo_manager, metrics)
        .await
        .context("Failed to start and run HTTP server")?;

    Ok(())
}

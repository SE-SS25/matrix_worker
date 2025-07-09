use anyhow::{Context, Result};
use matrix_commons::VERSION;
use matrix_db_manager::DbManager;
use std::env;
use std::process::exit;
use std::time::Duration;
use tracing::{Level, error, info, subscriber};
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

    let db_manager = DbManager::new()
        .await
        .context("Failed to initialize DB Manager")?;

    db_manager.migrate().await.context("DB Migration failed")?;

    {
        let db_manager = db_manager.clone();
        tokio::spawn(async move {
            db_manager.manage_mongo().await;
        });
    }

    let metrics = matrix_metrics::Metrics::new();
    {
        let db_manager = db_manager.clone();
        let metrics = metrics.clone();
        tokio::spawn(async move {
            db_manager.manage_metrics(metrics).await;
        });
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    while let Err(e) = matrix_mongo_manager::test()
        .await
        .context("Test went wrong")
    {
        error!(%e, "Oh no");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    // matrix_server::start(db_manager, metrics)
    //     .await
    //     .context("Failed to start and run HTTP server")?;

    Ok(())
}

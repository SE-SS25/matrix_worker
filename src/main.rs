use anyhow::{Context, Result};
use matrix_commons::VERSION;
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

    let (db_pool, mongo_client) =
        tokio::try_join!(matrix_db_manager::init(), matrix_mongo_manager::init(),)
            .context("Failed to initialize data stores")?;

    matrix_db_manager::migrate(&db_pool)
        .await
        .context("Migration failed")?;

    let metrics = matrix_metrics::Metrics::new();
    {
        let db_pool = db_pool.clone();
        let metrics = metrics.clone();
        let manage_task = matrix_db_manager::metrics_manager::manage(metrics, db_pool);
        tokio::spawn(manage_task);
    }

    matrix_server::start(db_pool, mongo_client, metrics)
        .await
        .context("Failed to start and run HTTP server")?;

    Ok(())
}

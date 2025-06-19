mod user;

use anyhow::{Context, Result};
use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use matrix_commons::VERSION;
use matrix_db_manager::DbPool;
use matrix_macros::get_env;
use matrix_mongo_manager::MongoClient;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tokio::{select, signal};
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, instrument};

#[cfg(unix)]
const DOCKER_SHUTDOWN_SIG_NUM: i32 = 15;

const INTERNAL_ERR_MSG: &str = "Internal Server Error";

#[derive(Debug, Clone)]
struct AppState {
    db_pool: DbPool,
    client: MongoClient,
}

#[instrument(name = "start server", skip_all)]
pub async fn start(db_pool: DbPool, client: MongoClient) -> Result<()> {
    const ORIGIN_ENV_KEY: &str = "ALLOW_ORIGIN_URL";
    let allow_origin = get_env!(ORIGIN_ENV_KEY);
    debug! {%allow_origin};
    let allow_origin = allow_origin.parse::<HeaderValue>().with_context(|| {
        format!(
            "Unable to parse {ORIGIN_ENV_KEY} ({allow_origin}) \
                    to HeaderValue"
        )
    })?;
    let port = get_env!("PORT", "8080", u16);

    let cors = CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE]);

    let state = AppState { db_pool, client };

    info!(port, "Starting server");

    let app = Router::new()
        .route("/version", get(version))
        .route("/robots.txt", get(robots))
        .route("/user/create", post(user::add_user))
        .route("/user", get(user::get_all_users))
        .route("/user/{name}", get(user::get_user_by_name))
        .with_state(state)
        .layer(cors);

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| format!("Unable to bind to port {port}"))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server failed")?;

    Ok(())
}

#[instrument]
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!(%e, "Error while waiting for ctrl-c");
        };
    };

    #[cfg(unix)]
    let docker_shutdown = async {
        match signal(SignalKind::from_raw(DOCKER_SHUTDOWN_SIG_NUM)) {
            Ok(mut sig) => {
                if let None = sig.recv().await {
                    error!("Can't receive signals for sig {DOCKER_SHUTDOWN_SIG_NUM} anymore");
                }
            }
            Err(e) => {
                let err_msg = "Error while creating docker shutdown signal";
                error!(%e,"{err_msg}");
                panic!("{err_msg}");
            }
        }
    };

    #[cfg(not(unix))]
    let docker_shutdown = std::future::pending::<()>();

    select! {
        _ = ctrl_c => info!("Ctrl-C received"),
        _ = docker_shutdown => info!("Docker shutdown received"),
    }
}

#[instrument]
async fn version() -> impl IntoResponse {
    (StatusCode::OK, VERSION)
}

#[instrument]
async fn robots() -> impl IntoResponse {
    (StatusCode::OK, "User-agent: *\nDisallow: /")
}

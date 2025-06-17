use crate::server::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use bson::doc;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct User {
    name: String,
}

#[instrument(skip_all)]
pub(super) async fn add_user(
    State(state): State<AppState>,
    Json(user): Json<User>,
) -> impl IntoResponse {
    let users = state.client.database("test").collection("users");
    let insert_res = match users.insert_one(user.clone()).await {
        Ok(res) => res,
        Err(e) => {
            error!(?user, %e, "Failed to insert user");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error");
        }
    };
    info!(?insert_res, "Probably inserted user");
    (StatusCode::CREATED, "User created")
}

#[instrument(skip_all)]
pub(super) async fn get_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let users = state.client.database("test").collection("users");
    let user: Option<User> = match users
        .find_one(doc! {
            "name": &username,
        })
        .await
    {
        Ok(user_opt) => user_opt,
        Err(e) => {
            error!(?username, %e, "Failed to get user");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong".to_string(),
            );
        }
    };

    let Some(user) = user else {
        warn!("No user");
        return (StatusCode::NOT_FOUND, "No user found".to_string());
    };

    info!(?user, "Found user");

    (StatusCode::FOUND, format!("Found user: {user:?}"))
}

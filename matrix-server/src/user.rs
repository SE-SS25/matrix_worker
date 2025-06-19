use crate::{AppState, INTERNAL_ERR_MSG};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use bson::doc;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, trace, warn};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct User {
    name: String,
}

#[instrument(skip_all)]
pub(super) async fn add_user(
    State(state): State<AppState>,
    Json(user): Json<User>,
) -> impl IntoResponse {
    let to_insert = doc! {
        "name": &user.name,
    };
    let users = state.client.database("test").collection("users");
    let insert_res = match users.insert_one(to_insert).await {
        Ok(res) => res,
        Err(e) => {
            error!(?user, %e, "Failed to insert user");
            return (StatusCode::INTERNAL_SERVER_ERROR, INTERNAL_ERR_MSG);
        }
    };
    info!(inserted_id = ?insert_res.inserted_id, "Inserted user");
    (StatusCode::CREATED, "User created")
}

#[instrument(skip_all)]
pub(super) async fn get_user_by_name(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let users = state.client.database("test").collection::<User>("users");

    let user = match users
        .find_one(
            doc! { // NOTE: This seems to always get the first matching entry
                "name": &username,
            },
        )
        .await
    {
        Ok(user_opt) => user_opt,
        Err(e) => {
            error!(?username, %e, "Failed to get user");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                INTERNAL_ERR_MSG.to_string(),
            );
        }
    };

    let Some(user) = user else {
        warn!("No user");
        return (
            StatusCode::NOT_FOUND,
            format!("User {username:?} not found"),
        );
    };

    info!(?user, "Found user");

    (StatusCode::FOUND, format!("Found user: {user:?}"))
}

#[instrument(skip_all)]
pub(super) async fn get_all_users(State(state): State<AppState>) -> impl IntoResponse {
    let users = state.client.database("test").collection::<User>("users");

    let doc_count = match users.estimated_document_count().await {
        Ok(count) => count,
        Err(e) => {
            error!(%e, "Can't get estimated document count");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                INTERNAL_ERR_MSG.to_string(),
            );
        }
    };

    info!(user_count = ?doc_count, "Got estimated count of users");

    let mut user_cursor = match users.find(doc! {}).await {
        Ok(cursor) => cursor,
        Err(e) => {
            error!(%e, "Failed to get all users");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                INTERNAL_ERR_MSG.to_string(),
            );
        }
    };

    let mut users = vec![];

    loop {
        match user_cursor.advance().await {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => error!(%e, "User advancing failed"),
        }
        match user_cursor.deserialize_current() {
            Ok(user) => {
                trace!(?user, "Found user");
                users.push(user);
            }
            Err(e) => error!(%e, "Error deserializing"),
        }
    }

    (StatusCode::FOUND, format!("Users: {users:?}"))
}

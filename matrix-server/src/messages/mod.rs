use axum::Json;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use bson::DateTime;
use chrono::Utc;
use matrix_mongo_manager::messaging;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{Span, instrument, warn};

const ERR_KEY: &str = "err";

#[derive(Debug, Deserialize)]
pub(crate) struct RoomConfig {
    name: String,
    allowed_users: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SendMessage {
    user: String,
    room: String,
    msg: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReadMessage {
    messages: Vec<messaging::Message>,
    total_messages: usize,
    collections_read: u32,
}

#[instrument(skip_all, fields(room))]
pub(crate) async fn create_room(Json(config): Json<RoomConfig>) -> impl IntoResponse {
    Span::current().record("room", &config.name);

    match matrix_mongo_manager::MongoManager::add_room(
        &config.name,
        messaging::RoomConfig {
            allowed_users: config.allowed_users,
        },
    )
    .await
    {
        Ok(name) => (StatusCode::CREATED, format!("Created room {name:?}")),
        Err(e) => {
            warn!(?e, "Failed to add room");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    }
}

#[instrument(skip_all, fields(user, room, msg))]
pub(crate) async fn send(Json(payload): Json<SendMessage>) -> impl IntoResponse {
    Span::current().record("user", &payload.user);
    Span::current().record("room", &payload.room);
    // Span::current().record("msg", &payload.msg); // CONSIDER Remove, can/will be big

    if let Err(e) = matrix_mongo_manager::MongoManager::write_message(
        &payload.room,
        messaging::Message {
            author: payload.user,
            content: payload.msg,
            timestamp: DateTime::from_chrono(Utc::now()),
        },
    )
    .await
    {
        warn!(?e, "Failed to post message");
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    };

    (StatusCode::CREATED, "Successfully posted".to_string())
}

#[instrument]
pub(crate) async fn read(
    Path(room): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    const N_KEY: &str = "n";
    let Some(n) = params.get(N_KEY) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ERR_KEY: format!("{N_KEY:} is not set")})),
        );
    };
    let Ok(n) = n.parse::<usize>() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ERR_KEY: format!("{N_KEY}={n:?} is not a valid usize")})),
        );
    };

    match matrix_mongo_manager::MongoManager::read_messages(&room, n).await {
        Ok((messages, col_cnt)) => {
            let msg_len = messages.len();
            let resp = ReadMessage {
                messages,
                total_messages: msg_len,
                collections_read: col_cnt,
            };
            match serde_json::to_value(&resp) {
                Ok(val) => (StatusCode::OK, Json(val)),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ERR_KEY: e.to_string()})),
                ),
            }
        }
        Err(e) => {
            warn!(?e, "Failed to get messages");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ERR_KEY: e.to_string()})),
            )
        }
    }
}

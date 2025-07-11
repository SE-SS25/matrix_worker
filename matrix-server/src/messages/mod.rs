use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use bson::DateTime;
use chrono::Utc;
use matrix_mongo_manager::messaging;
use serde::Deserialize;
use tracing::{Span, error, instrument};

#[allow(unused)] // TODO Remove once password impl is solved
#[derive(Debug, Deserialize)]
pub(crate) struct SendMessage {
    user: String,
    room: String,
    msg: String,
}

#[instrument(skip_all, fields(user, room, msg))]
pub(crate) async fn send(
    State(state): State<AppState>,
    Json(payload): Json<SendMessage>,
) -> impl IntoResponse {
    Span::current().record("user", &payload.user);
    Span::current().record("room", &payload.room);
    // Span::current().record("msg", &payload.msg); // CONSIDER Remove, can/will be big

    if let Err(e) = matrix_mongo_manager::MongoManager::write_message(
        &payload.room,
        messaging::Message {
            user: payload.user,
            content: payload.msg,
            timestamp: DateTime::from_chrono(Utc::now()),
        },
    )
    .await
    {
        error!(?e, "Failed to post message");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Post failed");
    };

    (StatusCode::CREATED, "Successfully posted")
}

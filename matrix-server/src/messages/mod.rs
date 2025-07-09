use crate::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use tracing::{Span, instrument};

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

    // matrix_mongo_manager::mappings::;

    (StatusCode::CREATED, "Successfully posted")
}

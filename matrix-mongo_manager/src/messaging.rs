use super::mappings;
use crate::MongoManager;
use anyhow::{Context, Result, bail};
use bson::DateTime;
use matrix_errors::MatrixErr;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, warn};

const INVALID_ROOM_NAMES: &[&str] = &["admin", "config", "local"];
const CHAT_PREFIX: &str = "chat";
const MAX_MSGS_PER_COL: u64 = 3;

#[derive(Debug, Serialize, Deserialize)]
pub struct RoomConfig {
    pub allowed_users: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub author: String,
    pub content: String,
    pub timestamp: DateTime,
}

impl MongoManager {
    #[instrument(skip_all)]
    pub async fn add_room(room_name: &str, room_conf: RoomConfig) -> Result<String> {
        let room_name = room_name.to_lowercase();
        let manager = mappings::write_manager(&room_name)
            .await
            .with_context(|| format!("Can't get manager for room {room_name}"))?;

        manager
            .create_room(&room_name, &room_conf)
            .await
            .context("Failed to create room")
            .map_err(|e| fritz!(manager, e))??;

        Ok(room_name)
    }

    #[instrument(skip_all)]
    pub async fn write_message(room: &str, message: Message) -> Result<()> {
        let room = room.to_lowercase();
        let manager = mappings::write_manager(&room)
            .await
            .with_context(|| format!("Can't get manager for room {room}"))?;

        if !manager
            .check_access(&room, &message.author)
            .await
            .context("Unable to check access")
            .map_err(|e| fritz!(manager, e))?
        {
            bail!("No access")
        }

        let (col, next_id) = manager
            .get_chat_collection(&room)
            .await
            .context("Internal Mongo Error")
            .map_err(|e| fritz!(manager, e))??;

        info!(col);

        let col = manager
            .actual_col_name(&room, col, next_id)
            .await
            .context("Can't get actual collection name")?;

        manager
            .write(&room, &col, &message)
            .await
            .context("Can't perform write")?;

        Ok(())
    }

    #[instrument(skip(self, room_name), level = "debug")]
    async fn create_room(
        &self,
        room_name: &str,
        room_config: &RoomConfig,
    ) -> Result<Result<(), MatrixErr>> {
        if INVALID_ROOM_NAMES.contains(&room_name) {
            return Ok(Err(MatrixErr::IllegalRoomName(room_name.to_string())));
        }
        match self.get_chat_collection(&room_name).await {
            Ok(Err(MatrixErr::RoomNotFound(_))) => {}
            Ok(Err(e)) => {
                error!(
                    ?e,
                    "Unexpected error encountered (only not found should be returned)"
                );
                return Ok(Err(MatrixErr::General("Internal error".to_string())));
            }
            Ok(_) => {
                warn!("Room already exists");
                return Ok(Err(MatrixErr::RoomAlreadyExists(room_name.to_string())));
            }
            Err(e) => return Err(e),
        }

        let col_name = format!("{CHAT_PREFIX}_0");
        backoff!(self)
            .database(&room_name)
            .collection::<RoomConfig>(&col_name)
            .insert_one(room_config)
            .await
            .context("Unable to create room")?;

        debug!("Created room");

        Ok(Ok(()))
    }

    /// Get Collection of a room with the newest messages
    ///
    /// # Arguments
    ///
    /// * `room`: Room name
    ///
    /// returns: Result<Result<(String, u32), MatrixErr>>
    /// - Outer Err: Mongo Error
    /// - Inner Err: Mongo works, but request was not valid
    /// - String: Collection name
    /// - u32: Next collection id (useful if you want to write and the collection is full)
    /// - Error: Something went wrong
    #[instrument(skip_all)]
    async fn get_chat_collection(&self, room: &str) -> Result<Result<(String, u32), MatrixErr>> {
        let client = backoff!(self);
        let db = client.database(&room);
        let mut col_cursor = db
            .list_collections()
            .await
            .context("Can't list connections")?;

        let mut cnt = 0u32;

        loop {
            match col_cursor.advance().await {
                Ok(true) => {
                    cnt += 1;
                }
                Ok(false) => break,
                Err(e) => {
                    error!(?e, "Error while advancing");
                    bail!("Can't get collection for chat because of an error");
                }
            };
        }
        error!(cnt);
        if cnt == 0 {
            Ok(Err(MatrixErr::RoomNotFound(room.to_string())))
        } else if cnt == 1 {
            Ok(Ok((format!("{CHAT_PREFIX}_1"), 2)))
        } else {
            Ok(Ok((format!("{CHAT_PREFIX}_{n}", n = cnt - 1), cnt)))
        }
    }

    #[instrument(skip(self, room))]
    async fn actual_col_name(&self, room: &str, col_name: String, next_num: u32) -> Result<String> {
        let client = backoff!(self);
        let col = client.database(&room).collection::<Message>(&col_name);
        let doc_count = col
            .estimated_document_count()
            .await
            .context("Can't get doc count")?;

        if doc_count < MAX_MSGS_PER_COL {
            // If == it's already full
            debug!(doc_count, "Returning original name");
            Ok(col_name)
        } else {
            let new_name = format!("{CHAT_PREFIX}_{next_num}");
            debug!(
                new_name,
                doc_count, "Maximum reached, writing in new collection"
            );
            Ok(new_name)
        }
    }

    #[instrument(skip(self, room))]
    async fn check_access(&self, room: &str, user_name: &String) -> Result<bool> {
        let col_name = format!("{CHAT_PREFIX}_0");
        let is_allowed = backoff!(self)
            .database(&room)
            .collection::<RoomConfig>(&col_name)
            .find_one(bson::doc! {})
            .await
            .context("Unable to get config")?
            .context("No conf in room (how?)")?
            .allowed_users
            .contains(user_name);

        Ok(is_allowed)
    }

    #[instrument(skip(self, room, msg))]
    async fn write(&self, room: &str, collection: &str, msg: &Message) -> Result<()> {
        debug!("Writing message");
        let client = backoff!(self);
        let col = client.database(&room).collection::<Message>(&collection);
        col.insert_one(msg).await.context("Failed to insert msg")?;

        Ok(())
    }
}

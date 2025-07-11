use crate::MongoManager;
use anyhow::{Context, Result, bail};
use bson::DateTime;
use matrix_errors::MatrixErr;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

const CHAT_PREFIX: &str = "chat";
const MAX_MSGS_PER_COL: u64 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub user: String,
    pub content: String,
    pub timestamp: DateTime,
}

impl MongoManager {
    pub async fn write_message(room: &str, message: Message) -> Result<()> {
        let manager = super::mappings::write_manager(&room)
            .await
            .with_context(|| format!("Can't get manager for room {room}"))?;

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
            .inner_write(&room, &col, &message)
            .await
            .context("Can't perform write")?;

        Ok(())
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
    #[instrument(skip(self))]
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
        if cnt == 0 {
            return Ok(Err(MatrixErr::RoomNotFound(room.to_string())));
        }

        info!("Found {cnt} collection{}", if cnt == 1 { "" } else { "s" });

        Ok(Ok((format!("{CHAT_PREFIX}_{n}", n = cnt - 1), cnt)))
    }

    #[instrument(skip(self))]
    async fn actual_col_name(&self, room: &str, col_name: String, next_num: u32) -> Result<String> {
        let client = backoff!(self);
        let col = client.database(&room).collection::<Message>(&col_name);
        let doc_count = col
            .estimated_document_count()
            .await
            .context("Can't get doc count")?;

        if doc_count <= MAX_MSGS_PER_COL {
            Ok(col_name)
        } else {
            Ok(format!("{CHAT_PREFIX}_{next_num}"))
        }
    }

    async fn inner_write(&self, room: &str, collection: &str, msg: &Message) -> Result<()> {
        let client = backoff!(self);
        let col = client.database(&room).collection::<Message>(&collection);
        col.insert_one(msg).await.context("Failed to insert msg")?;

        Ok(())
    }
}

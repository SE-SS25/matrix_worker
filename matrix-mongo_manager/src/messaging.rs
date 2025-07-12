use super::mappings;
use crate::MongoManager;
use anyhow::{Context, Result, bail};
use bson::DateTime;
use matrix_errors::MatrixErr;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, error, info, instrument, trace, warn};

const INTERNAL_ERR_MSG: &str = "Internal server error";
const INVALID_ROOM_NAMES: &[&str] = &["admin", "config", "local"];
const CHAT_PREFIX: &str = "chat";
const MAX_MSGS_PER_COL: u64 = 3;

#[derive(Debug, Serialize, Deserialize)]
pub struct RoomConfig {
    pub allowed_users: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Message {
    pub timestamp: DateTime,
    pub author: String,
    pub content: String,
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
            .context(INTERNAL_ERR_MSG)
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

    #[instrument(skip_all)]
    pub async fn read_messages(room: &str, n: usize) -> Result<(Vec<Message>, u32)> {
        let (messages, cnt) = match mappings::read_manager(&room).await {
            Ok(either::Left(manager)) => manager
                .read_n(&room, n)
                .await
                .context(INTERNAL_ERR_MSG)
                .map_err(|e| fritz!(manager, e))??,
            Ok(either::Right((man, mig_m))) => {
                // Not the optimal approach, but the only one that guarantees that no messages are lost
                let (res, mig_res) = tokio::join!(man.read_n(&room, n), mig_m.read_n(&room, n),);
                let (mut messages, collections_read) = res
                    .context("Failed to read from manager")
                    .context(INTERNAL_ERR_MSG) // First context internal, second for return val
                    .map_err(|e| fritz!(man, e))??;
                let (migration_messages, mig_collections_read) = mig_res
                    .context("Failed to read from migration manager")
                    .context(INTERNAL_ERR_MSG)
                    .map_err(|e| fritz!(mig_m, e))??;

                messages.extend(migration_messages);

                (messages, collections_read.max(mig_collections_read))
            }
            Err(e) => {
                warn!(?e, "Failed to get migration manager");
                bail!(INTERNAL_ERR_MSG);
            }
        };

        let mut messages = messages
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        messages.sort_unstable();

        Ok((messages, cnt))
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

        let mut names = vec![];

        loop {
            match col_cursor.advance().await {
                Ok(true) => {
                    let x = col_cursor.current();
                    let name = x
                        .get("name")
                        .context("Can't execute get call for collection")?
                        .context("Name of collection is not set")?;
                    let name = name.as_str().unwrap_or("").to_string();
                    if !name.starts_with(CHAT_PREFIX) {
                        error!(name, "Invalid collection name found");
                        bail!("Internal server error");
                    }
                    let index = name[CHAT_PREFIX.len() + 1..]
                        .parse::<u32>()
                        .map_err(|e| {
                            error!(name, "Invalid collection name found (no '_' after prefix, or invalid num at end)");
                            e
                        })
                        .context("Internal server error")?;
                    names.push(index);
                }
                Ok(false) => break,
                Err(e) => {
                    error!(?e, "Error while advancing");
                    bail!("Can't get collection for chat because of an error");
                }
            };
        }
        names.sort_unstable();
        if names.is_empty() {
            Ok(Err(MatrixErr::RoomNotFound(room.to_string())))
        } else {
            let count = names[names.len() - 1].max(1);
            Ok(Ok((format!("{CHAT_PREFIX}_{count}"), count + 1)))
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

    #[instrument(skip_all)]
    async fn read_n(&self, room: &str, n: usize) -> Result<Result<(Vec<Message>, u32), MatrixErr>> {
        debug!("Trying to read up to n");
        let client = backoff!(self);

        let db = client.database(&room);
        if !self
            .room_exists(&room)
            .await
            .context("Unable to check if room exists")?
        {
            return Ok(Err(MatrixErr::RoomNotFound(room.to_string())));
        }
        let mut col_cursor = db
            .list_collections()
            .await
            .context("Can't list connections")?;

        let mut names = vec![];

        loop {
            match col_cursor.advance().await {
                Ok(true) => {
                    let collection_name = col_cursor
                        .current()
                        .get("name")
                        .context("Can't execute get call for collection")?
                        .context("Name of collection is not set")?
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    if !collection_name.starts_with(CHAT_PREFIX) {
                        error!(collection_name, "Invalid collection name found");
                        bail!("Internal server error");
                    }
                    let index = collection_name[CHAT_PREFIX.len() + 1..]
                        .parse::<u32>()
                        .map_err(|e| {
                            error!(collection_name, "Invalid collection name found (no '_' after prefix, or invalid num at end)");
                            e
                        })
                        .context("Internal server error")?;
                    if index != 0 {
                        // Skip metadata collection
                        trace!(index, "Pushing index");
                        names.push(index);
                    }
                }
                Ok(false) => break,
                Err(e) => {
                    error!(?e, "Error while advancing");
                    bail!("Can't get collection for chat because of an error");
                }
            };
        }
        names.sort_unstable();

        let mut actual_read = 0;
        let mut collections_read = 0;
        let mut messages = vec![];

        for i in (0..names.len()).rev() {
            let read_col = format!("{CHAT_PREFIX}_{col_idx}", col_idx = names[i]);
            let new_messages = self
                .read_collection(&room, &read_col)
                .await
                .with_context(|| format!("Failed to read collection {read_col:?}"))?;

            collections_read += 1;
            actual_read += new_messages.len();
            messages.extend(new_messages);

            trace!(
                n = collections_read,
                collection = read_col,
                total_read = actual_read,
                "nth run"
            );
            if actual_read >= n {
                break;
            }
        }

        Ok(Ok((messages, collections_read)))
    }

    #[instrument(skip_all)]
    async fn room_exists(&self, room: &str) -> Result<bool> {
        debug!("We are checking");
        let mut col_cursor = backoff!(self)
            .database(&room)
            .list_collections()
            .await
            .context("Can't list connections")?;

        let exists = col_cursor.advance().await.context("Can't advance cursor")?;
        debug!(exists, "We have checked");
        Ok(exists)
    }

    #[instrument(skip(self, room))]
    async fn read_collection(&self, room: &str, col: &str) -> Result<Vec<Message>> {
        let col = backoff!(self).database(&room).collection::<Message>(&col);
        let mut msg_cursor = col
            .find(bson::doc! {})
            .await
            .with_context(|| format!("Can't read messages from db {room:?} with col {col:?}"))?;

        let col_size = col.estimated_document_count().await.with_context(|| {
            format!("Can't get estimated doc count for db {room:?} with col {col:?}")
        })?;
        let mut messages = Vec::with_capacity(col_size as usize);

        loop {
            match msg_cursor.advance().await {
                Ok(true) => match msg_cursor.deserialize_current() {
                    Ok(m) => messages.push(m),
                    Err(_) => debug!("Encountered migration marker"),
                },
                Ok(false) => break,
                Err(e) => {
                    warn!(?e, "Error while advancing messages");
                    bail!("Advancing messages failed");
                }
            }
        }

        Ok(messages)
    }
}

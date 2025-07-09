use crate::MongoManager;
use bson::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub user: String,
    pub room: String,
    pub content: String,
    pub timestamp: DateTime,
}

impl MongoManager {
    pub async fn write_message(&self, message: Message) -> anyhow::Result<()> {
        todo!("Implement message writing to MongoDB")
    }
}

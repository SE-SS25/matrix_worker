use crate::MongoManager;
use anyhow::{Context, Result};
use bson::doc;
use mongodb::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, instrument};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct User {
    name: String,
}

impl MongoManager {
    #[instrument(skip(self))]
    pub async fn f(&self, user: User) -> Result<()> {
        let client = backoff!(self);

        let to_insert = doc! {
            "name": &user.name,
        };
        let users = client.database("test").collection("users");
        let insert_res = users
            .insert_one(to_insert)
            .await
            .with_context(|| format!("Unable to insert user: {user:?}"))
            .map_err(|e| fritz!(self, e))?;
        Ok(())
    }
    #[instrument(skip(self))]
    pub async fn insert(&self) -> Result<()> {
        let client = backoff!(self);
        Ok(())
    }
}

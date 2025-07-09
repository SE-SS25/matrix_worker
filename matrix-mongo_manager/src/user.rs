use crate::MongoManager;
use bson::doc;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct User {
    name: String,
}

impl MongoManager {}

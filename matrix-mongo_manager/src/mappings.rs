use crate::MongoManager;
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use uuid::Uuid;

pub static MONGO_MAPPINGS_MANAGER: LazyLock<RwLock<Mappings>> = LazyLock::new(|| RwLock::default());

#[derive(Debug, Default)]
pub struct Mappings {
    pub instances: Vec<Instance>,
    pub migration_instances: Vec<MigrationInstance>,
    pub managers: HashMap<String, MongoManager>,
}

#[derive(Debug)]
pub struct Instance {
    pub id: Uuid,
    pub url: String,
    pub from: String,
}

#[derive(Debug)]
pub struct MigrationInstance {
    pub id: Uuid,
    pub url: String,
    pub from: String,
    pub to: String,
}

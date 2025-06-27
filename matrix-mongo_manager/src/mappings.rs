use crate::MongoManager;
use anyhow::{Result, anyhow, bail};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::RwLock;
use tracing::{instrument, warn};
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

#[instrument]
pub(super) async fn write_manager(namespace: &str) -> Result<MongoManager> {
    let guard = MONGO_MAPPINGS_MANAGER.read().await;

    if let Some(manager) = guard
        .migration_instances
        .iter()
        .find(|m| *m.from <= *namespace && *m.to >= *namespace)
        .and_then(|m| guard.managers.get(&m.url))
    {
        return Ok(manager.clone());
    }

    if guard.instances.is_empty() {
        warn!("Write request received, but no Mongo DBs are registered");
        bail!("No Mongo instance available");
    }

    let instance = guard
        .instances
        .windows(2)
        .find(|w| *w[1].url > *namespace)
        .map(|w| &w[0])
        .unwrap_or(&guard.instances.last().unwrap()); // We can unwrap because we know it is not empty

    let manager = guard
        .managers
        .get(&instance.url)
        .ok_or_else(|| anyhow!("No instance for url (this should not be possible)"))
        .map(|m| m.clone())?;
    Ok(manager)
}

#[instrument]
pub(super) async fn read_manager(namespace: &str) -> Vec<MongoManager> {
    todo!()
}

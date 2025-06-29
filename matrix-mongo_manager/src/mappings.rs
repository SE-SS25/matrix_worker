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

/// Gets the appropriate MongoManager instance for writing data based on the provided namespace.
/// The function searches through migration instances and regular instances to find the matching MongoDB instance.
///
/// # Arguments
///
/// * `namespace`: The namespace string used to determine which MongoDB instance should handle the write operation
///
/// # Returns
///
/// Returns a Result containing either:
/// - Ok(MongoManager): The MongoDB manager instance that should handle the write
/// - Err: If no suitable MongoDB instance is found or other errors occur
///
#[instrument]
pub(super) async fn write_manager(namespace: &str) -> Result<MongoManager> {
    let guard = MONGO_MAPPINGS_MANAGER.read().await;

    if let Some(manager) = guard
        .migration_instances
        .iter()
        .find(|m| *m.from <= *namespace && *m.to >= *namespace)
        .and_then(|m| guard.managers.get(&m.url))
    // NOTE Not sure how much I like it, technically it should be impossible to not find one, but still...
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

/// Gets the appropriate MongoManager instances for reading data based on the provided namespace.
/// The function returns a vector of MongoManager instances that can contain either:
/// - One manager (capacity=1): When no migration is in progress and only the regular instance is used
/// - Two managers (capacity=2): During migration when both old and new instances need to be queried
///
/// # Arguments
///
/// * `namespace`: The namespace string used to determine which MongoDB instance(s) should handle the read operation
///
/// # Returns
///
/// Returns a Result containing either:
/// - Ok(Vec<MongoManager>): One or two MongoDB manager instances that should handle the read
/// - Err: If no suitable MongoDB instance is found or other errors occur
///
#[instrument]
pub(super) async fn read_manager(namespace: &str) -> Result<Vec<MongoManager>> {
    let guard = MONGO_MAPPINGS_MANAGER.read().await;

    let mut managers = guard
        .migration_instances
        .iter()
        .find(|m| *m.from <= *namespace && *m.to >= *namespace)
        .and_then(|m| guard.managers.get(&m.url))
        .map_or_else(
            || Vec::with_capacity(1),
            |m| {
                let mut v = Vec::with_capacity(2);
                v.push(m.clone());
                v
            },
        );

    if guard.instances.is_empty() {
        warn!("Write request received, but no Mongo DBs are registered");
        bail!("No Mongo instance available");
    }

    let instance = guard
        .instances
        .windows(2)
        .find(|w| *w[1].url > *namespace)
        .map(|w| &w[0])
        .unwrap_or(&guard.instances.last().unwrap());

    let manager = guard
        .managers
        .get(&instance.url)
        .ok_or_else(|| anyhow!("No instance for url (this should not be possible)"))
        .map(|m| m.clone())?;

    managers.push(manager);

    Ok(managers)
}

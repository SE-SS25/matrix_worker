use crate::MongoManager;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::{RwLock, RwLockReadGuard};
use tracing::{debug, instrument};
use uuid::Uuid;

// TODO Make private (should be pretty easy, as we only need it public to update and we can migrate that logic)
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

    let manager = get_manager_for_instance(&namespace, &guard)
        .context("Unable to get write instance manager")?;

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

    let manager = get_manager_for_instance(&namespace, &guard)
        .context("Unable to get read instance manager")?;

    managers.push(manager);

    Ok(managers)
}

/// Retrieves the appropriate MongoManager instance based on the provided namespace
/// by searching through available instances in the guard.
///
/// The function uses a sliding window approach to find the correct instance based on URL ranges.
/// If no exact match is found, it defaults to the last available instance.
///
/// # Arguments
///
/// * `namespace`: The namespace string used to determine which MongoDB instance should be used
/// * `guard`: Read guard containing the current MongoDB instances and managers
///
/// # Returns
///
/// Returns a Result containing either:
/// * `Ok(MongoManager)`: The MongoDB manager instance that matches the namespace
/// * `Err`: If no instances are available or if no manager exists for the matched instance
///
#[instrument(skip_all)]
fn get_manager_for_instance(
    namespace: &str,
    guard: &RwLockReadGuard<'_, Mappings>,
) -> Result<MongoManager> {
    debug!(instances = ?guard.instances);
    if guard.instances.is_empty() {
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

    Ok(manager)
}

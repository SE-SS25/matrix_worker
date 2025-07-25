use crate::MongoManager;
use anyhow::{Context, Result, anyhow, bail};
use either::Either;
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::{RwLock, RwLockReadGuard};
use tracing::{debug, debug_span, instrument};
use uuid::Uuid;

// TODO Make private (should be pretty easy, as we only need it public to update and we can migrate that logic)
pub static MONGO_MAPPINGS_MANAGER: LazyLock<RwLock<Mappings>> = LazyLock::new(RwLock::default);

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
    debug!(instances = ?guard.instances);

    if let Some(manager) = guard
        .migration_instances
        .iter()
        .find(|m| *m.from <= *namespace && *m.to >= *namespace)
        .and_then(|m| guard.managers.get(&m.url))
    // NOTE Not sure how much I like it, technically it should be impossible to not find one, but still...
    {
        debug!(?manager, "Found migration manager");
        return Ok(manager.clone());
    }

    let manager = get_manager_for_instance(&namespace, &guard)
        .context("Unable to get write instance manager")?;

    Ok(manager)
}

/// Fetches the relevant MongoManager instances for read operations based on the namespace.
///
/// - Provides a single manager when no migration is active.
/// - Provides two managers during a migration for querying both old and new instances.
///
/// # Arguments
///
/// * `namespace` - Namespace used to determine which MongoDB instance(s) should handle the read.
///
/// # Returns
///
/// A `Result` containing:
/// - `Ok(Either<MongoManager, (MongoManager, MongoManager)>)`:
///   - `Left(MongoManager)`: The regular MongoManager instance to handle the read when no migration is in progress.
///   - `Right((MongoManager, MongoManager))`: Both the regular MongoManager and the migration MongoManager when a migration is in progress **(in that order)**.
/// - `Err`: If no suitable instance is found or other errors occur.
#[instrument]
pub(super) async fn read_manager(
    namespace: &str,
) -> Result<Either<MongoManager, (MongoManager, MongoManager)>> {
    let guard = MONGO_MAPPINGS_MANAGER.read().await;
    debug!(instances = ?guard.instances);

    let migration_manager = guard
        .migration_instances
        .iter()
        .find(|m| *m.from <= *namespace && *m.to >= *namespace)
        .and_then(|m| guard.managers.get(&m.url))
        .map(|m| m.clone());

    let manager = get_manager_for_instance(&namespace, &guard)
        .context("Unable to get read instance manager")?;

    let res = match migration_manager {
        Some(mig_man) => either::Right((manager, mig_man)),
        None => either::Left(manager),
    };
    Ok(res)
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

    let mut instance = None;
    for (idx, iter_instance) in guard.instances.iter().enumerate() {
        let span = debug_span!("Instance", iter_instance.from);
        let _guard = span.enter();

        let from_bigger_namespace = iter_instance.from.as_str() > namespace;
        debug!(from_bigger_namespace, "Checking");
        if !from_bigger_namespace {
            continue;
        }

        if idx == 0 {
            bail!("First instance is bigger than namespace (should not be possible)");
        }

        instance = Some(&guard.instances[idx - 1]);
        break;
    }

    // Unwrap because instances can't be empty
    let instance = instance.unwrap_or(&guard.instances.last().unwrap());
    debug!(?instance, "Found instance");

    let manager = guard
        .managers
        .get(&instance.url)
        .ok_or_else(|| anyhow!("No instance for url (this should not be possible)"))
        .map(|m| m.clone())?;

    debug!(?manager, "Found manager");

    Ok(manager)
}

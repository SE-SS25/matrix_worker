use crate::DbManager;
use anyhow::{Context, Result};
use futures::future;
use itertools::Itertools;
use matrix_mongo_manager::MongoManager;
use matrix_mongo_manager::mappings::{
    Instance, MONGO_MAPPINGS_MANAGER, Mappings, MigrationInstance,
};
use sqlx::{query, query_as};
use std::process::exit;
use std::time::Duration;
use tokio::sync::RwLockWriteGuard;
use tokio::time::sleep;
use tracing::{debug, error, instrument};

const MAP_INTERVAL: Duration = Duration::from_secs(10);

impl DbManager {
    #[instrument(skip_all)]
    pub async fn manage_mongo(self) {
        loop {
            debug!("Get mappings");
            'guarded: {
                let mongo_mappings = match self.get_mappings().await {
                    Ok(mappings) => mappings,
                    Err(e) => {
                        error!(?e, "Getting Mongo mappings failed");
                        break 'guarded;
                    }
                };
                let mongo_migration_mappings = match self.get_migration_mappings().await {
                    Ok(mappings) => mappings,
                    Err(e) => {
                        error!(?e, "Getting Mongo migration mappings failed");
                        break 'guarded;
                    }
                };
                let mut guard = MONGO_MAPPINGS_MANAGER.write().await;
                guard.instances = mongo_mappings;
                guard.migration_instances = mongo_migration_mappings;
                self.set_mongo_mapping_guards(&mut guard).await;
                debug!("Set mappings");
            }
            sleep(MAP_INTERVAL).await;
        }
    }

    #[instrument(skip_all)]
    async fn get_mappings(&self) -> Result<Vec<Instance>> {
        let db_pool = backoff!(self);

        let new_mappings = query_as!(
            Instance,
            r#"
            SELECT id, url, "from"
                FROM db_mapping
                ORDER BY "from";
            "#
        )
        .fetch_all(db_pool)
        .await
        .context("Can't get Mongo mappings")
        .map_err(|e| hans!(self, e))?;

        if new_mappings.is_empty() {
            error!("No regular Mongo instances found");
            exit(1);
        }
        debug!(mappings = ?new_mappings, "Successfully got Mongo mappings"); // TODO This logs credentials

        Ok(new_mappings)
    }

    #[instrument(skip_all)]
    async fn get_migration_mappings(&self) -> Result<Vec<MigrationInstance>> {
        let db_pool = backoff!(self);

        let new_migration_records = query!(
            r#"
            SELECT id, url, "from", "to"
                FROM db_migration
                ORDER BY "from";
            "#
        )
        .fetch_all(db_pool)
        .await
        .context("Can't get Mongo mappings")
        .map_err(|e| hans!(self, e))?;

        debug!(mappings = ?new_migration_records, "Successfully got Mongo migration mappings"); // TODO This logs credentials

        let new_migration_mappings = new_migration_records
            .into_iter()
            .filter(|r| r.to.is_some())
            .map(|r| MigrationInstance {
                id: r.id,
                url: r.url,
                from: r.from,
                to: r.to.unwrap(),
            })
            .collect();

        Ok(new_migration_mappings)
    }

    #[instrument(skip_all)]
    async fn set_mongo_mapping_guards(&self, mappings: &mut RwLockWriteGuard<'_, Mappings>) {
        let existing_ids = mappings
            .instances
            .iter()
            .map(|i| i.id)
            .chain(mappings.migration_instances.iter().map(|i| i.id))
            .collect::<Vec<_>>();

        mappings.managers = mappings
            .managers
            .iter()
            .filter(|(_, m)| existing_ids.contains(&m.db_id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let existing_urls = mappings
            .managers
            .iter()
            .map(|(url, _)| url.as_str())
            .collect::<Vec<_>>();

        let futures = mappings
            .instances
            .iter()
            .map(|i| (i.url.as_str(), i.id))
            .chain(
                mappings
                    .migration_instances
                    .iter()
                    .map(|mi| (mi.url.as_str(), mi.id)),
            )
            .unique_by(|(url, _)| *url)
            .filter(|(url, _)| !existing_urls.contains(url))
            .map(|(url, id)| async move {
                let manager = MongoManager::new(url, id).await;
                (url.to_string(), manager)
            })
            .collect::<Vec<_>>();

        let new_managers = future::join_all(futures).await.into_iter();
        mappings.managers.extend(new_managers);

        if mappings.managers.is_empty() {
            error!("No Mongo managers available");
            exit(1);
        }
    }
}

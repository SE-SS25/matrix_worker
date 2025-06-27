use crate::DbManager;
use anyhow::{Context, Result};
use matrix_mongo_manager::MongoManager;
use matrix_mongo_manager::mappings::{
    Instance, MONGO_MAPPINGS_MANAGER, Mappings, MigrationInstance,
};
use sqlx::query_as;
use std::collections::HashMap;
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
            }
            sleep(MAP_INTERVAL).await;
        }
    }

    #[instrument(skip_all)]
    async fn get_mappings(&self) -> Result<Vec<Instance>> {
        let db_pool = backoff!(self);

        let mut new_mappings = query_as!(
            Instance,
            r#"
            SELECT id, url, "from"
                FROM db_mapping;
            "#
        )
        .fetch_all(db_pool)
        .await
        .context("Can't get Mongo mappings")
        .map_err(|e| hans!(self, e))?;
        new_mappings.sort_by(|a, b| a.from.cmp(&b.from));

        debug!("Successfully got Mongo mappings");

        Ok(new_mappings)
    }

    #[instrument(skip_all)]
    async fn get_migration_mappings(&self) -> Result<Vec<MigrationInstance>> {
        let db_pool = backoff!(self);

        let mut new_migration_mappings = query_as!(
            MigrationInstance,
            r#"
            SELECT id, url, "from", "to"
                FROM db_migration;
            "#
        )
        .fetch_all(db_pool)
        .await
        .context("Can't get Mongo mappings")
        .map_err(|e| hans!(self, e))?;
        new_migration_mappings.sort_by(|a, b| a.from.cmp(&b.from));

        debug!("Successfully got Mongo mappings");

        Ok(new_migration_mappings)
    }

    #[instrument(skip_all)]
    async fn set_mongo_mapping_guards(&self, mappings: &mut RwLockWriteGuard<'_, Mappings>) {
        let mut map = HashMap::new();
        let tmp_map = mappings
            .migration_instances
            .iter()
            .map(|i| (i.url.as_str(), i.id))
            .chain(
                mappings
                    .migration_instances
                    .iter()
                    .map(|mi| (mi.url.as_str(), mi.id)),
            )
            .collect::<HashMap<_, _>>();
        for (k, v) in tmp_map {
            if map.contains_key(k) {
                continue;
            }
            let manager = MongoManager::new(k, v).await;
            map.insert(k.to_string(), manager);
        }
        mappings.managers = map;
    }
}

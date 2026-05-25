use super::ResultsTable;
use crate::components::SqlCtx;
use crate::futures;
use pgone_sql::Session;
use poll_promise::Promise;
use tracing::debug;

impl ResultsTable {
    /// Load available databases from the PostgreSQL instance
    pub fn load_databases(&mut self, ctxs: &mut SqlCtx) {
        if self.databases_promise.is_some() {
            return; // Already loading
        }

        let db_id = ctxs.db.active_db_config_id.clone();
        let Some(db_id) = db_id else {
            return;
        };

        ctxs.db.ensure_storage();
        let dsn = if let Some(ref storage) = ctxs.db.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => cfg.dsn,
                Ok(None) => {
                    debug!("Database config not found: {}", db_id);
                    return;
                }
                Err(e) => {
                    debug!("Failed to load database config: {}", e);
                    return;
                }
            }
        } else {
            return;
        };

        let dsn_clone = dsn.clone();
        let (sender, promise) = Promise::new();
        self.databases_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<String>, String> = async {
                let session = Session::connect_to_postgres(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to connect to postgres: {}", e))?;

                let databases = session
                    .list_databases()
                    .await
                    .map_err(|e| format!("Failed to list databases: {}", e))?;

                Ok(databases.into_iter().map(|db| db.name).collect())
            }
            .await;

            sender.send(result);
        });
    }
}

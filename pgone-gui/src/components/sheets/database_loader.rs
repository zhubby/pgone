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

        let Some(dsn) = ctxs.db.active_dsn() else {
            debug!("Database config not found");
            return;
        };

        let pools = ctxs.db.pools.clone();
        let dsn_clone =
            crate::components::structures::utils::replace_database_in_dsn(&dsn, "postgres")
                .unwrap_or(dsn);
        let (sender, promise) = Promise::new();
        self.databases_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<String>, String> = async {
                let pool = pools.get_or_create_pool(&dsn_clone).await?;
                let session = Session::from_pool(pool);

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

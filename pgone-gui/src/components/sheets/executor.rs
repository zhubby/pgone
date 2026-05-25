use super::ResultsTable;
use super::utils;
use crate::components::SqlCtx;
use crate::futures;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row};

impl ResultsTable {
    /// Execute SQL query
    pub fn run_sql(&mut self, ctxs: &mut SqlCtx) {
        self.sql_error = None;
        self.primary_key_columns.clear();

        // Get DSN from active database config instead of session
        let db_id = match &ctxs.db.active_db_config_id {
            Some(id) => id.clone(),
            None => {
                self.sql_error = Some("No database selected".into());
                return;
            }
        };

        ctxs.db.ensure_storage();
        let mut dsn = if let Some(ref storage) = ctxs.db.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => cfg.dsn,
                Ok(None) => {
                    self.sql_error = Some("Database config not found".into());
                    return;
                }
                Err(e) => {
                    self.sql_error = Some(format!("Failed to load database config: {}", e));
                    return;
                }
            }
        } else {
            self.sql_error = Some("Storage not initialized".into());
            return;
        };

        if dsn.trim().is_empty() {
            self.sql_error = Some("DSN is empty".into());
            return;
        }

        // Replace database in DSN if a different database is selected
        if let Some(ref selected_db) = self.selected_database {
            if let Some(new_dsn) = utils::replace_database_in_dsn(&dsn, selected_db) {
                dsn = new_dsn;
            } else {
                self.sql_error = Some(format!(
                    "Failed to replace database in DSN: {}",
                    selected_db
                ));
                return;
            }
        }

        let sql = self.sql_input.clone();
        // Use a hash of the actual DSN (including database name) as the pool key
        // This ensures that different databases get different connection pools
        let pool_key = utils::calculate_dsn_hash(&dsn);

        // Get or create connection pool
        let pool = if let Some(p) = ctxs.db.pools.get(&pool_key).cloned() {
            p
        } else {
            // Create new pool with the modified DSN
            let new_pool_result = futures::block_on_async(async {
                PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
                    .map_err(|e| e.to_string())
            });
            match new_pool_result {
                Ok(new_pool) => {
                    // Save the pool for future use
                    ctxs.db.pools.insert(pool_key, new_pool.clone());
                    new_pool
                }
                Err(e) => {
                    self.sql_error = Some(format!("Failed to create connection pool: {}", e));
                    return;
                }
            }
        };

        // Try to detect primary key columns from SQL query
        // Note: detect_primary_keys is now in table_view.rs
        let pk_cols = None; // Primary key detection moved to table_view

        let res: Result<(Vec<String>, Vec<Vec<String>>), String> =
            futures::block_on_async(async move {
                let rows: Vec<PgRow> = sqlx::query(&sql)
                    .fetch_all(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut cols: Vec<String> = Vec::new();
                let mut data: Vec<Vec<String>> = Vec::new();
                if let Some(first) = rows.first() {
                    for c in first.columns() {
                        cols.push(c.name().to_string());
                    }
                }
                for row in rows.into_iter().take(10000) {
                    let mut r: Vec<String> = Vec::new();
                    let n = if cols.is_empty() {
                        row.len()
                    } else {
                        cols.len()
                    };
                    for i in 0..n {
                        r.push(crate::sql::format_cell(&row, i));
                    }
                    data.push(r);
                }
                Ok((cols, data))
            });

        match res {
            Ok((cols, rows)) => {
                self.query_columns = cols;
                self.query_rows = rows;
                // Update primary key columns if detected
                if let Some(pk) = pk_cols {
                    self.primary_key_columns = pk;
                }
                self.current_sql = Some(self.sql_input.clone());
            }
            Err(e) => {
                self.sql_error = Some(e);
            }
        }
    }
}

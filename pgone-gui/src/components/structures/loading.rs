use super::types::DbTree;
use super::utils;
use crate::components::ResultsTable;
use crate::futures;
use pgone_sql::{
    DatabaseInfo, ForeignKeyDetail, FunctionInfo, IndexInfo, MaterializedViewInfo, SchemaInfo,
    Session, TableDetail, TableInfo, TriggerInfo, ViewInfo,
};
use poll_promise::Promise;
use sqlx::Row;

pub(super) fn check_promises(tree: &mut DbTree) {
    // Check databases promise
    if let Some(ref promise) = tree.databases_promise {
        if let Some(result) = promise.ready() {
            match result {
                Ok(databases) => {
                    tree.databases = databases.clone();
                    tree.loaded_databases = true;
                }
                Err(e) => {
                    tree.error = Some(e.clone());
                    tree.loaded_databases = false;
                }
            }
            tree.databases_promise = None;
        }
    }

    // Check schemas promises
    let mut completed_schemas = Vec::new();
    for (db_name, promise) in &tree.schemas_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(schemas) => {
                    tree.schemas.insert(db_name.clone(), schemas.clone());
                    tree.loaded_schemas.insert(db_name.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load schemas for {}: {}", db_name, e));
                }
            }
            completed_schemas.push(db_name.clone());
        }
    }
    for db_name in completed_schemas {
        tree.schemas_promises.remove(&db_name);
    }

    // Check tables promises
    let mut completed_tables = Vec::new();
    for (key, promise) in &tree.tables_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(tables) => {
                    tree.tables.insert(key.clone(), tables.clone());
                    tree.loaded_tables.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load tables for {}: {}", key, e));
                }
            }
            completed_tables.push(key.clone());
        }
    }
    for key in completed_tables {
        tree.tables_promises.remove(&key);
    }

    // Check views promises
    let mut completed_views = Vec::new();
    for (key, promise) in &tree.views_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(views) => {
                    tree.views.insert(key.clone(), views.clone());
                    tree.loaded_views.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load views for {}: {}", key, e));
                }
            }
            completed_views.push(key.clone());
        }
    }
    for key in completed_views {
        tree.views_promises.remove(&key);
    }

    // Check materialized views promises
    let mut completed_materialized_views = Vec::new();
    for (key, promise) in &tree.materialized_views_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(materialized_views) => {
                    tree.materialized_views
                        .insert(key.clone(), materialized_views.clone());
                    tree.loaded_materialized_views.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!(
                        "Failed to load materialized views for {}: {}",
                        key, e
                    ));
                }
            }
            completed_materialized_views.push(key.clone());
        }
    }
    for key in completed_materialized_views {
        tree.materialized_views_promises.remove(&key);
    }

    // Check functions promises
    let mut completed_functions = Vec::new();
    for (key, promise) in &tree.functions_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(functions) => {
                    tree.functions.insert(key.clone(), functions.clone());
                    tree.loaded_functions.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load functions for {}: {}", key, e));
                }
            }
            completed_functions.push(key.clone());
        }
    }
    for key in completed_functions {
        tree.functions_promises.remove(&key);
    }

    // Check indexes promises
    let mut completed_indexes = Vec::new();
    for (key, promise) in &tree.indexes_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(indexes) => {
                    tree.indexes.insert(key.clone(), indexes.clone());
                    tree.loaded_indexes.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load indexes for {}: {}", key, e));
                }
            }
            completed_indexes.push(key.clone());
        }
    }
    for key in completed_indexes {
        tree.indexes_promises.remove(&key);
    }

    // Check foreign keys promises
    let mut completed_foreign_keys = Vec::new();
    for (key, promise) in &tree.foreign_keys_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(foreign_keys) => {
                    tree.foreign_keys.insert(key.clone(), foreign_keys.clone());
                    tree.loaded_foreign_keys.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load foreign keys for {}: {}", key, e));
                }
            }
            completed_foreign_keys.push(key.clone());
        }
    }
    for key in completed_foreign_keys {
        tree.foreign_keys_promises.remove(&key);
    }

    // Check triggers promises
    let mut completed_triggers = Vec::new();
    for (key, promise) in &tree.triggers_promises {
        if let Some(result) = promise.ready() {
            match result {
                Ok(triggers) => {
                    tree.triggers.insert(key.clone(), triggers.clone());
                    tree.loaded_triggers.insert(key.clone(), true);
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load triggers for {}: {}", key, e));
                }
            }
            completed_triggers.push(key.clone());
        }
    }
    for key in completed_triggers {
        tree.triggers_promises.remove(&key);
    }

    // Check DDL promise
    if let Some(ref promise) = tree.ddl_promise {
        if let Some(result) = promise.ready() {
            match result {
                Ok(ddl) => {
                    tree.dialog_ddl_content = ddl.clone();
                }
                Err(e) => {
                    tree.error = Some(format!("Failed to load DDL: {}", e));
                }
            }
            tree.ddl_promise = None;
        }
    }
}

pub(super) fn check_result_promises(tree: &mut DbTree, results_table: &mut ResultsTable) {
    if let Some(ref promise) = tree.results_promise {
        if let Some(result) = promise.ready() {
            match result {
                Ok((columns, rows)) => {
                    results_table.query_columns = columns.clone();
                    results_table.query_rows = rows.clone();
                }
                Err(error) => {
                    tree.error = Some(error.clone());
                }
            }
            tree.results_promise = None;
        }
    }
}

pub(super) fn load_databases(tree: &mut DbTree, db_manager: &mut crate::components::DbManager) {
    if tree.loaded_databases || tree.databases_promise.is_some() {
        return;
    }

    let Some(db_id) = tree.current_db_id.clone() else {
        return;
    };

    let dsn = if let Some(dsn) = db_manager.dsn_for_config(&db_id) {
        dsn
    } else {
        tree.error = Some("Database config not available".to_string());
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = utils::replace_database_in_dsn(&dsn, "postgres").unwrap_or(dsn);
    let (sender, promise) = Promise::new();
    tree.databases_promise = Some(promise);

    futures::spawn(async move {
        let result: Result<Vec<DatabaseInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_databases()
                .await
                .map_err(|e| format!("Failed to list databases: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_schemas(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
) {
    if tree.loaded_schemas.get(database).copied().unwrap_or(false)
        || tree.schemas_promises.contains_key(database)
    {
        return; // Already loading
    }

    let Some(db_id) = tree.current_db_id.clone() else {
        return;
    };

    let Some(dsn) = db_manager.dsn_for_config_database(&db_id, database) else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let (sender, promise) = Promise::new();
    tree.schemas_promises.insert(database.to_string(), promise);

    futures::spawn(async move {
        let result: Result<Vec<SchemaInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_schemas()
                .await
                .map_err(|e| format!("Failed to list schemas: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_tables(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    if tree.loaded_tables.get(&key).copied().unwrap_or(false)
        || tree.tables_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let Some(db_id) = tree.current_db_id.clone() else {
        return;
    };

    let Some(dsn) = db_manager.dsn_for_config_database(&db_id, database) else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let (sender, promise) = Promise::new();
    tree.tables_promises.insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<TableInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_tables(Some(&schema_clone))
                .await
                .map_err(|e| format!("Failed to list tables: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_views(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    if tree.loaded_views.get(&key).copied().unwrap_or(false)
        || tree.views_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let Some(db_id) = tree.current_db_id.clone() else {
        return;
    };

    let Some(dsn) = db_manager.dsn_for_config_database(&db_id, database) else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let (sender, promise) = Promise::new();
    tree.views_promises.insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<ViewInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_views(Some(&schema_clone))
                .await
                .map_err(|e| format!("Failed to list views: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_materialized_views(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    if tree
        .loaded_materialized_views
        .get(&key)
        .copied()
        .unwrap_or(false)
        || tree.materialized_views_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let Some(db_id) = tree.current_db_id.clone() else {
        return;
    };

    let Some(dsn) = db_manager.dsn_for_config_database(&db_id, database) else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let (sender, promise) = Promise::new();
    tree.materialized_views_promises
        .insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<MaterializedViewInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_materialized_views(Some(&schema_clone))
                .await
                .map_err(|e| format!("Failed to list materialized views: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_functions(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    if tree.loaded_functions.get(&key).copied().unwrap_or(false)
        || tree.functions_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let Some(db_id) = tree.current_db_id.clone() else {
        return;
    };

    let Some(dsn) = db_manager.dsn_for_config_database(&db_id, database) else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let (sender, promise) = Promise::new();
    tree.functions_promises.insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<FunctionInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_functions(Some(&schema_clone))
                .await
                .map_err(|e| format!("Failed to list functions: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn refresh_databases(tree: &mut DbTree, db_manager: &mut crate::components::DbManager) {
    tree.loaded_databases = false;
    tree.databases_promise = None;
    load_databases(tree, db_manager);
}

pub(super) fn refresh_schemas(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
) {
    tree.loaded_schemas.insert(database.to_string(), false);
    tree.schemas_promises.remove(database);
    load_schemas(tree, db_manager, database);
}

pub(super) fn refresh_tables(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    tree.loaded_tables.insert(key.clone(), false);
    tree.tables_promises.remove(&key);
    load_tables(tree, db_manager, database, schema);
}

pub(super) fn refresh_views(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    tree.loaded_views.insert(key.clone(), false);
    tree.views_promises.remove(&key);
    load_views(tree, db_manager, database, schema);
}

pub(super) fn refresh_materialized_views(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    tree.loaded_materialized_views.insert(key.clone(), false);
    tree.materialized_views_promises.remove(&key);
    load_materialized_views(tree, db_manager, database, schema);
}

pub(super) fn refresh_functions(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    let key = format!("{}.{}", database, schema);
    tree.loaded_functions.insert(key.clone(), false);
    tree.functions_promises.remove(&key);
    load_functions(tree, db_manager, database, schema);
}

pub(super) fn refresh_schema_children(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
) {
    refresh_tables(tree, db_manager, database, schema);
    refresh_views(tree, db_manager, database, schema);
    refresh_materialized_views(tree, db_manager, database, schema);
    refresh_functions(tree, db_manager, database, schema);
}

pub(super) fn query_table_data(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    table: &str,
) {
    if let Some(connection_id) = tree.current_db_id.clone() {
        db_manager.select_db_config(&connection_id);
        results_table.current_db_id = Some(connection_id);
    }

    // Generate SQL query statement for the table component to execute
    // Use LIMIT 100 to limit result size and avoid querying large datasets
    let sql = format!("SELECT * FROM \"{}\".\"{}\" LIMIT 100", schema, table);

    // Set the SQL into the table component's input
    results_table.sql_input = sql.clone();

    // Set the selected database so the table component can correctly switch database connections
    results_table.selected_database = Some(database.to_string());

    // Set the current SQL for display
    results_table.current_sql = Some(sql);

    // Request SQL execution; the table component will execute it on the next render
    results_table.execute_sql_requested = true;
}

/// Load table structure details for the design dialog
pub(super) fn load_table_detail_for_design(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    // Check if it's the same table; if not, clear old data
    let current_table = (database.to_string(), schema.to_string(), table.to_string());
    if let Some(ref loaded_table) = tree.design_table_loaded {
        if *loaded_table != current_table {
            // Switched tables, clear old data
            tree.design_table_detail = None;
            tree.design_table_columns.clear();
            tree.design_table_promise = None;
        } else if tree.design_table_promise.is_some() {
            // Same table and still loading, no need to reload
            return;
        } else if tree.design_table_detail.is_some() {
            // Same table and already loaded, no need to reload
            return;
        }
    } else if tree.design_table_promise.is_some() {
        // No record of loaded table, but promise exists - may be the first load
        return; // Already loading
    }

    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    // Record the table currently being loaded
    tree.design_table_loaded = Some(current_table.clone());

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.design_table_promise = Some(promise);

    futures::spawn(async move {
        let result: Result<TableDetail, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .get_table_detail(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to get table detail: {}", e))
        }
        .await;

        sender.send(result);
    });
}

/// Load table DDL
pub(super) fn load_table_ddl(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    if tree.ddl_promise.is_some() {
        return; // Already loading
    }

    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.ddl_promise = Some(promise);

    futures::spawn(async move {
        let result: Result<String, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            // Get table structure details
            let table_detail = session
                .get_table_detail(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to get table detail: {}", e))?;

            // Get index information
            let indexes = session
                .list_table_indexes(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to list indexes: {}", e))?;

            // Generate DDL
            Ok(utils::generate_table_ddl(
                &schema_clone,
                &table_clone,
                &table_detail,
                &indexes,
            ))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn get_dsn_for_database(
    tree: &DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
) -> Option<String> {
    let db_id = tree.current_db_id.as_ref()?;
    db_manager.dsn_for_config_database(db_id, database)
}

pub(super) fn load_indexes(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    let key = format!("{}.{}.{}", database, schema, table);
    if tree.loaded_indexes.get(&key).copied().unwrap_or(false)
        || tree.indexes_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.indexes_promises.insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<IndexInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            session
                .list_table_indexes(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to list indexes: {}", e))
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_foreign_keys(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    let key = format!("{}.{}.{}", database, schema, table);
    if tree.loaded_foreign_keys.get(&key).copied().unwrap_or(false)
        || tree.foreign_keys_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.foreign_keys_promises.insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<ForeignKeyDetail>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let detail = session
                .get_table_detail(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to get table detail: {}", e))?;

            Ok(detail.foreign_keys)
        }
        .await;

        sender.send(result);
    });
}

pub(super) fn load_triggers(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    let key = format!("{}.{}.{}", database, schema, table);
    if tree.loaded_triggers.get(&key).copied().unwrap_or(false)
        || tree.triggers_promises.contains_key(&key)
    {
        return; // Already loading
    }

    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.triggers_promises.insert(key.clone(), promise);

    futures::spawn(async move {
        let result: Result<Vec<TriggerInfo>, String> = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let triggers = session
                .list_triggers(Some(&schema_clone))
                .await
                .map_err(|e| format!("Failed to list triggers: {}", e))?;

            // Filter triggers for this specific table
            Ok(triggers
                .into_iter()
                .filter(|t| t.table_schema == schema_clone && t.table_name == table_clone)
                .collect())
        }
        .await;

        sender.send(result);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{DbManager, ResultsTable};

    #[test]
    fn query_table_data_switches_active_connection_to_tree_connection() {
        let mut tree = DbTree {
            current_db_id: Some("analytics".to_string()),
            ..Default::default()
        };
        let mut db_manager = DbManager::default();
        db_manager.active_db_config_id = Some("default".to_string());
        let mut results_table = ResultsTable::default();

        query_table_data(
            &mut tree,
            &mut db_manager,
            &mut results_table,
            "warehouse",
            "public",
            "events",
        );

        assert_eq!(db_manager.active_db_config_id.as_deref(), Some("analytics"));
        assert_eq!(results_table.current_db_id.as_deref(), Some("analytics"));
        assert_eq!(
            results_table.selected_database.as_deref(),
            Some("warehouse")
        );
        assert_eq!(
            results_table.sql_input,
            "SELECT * FROM \"public\".\"events\" LIMIT 100"
        );
        assert!(results_table.execute_sql_requested);
    }
}

pub(super) fn refresh_indexes(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    let key = format!("{}.{}.{}", database, schema, table);
    tree.loaded_indexes.insert(key.clone(), false);
    tree.indexes_promises.remove(&key);
    load_indexes(tree, db_manager, database, schema, table);
}

pub(super) fn refresh_foreign_keys(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    let key = format!("{}.{}.{}", database, schema, table);
    tree.loaded_foreign_keys.insert(key.clone(), false);
    tree.foreign_keys_promises.remove(&key);
    load_foreign_keys(tree, db_manager, database, schema, table);
}

pub(super) fn refresh_triggers(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    let key = format!("{}.{}.{}", database, schema, table);
    tree.loaded_triggers.insert(key.clone(), false);
    tree.triggers_promises.remove(&key);
    load_triggers(tree, db_manager, database, schema, table);
}

pub(super) fn refresh_table_children(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    refresh_indexes(tree, db_manager, database, schema, table);
    refresh_foreign_keys(tree, db_manager, database, schema, table);
    refresh_triggers(tree, db_manager, database, schema, table);
}

pub(super) fn query_index_detail(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    _results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    table: &str,
    index: &str,
) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let index_clone = index.to_string();

    let (sender, promise) = Promise::new();
    tree.results_promise = Some(promise);
    futures::spawn(async move {
        let result = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let indexes = session
                .list_table_indexes(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to list indexes: {}", e))?;

            let index_info = indexes
                .into_iter()
                .find(|idx| idx.name == index_clone)
                .ok_or_else(|| format!("Index '{}' not found", index_clone))?;

            let columns = vec!["Property".to_string(), "Value".to_string()];
            let mut rows = Vec::new();
            rows.push(vec!["Name".to_string(), index_info.name.clone()]);
            rows.push(vec!["Unique".to_string(), index_info.unique.to_string()]);
            rows.push(vec!["Columns".to_string(), index_info.columns.join(", ")]);
            if let Some(ref def) = index_info.definition {
                rows.push(vec!["Definition".to_string(), def.clone()]);
            }
            if let Some(ref desc) = index_info.description {
                rows.push(vec!["Description".to_string(), desc.clone()]);
            }
            Ok((columns, rows))
        }
        .await;
        sender.send(result);
    });
}

pub(super) fn query_foreign_key_detail(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    _results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    table: &str,
    fk_info: &str,
) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    // Parse fk_info: "columns|ref_table|idx"
    let parts: Vec<&str> = fk_info.split('|').collect();
    if parts.len() < 2 {
        tree.error = Some("Invalid foreign key info format".to_string());
        return;
    }
    let fk_columns: Vec<String> = parts[0].split(',').map(str::to_string).collect();
    let ref_table_part = parts[1];
    // Extract schema and table from ref_table (format: "schema.table" or just "table")
    let (ref_schema, ref_table_name) = if let Some(dot_pos) = ref_table_part.rfind('.') {
        (&ref_table_part[..dot_pos], &ref_table_part[dot_pos + 1..])
    } else {
        (schema, ref_table_part)
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let ref_schema_clone = ref_schema.to_string();
    let ref_table_name_clone = ref_table_name.to_string();

    let (sender, promise) = Promise::new();
    tree.results_promise = Some(promise);
    futures::spawn(async move {
        let result = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;

            // First, find the constraint name by matching columns and ref_table
            let fk_rows = sqlx::query(
                r#"
            SELECT DISTINCT tc.constraint_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
              ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            JOIN information_schema.referential_constraints rc
              ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema
            JOIN information_schema.constraint_column_usage ccu
              ON ccu.constraint_name = rc.unique_constraint_name
              AND ccu.constraint_schema = rc.unique_constraint_schema
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema = $1
              AND tc.table_name = $2
              AND ccu.table_schema = $3
              AND ccu.table_name = $4
            "#,
            )
            .bind(&schema_clone)
            .bind(&table_clone)
            .bind(&ref_schema_clone)
            .bind(&ref_table_name_clone)
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Failed to query foreign key constraints: {}", e))?;

            // Find the constraint that matches all columns
            let mut matching_constraint: Option<String> = None;
            for row in fk_rows {
                let constraint_name: String = row.get(0);
                // Get columns for this constraint
                let col_rows = sqlx::query(
                    r#"
                SELECT kcu.column_name
                FROM information_schema.key_column_usage kcu
                WHERE kcu.constraint_name = $1 AND kcu.table_schema = $2 AND kcu.table_name = $3
                ORDER BY kcu.ordinal_position
                "#,
                )
                .bind(&constraint_name)
                .bind(&schema_clone)
                .bind(&table_clone)
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to query constraint columns: {}", e))?;

                let constraint_cols: Vec<String> = col_rows.iter().map(|r| r.get(0)).collect();
                if constraint_cols.len() == fk_columns.len()
                    && constraint_cols
                        .iter()
                        .zip(fk_columns.iter())
                        .all(|(a, b)| a == b)
                {
                    matching_constraint = Some(constraint_name);
                    break;
                }
            }

            let constraint_name =
                matching_constraint.ok_or_else(|| format!("Foreign key not found"))?;

            // Now get the full foreign key details
            let fk_row = sqlx::query(
                r#"
            SELECT
                tc.constraint_name,
                kcu.column_name,
                ccu.table_schema AS ref_schema,
                ccu.table_name AS ref_table,
                ccu.column_name AS ref_column,
                rc.update_rule,
                rc.delete_rule
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
              ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            JOIN information_schema.referential_constraints rc
              ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema
            JOIN information_schema.constraint_column_usage ccu
              ON ccu.constraint_name = rc.unique_constraint_name
              AND ccu.constraint_schema = rc.unique_constraint_schema
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema = $1
              AND tc.table_name = $2
              AND tc.constraint_name = $3
            ORDER BY kcu.ordinal_position
            LIMIT 1
            "#,
            )
            .bind(&schema_clone)
            .bind(&table_clone)
            .bind(&constraint_name)
            .fetch_optional(&pool)
            .await
            .map_err(|e| format!("Failed to query foreign key: {}", e))?
            .ok_or_else(|| format!("Foreign key not found"))?;

            // Build result table
            let columns = vec!["Property".to_string(), "Value".to_string()];
            let mut rows = Vec::new();

            let ref_schema: String = fk_row.get("ref_schema");
            let ref_table: String = fk_row.get("ref_table");
            let on_update: Option<String> = fk_row.try_get("update_rule").ok();
            let on_delete: Option<String> = fk_row.try_get("delete_rule").ok();

            rows.push(vec!["Constraint Name".to_string(), constraint_name.clone()]);
            rows.push(vec![
                "Referenced Table".to_string(),
                format!("{}.{}", ref_schema, ref_table),
            ]);
            if let Some(update) = on_update {
                rows.push(vec!["On Update".to_string(), update]);
            }
            if let Some(delete) = on_delete {
                rows.push(vec!["On Delete".to_string(), delete]);
            }

            // Get all columns
            let fk_rows = sqlx::query(
                r#"
            SELECT kcu.column_name, ccu.column_name AS ref_column
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
              ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            JOIN information_schema.referential_constraints rc
              ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema
            JOIN information_schema.constraint_column_usage ccu
              ON ccu.constraint_name = rc.unique_constraint_name
              AND ccu.constraint_schema = rc.unique_constraint_schema
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema = $1
              AND tc.table_name = $2
              AND tc.constraint_name = $3
            ORDER BY kcu.ordinal_position
            "#,
            )
            .bind(&schema_clone)
            .bind(&table_clone)
            .bind(&constraint_name)
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Failed to query foreign key columns: {}", e))?;

            let mut local_cols = Vec::new();
            let mut ref_cols = Vec::new();
            for row in fk_rows {
                local_cols.push(row.get::<String, _>("column_name"));
                ref_cols.push(row.get::<String, _>("ref_column"));
            }

            rows.push(vec!["Local Columns".to_string(), local_cols.join(", ")]);
            rows.push(vec!["Referenced Columns".to_string(), ref_cols.join(", ")]);

            Ok((columns, rows))
        }
        .await;
        sender.send(result);
    });
}

pub(super) fn query_trigger_detail(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    _results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    table: &str,
    trigger: &str,
) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let trigger_clone = trigger.to_string();
    let table_clone = table.to_string();

    let (sender, promise) = Promise::new();
    tree.results_promise = Some(promise);
    futures::spawn(async move {
        let result = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let triggers = session
                .get_trigger_info(&schema_clone, &trigger_clone)
                .await
                .map_err(|e| format!("Failed to get trigger info: {}", e))?;

            let trigger_info = triggers
                .into_iter()
                .find(|t| t.name == trigger_clone && t.table_name == table_clone)
                .ok_or_else(|| format!("Trigger '{}' not found", trigger_clone))?;

            // Convert to table format
            let columns = vec!["Property".to_string(), "Value".to_string()];
            let mut rows = Vec::new();

            rows.push(vec!["Name".to_string(), trigger_info.name.clone()]);
            rows.push(vec![
                "Table".to_string(),
                format!("{}.{}", trigger_info.table_schema, trigger_info.table_name),
            ]);
            rows.push(vec!["Timing".to_string(), trigger_info.timing.clone()]);
            rows.push(vec!["Events".to_string(), trigger_info.events.join(", ")]);
            rows.push(vec![
                "Enabled".to_string(),
                trigger_info.enabled.to_string(),
            ]);
            if let Some(ref func) = trigger_info.function_name {
                rows.push(vec!["Function".to_string(), func.clone()]);
            }
            if let Some(ref desc) = trigger_info.description {
                rows.push(vec!["Description".to_string(), desc.clone()]);
            }

            Ok((columns, rows))
        }
        .await;
        sender.send(result);
    });
}

pub(super) fn query_view_detail(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    _results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    view: &str,
) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let view_clone = view.to_string();

    let (sender, promise) = Promise::new();
    tree.results_promise = Some(promise);
    futures::spawn(async move {
        let result = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let view_info = session
                .get_view_info(&schema_clone, &view_clone)
                .await
                .map_err(|e| format!("Failed to get view info: {}", e))?;

            // Convert to table format
            let columns = vec!["Property".to_string(), "Value".to_string()];
            let mut rows = Vec::new();

            rows.push(vec!["Schema".to_string(), view_info.schema.clone()]);
            rows.push(vec!["Name".to_string(), view_info.name.clone()]);
            rows.push(vec!["Owner".to_string(), view_info.owner.clone()]);
            if let Some(ref def) = view_info.definition {
                rows.push(vec!["Definition".to_string(), def.clone()]);
            }
            if let Some(ref desc) = view_info.description {
                rows.push(vec!["Description".to_string(), desc.clone()]);
            }

            Ok((columns, rows))
        }
        .await;
        sender.send(result);
    });
}

pub(super) fn query_materialized_view_detail(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    _results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    materialized_view: &str,
) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let matview_clone = materialized_view.to_string();

    let (sender, promise) = Promise::new();
    tree.results_promise = Some(promise);
    futures::spawn(async move {
        let result = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let matview_info = session
                .get_materialized_view_info(&schema_clone, &matview_clone)
                .await
                .map_err(|e| format!("Failed to get materialized view info: {}", e))?;

            // Convert to table format
            let columns = vec!["Property".to_string(), "Value".to_string()];
            let mut rows = Vec::new();

            rows.push(vec!["Schema".to_string(), matview_info.schema.clone()]);
            rows.push(vec!["Name".to_string(), matview_info.name.clone()]);
            rows.push(vec!["Owner".to_string(), matview_info.owner.clone()]);
            if let Some(ref def) = matview_info.definition {
                rows.push(vec!["Definition".to_string(), def.clone()]);
            }
            if let Some(ref desc) = matview_info.description {
                rows.push(vec!["Description".to_string(), desc.clone()]);
            }

            Ok((columns, rows))
        }
        .await;
        sender.send(result);
    });
}

pub(super) fn query_function_detail(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    _results_table: &mut ResultsTable,
    database: &str,
    schema: &str,
    function: &str,
) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else {
        return;
    };

    let pools = db_manager.pools.clone();
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let function_clone = function.to_string();

    let (sender, promise) = Promise::new();
    tree.results_promise = Some(promise);
    futures::spawn(async move {
        let result = async {
            let pool = pools.get_or_create_pool(&dsn_clone).await?;
            let session = Session::from_pool(pool);

            let functions = session
                .get_function_info(&schema_clone, &function_clone)
                .await
                .map_err(|e| format!("Failed to get function info: {}", e))?;

            // Use the first function if multiple overloads exist
            let function_info = functions
                .first()
                .ok_or_else(|| format!("Function '{}' not found", function_clone))?;

            // Convert to table format
            let columns = vec!["Property".to_string(), "Value".to_string()];
            let mut rows = Vec::new();

            rows.push(vec!["Schema".to_string(), function_info.schema.clone()]);
            rows.push(vec!["Name".to_string(), function_info.name.clone()]);
            rows.push(vec!["Owner".to_string(), function_info.owner.clone()]);
            if let Some(ref lang) = function_info.language {
                rows.push(vec!["Language".to_string(), lang.clone()]);
            }
            if let Some(ref ret_type) = function_info.return_type {
                rows.push(vec!["Return Type".to_string(), ret_type.clone()]);
            }
            if let Some(ref def) = function_info.definition {
                rows.push(vec!["Definition".to_string(), def.clone()]);
            }
            if let Some(ref desc) = function_info.description {
                rows.push(vec!["Description".to_string(), desc.clone()]);
            }
            if functions.len() > 1 {
                rows.push(vec![
                    "Overloads".to_string(),
                    format!("{} overloads", functions.len()),
                ]);
            }

            Ok((columns, rows))
        }
        .await;
        sender.send(result);
    });
}

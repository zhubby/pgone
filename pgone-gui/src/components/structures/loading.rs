use super::types::DbTree;
use pgone_sql::{DatabaseInfo, SchemaInfo, Session, TableInfo, IndexInfo, ForeignKeyDetail, TriggerInfo, TableDetail};
use poll_promise::Promise;
use crate::components::ResultsTable;
use crate::futures;
use super::utils;

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
}

pub(super) fn load_databases(tree: &mut DbTree, db_manager: &mut crate::components::DbManager) {
    let Some(db_id) = db_manager.active_db_config_id.clone() else {
        return;
    };
    
    db_manager.ensure_storage();
    let dsn = if let Some(ref storage) = db_manager.storage {
        if let Ok(Some(cfg)) = futures::block_on_async(async {
            storage.get_db_config(&db_id).await
        }) {
            cfg.dsn
        } else {
            tree.error = Some("Failed to get database config".to_string());
            return;
        }
    } else {
        tree.error = Some("Storage not available".to_string());
        return;
    };
    
    let dsn_clone = dsn.clone();
    let (sender, promise) = Promise::new();
    tree.databases_promise = Some(promise);
    
    futures::spawn(async move {
        let result: Result<Vec<DatabaseInfo>, String> = async {
            let session = Session::connect_to_postgres(&dsn_clone)
                .await
                .map_err(|e| format!("Failed to connect to postgres: {}", e))?;
            
            session.list_databases()
                .await
                .map_err(|e| format!("Failed to list databases: {}", e))
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn load_schemas(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str) {
    if tree.schemas_promises.contains_key(database) {
        return; // Already loading
    }
    
    let Some(db_id) = db_manager.active_db_config_id.clone() else {
        return;
    };
    
    db_manager.ensure_storage();
    let dsn = if let Some(ref storage) = db_manager.storage {
        if let Ok(Some(cfg)) = futures::block_on_async(async {
            storage.get_db_config(&db_id).await
        }) {
            // Replace database name in DSN while preserving password
            utils::replace_database_in_dsn(&cfg.dsn, database).unwrap_or_else(|| {
                // Fallback to manual construction if URL parsing fails
                if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                    format!("{}://{}@{}:{}/{}", 
                        parsed.engine, parsed.user, parsed.host, parsed.port, database)
                } else {
                    cfg.dsn.clone()
                }
            })
        } else {
            return;
        }
    } else {
        return;
    };
    
    let dsn_clone = dsn.clone();
    let (sender, promise) = Promise::new();
    tree.schemas_promises.insert(database.to_string(), promise);

    futures::spawn(async move {
        let result: Result<Vec<SchemaInfo>, String> = async {
            let session = Session::new(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
            session.list_schemas()
                    .await
                .map_err(|e| format!("Failed to list schemas: {}", e))
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn load_tables(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str) {
    let key = format!("{}.{}", database, schema);
    if tree.tables_promises.contains_key(&key) {
        return; // Already loading
    }
    
    let Some(db_id) = db_manager.active_db_config_id.clone() else {
        return;
    };
    
    db_manager.ensure_storage();
    let dsn = if let Some(ref storage) = db_manager.storage {
        if let Ok(Some(cfg)) = futures::block_on_async(async {
            storage.get_db_config(&db_id).await
        }) {
            // Replace database name in DSN while preserving password
            utils::replace_database_in_dsn(&cfg.dsn, database).unwrap_or_else(|| {
                // Fallback to manual construction if URL parsing fails
                if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                    format!("{}://{}@{}:{}/{}", 
                        parsed.engine, parsed.user, parsed.host, parsed.port, database)
                } else {
                    cfg.dsn.clone()
                }
            })
        } else {
            return;
        }
    } else {
        return;
    };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let (sender, promise) = Promise::new();
    tree.tables_promises.insert(key.clone(), promise);
    
    futures::spawn(async move {
        let result: Result<Vec<TableInfo>, String> = async {
            let session = Session::new(&dsn_clone)
                    .await
                .map_err(|e| format!("Failed to create session: {}", e))?;
                
            session.list_tables(Some(&schema_clone))
                    .await
                .map_err(|e| format!("Failed to list tables: {}", e))
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn query_table_data(_tree: &mut DbTree, _db_manager: &mut crate::components::DbManager, results_table: &mut ResultsTable, database: &str, schema: &str, table: &str) {
    // 生成 SQL 查询语句，让表格组件自己执行查询
    // 使用 LIMIT 100 限制结果数量，避免查询过大数据集
    let sql = format!("SELECT * FROM \"{}\".\"{}\" LIMIT 100", schema, table);
    
    // 设置 SQL 到表格组件的输入框
    results_table.sql_input = sql.clone();
    
    // 设置选中的数据库，以便表格组件能够正确切换数据库连接
    results_table.selected_database = Some(database.to_string());
    
    // 设置当前 SQL 用于显示
    results_table.current_sql = Some(sql);
    
    // 请求执行 SQL，表格组件会在下次渲染时自动执行
    results_table.execute_sql_requested = true;
}

/// 加载表结构详情用于设计对话框
pub(super) fn load_table_detail_for_design(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    table: &str,
) {
    if tree.design_table_promise.is_some() {
        return; // Already loading
    }
    
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.design_table_promise = Some(promise);
    
    futures::spawn(async move {
        let result: Result<TableDetail, String> = async {
            let session = Session::new(&dsn_clone)
                .await
                .map_err(|e| format!("Failed to create session: {}", e))?;
            
            session.get_table_detail(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to get table detail: {}", e))
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn get_dsn_for_database(_tree: &DbTree, db_manager: &mut crate::components::DbManager, database: &str) -> Option<String> {
    let db_id = db_manager.active_db_config_id.clone()?;
    db_manager.ensure_storage();
    if let Some(ref storage) = db_manager.storage {
        if let Ok(Some(cfg)) = futures::block_on_async(async {
            storage.get_db_config(&db_id).await
        }) {
            // Replace database name in DSN while preserving password
            return utils::replace_database_in_dsn(&cfg.dsn, database).or_else(|| {
                // Fallback to manual construction if URL parsing fails
                if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                    Some(format!("{}://{}@{}:{}/{}", 
                        parsed.engine, parsed.user, parsed.host, parsed.port, database))
                } else {
                    Some(cfg.dsn.clone())
                }
            });
        }
    }
    None
}

pub(super) fn load_indexes(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, table: &str) {
    let key = format!("{}.{}.{}", database, schema, table);
    if tree.indexes_promises.contains_key(&key) {
        return; // Already loading
    }
    
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.indexes_promises.insert(key.clone(), promise);
    
    futures::spawn(async move {
        let result: Result<Vec<IndexInfo>, String> = async {
            let session = Session::new(&dsn_clone)
                .await
                .map_err(|e| format!("Failed to create session: {}", e))?;
            
            session.list_table_indexes(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to list indexes: {}", e))
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn load_foreign_keys(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, table: &str) {
    let key = format!("{}.{}.{}", database, schema, table);
    if tree.foreign_keys_promises.contains_key(&key) {
        return; // Already loading
    }
    
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.foreign_keys_promises.insert(key.clone(), promise);
    
    futures::spawn(async move {
        let result: Result<Vec<ForeignKeyDetail>, String> = async {
            let session = Session::new(&dsn_clone)
                .await
                .map_err(|e| format!("Failed to create session: {}", e))?;
            
            let detail = session.get_table_detail(&schema_clone, &table_clone)
                .await
                .map_err(|e| format!("Failed to get table detail: {}", e))?;
            
            Ok(detail.foreign_keys)
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn load_triggers(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, table: &str) {
    let key = format!("{}.{}.{}", database, schema, table);
    if tree.triggers_promises.contains_key(&key) {
        return; // Already loading
    }
    
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let (sender, promise) = Promise::new();
    tree.triggers_promises.insert(key.clone(), promise);
    
    futures::spawn(async move {
        let result: Result<Vec<TriggerInfo>, String> = async {
            let session = Session::new(&dsn_clone)
                .await
                .map_err(|e| format!("Failed to create session: {}", e))?;
            
            let triggers = session.list_triggers(Some(&schema_clone))
                .await
                .map_err(|e| format!("Failed to list triggers: {}", e))?;
            
            // Filter triggers for this specific table
            Ok(triggers.into_iter()
                .filter(|t| t.table_schema == schema_clone && t.table_name == table_clone)
                .collect())
        }.await;
        
        sender.send(result);
    });
}

pub(super) fn query_index_detail(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, results_table: &mut ResultsTable, database: &str, schema: &str, table: &str, index: &str) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let index_clone = index.to_string();
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn_clone)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        let indexes = session.list_table_indexes(&schema_clone, &table_clone)
            .await
            .map_err(|e| format!("Failed to list indexes: {}", e))?;
        
        let index_info = indexes.into_iter()
            .find(|idx| idx.name == index_clone)
            .ok_or_else(|| format!("Index '{}' not found", index))?;
        
        // Convert to table format
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
    });
    
    match result {
        Ok((columns, rows)) => {
            results_table.query_columns = columns;
            results_table.query_rows = rows;
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn query_foreign_key_detail(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, results_table: &mut ResultsTable, database: &str, schema: &str, table: &str, fk_info: &str) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    // Parse fk_info: "columns|ref_table|idx"
    let parts: Vec<&str> = fk_info.split('|').collect();
    if parts.len() < 2 {
        tree.error = Some("Invalid foreign key info format".to_string());
        return;
    }
    let fk_columns: Vec<&str> = parts[0].split(',').collect();
    let ref_table_part = parts[1];
    // Extract schema and table from ref_table (format: "schema.table" or just "table")
    let (ref_schema, ref_table_name) = if let Some(dot_pos) = ref_table_part.rfind('.') {
        (&ref_table_part[..dot_pos], &ref_table_part[dot_pos + 1..])
    } else {
        (schema, ref_table_part)
    };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let table_clone = table.to_string();
    let ref_schema_clone = ref_schema.to_string();
    let ref_table_name_clone = ref_table_name.to_string();
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn_clone)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        // Query foreign key constraint name from information_schema using columns and ref_table
        let conn = session.get_connection().await
            .map_err(|e| format!("Failed to get connection: {}", e))?;
        
        // First, find the constraint name by matching columns and ref_table
        let fk_rows = conn.query(
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
            &[&schema_clone, &table_clone, &ref_schema_clone, &ref_table_name_clone],
        )
        .await
        .map_err(|e| format!("Failed to query foreign key constraints: {}", e))?;
        
        // Find the constraint that matches all columns
        let mut matching_constraint: Option<String> = None;
        for row in fk_rows {
            let constraint_name: String = row.get(0);
            // Get columns for this constraint
            let col_rows = conn.query(
                r#"
                SELECT kcu.column_name
                FROM information_schema.key_column_usage kcu
                WHERE kcu.constraint_name = $1 AND kcu.table_schema = $2 AND kcu.table_name = $3
                ORDER BY kcu.ordinal_position
                "#,
                &[&constraint_name, &schema_clone, &table_clone],
            )
            .await
            .map_err(|e| format!("Failed to query constraint columns: {}", e))?;
            
            let constraint_cols: Vec<String> = col_rows.iter().map(|r| r.get(0)).collect();
            if constraint_cols.len() == fk_columns.len() && 
               constraint_cols.iter().zip(fk_columns.iter()).all(|(a, b)| a == *b) {
                matching_constraint = Some(constraint_name);
                break;
            }
        }
        
        let constraint_name = matching_constraint.ok_or_else(|| format!("Foreign key not found"))?;
        
        // Now get the full foreign key details
        let fk_row = conn.query_opt(
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
            &[&schema_clone, &table_clone, &constraint_name],
        )
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
        rows.push(vec!["Referenced Table".to_string(), format!("{}.{}", ref_schema, ref_table)]);
        if let Some(update) = on_update {
            rows.push(vec!["On Update".to_string(), update]);
        }
        if let Some(delete) = on_delete {
            rows.push(vec!["On Delete".to_string(), delete]);
        }
        
        // Get all columns
        let fk_rows = conn.query(
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
            &[&schema_clone, &table_clone, &constraint_name],
        )
        .await
        .map_err(|e| format!("Failed to query foreign key columns: {}", e))?;
        
        let mut local_cols = Vec::new();
        let mut ref_cols = Vec::new();
        for row in fk_rows {
            local_cols.push(row.get::<_, String>("column_name"));
            ref_cols.push(row.get::<_, String>("ref_column"));
        }
        
        rows.push(vec!["Local Columns".to_string(), local_cols.join(", ")]);
        rows.push(vec!["Referenced Columns".to_string(), ref_cols.join(", ")]);
        
        Ok((columns, rows))
    });
    
    match result {
        Ok((columns, rows)) => {
            results_table.query_columns = columns;
            results_table.query_rows = rows;
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn query_trigger_detail(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, results_table: &mut ResultsTable, database: &str, schema: &str, table: &str, trigger: &str) {
    let dsn = get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let schema_clone = schema.to_string();
    let trigger_clone = trigger.to_string();
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn_clone)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        let triggers = session.get_trigger_info(&schema_clone, &trigger_clone)
            .await
            .map_err(|e| format!("Failed to get trigger info: {}", e))?;
        
        let trigger_info = triggers.into_iter()
            .find(|t| t.name == trigger_clone && t.table_name == table)
            .ok_or_else(|| format!("Trigger '{}' not found", trigger))?;
        
        // Convert to table format
        let columns = vec!["Property".to_string(), "Value".to_string()];
        let mut rows = Vec::new();
        
        rows.push(vec!["Name".to_string(), trigger_info.name.clone()]);
        rows.push(vec!["Table".to_string(), format!("{}.{}", trigger_info.table_schema, trigger_info.table_name)]);
        rows.push(vec!["Timing".to_string(), trigger_info.timing.clone()]);
        rows.push(vec!["Events".to_string(), trigger_info.events.join(", ")]);
        rows.push(vec!["Enabled".to_string(), trigger_info.enabled.to_string()]);
        if let Some(ref func) = trigger_info.function_name {
            rows.push(vec!["Function".to_string(), func.clone()]);
        }
        if let Some(ref desc) = trigger_info.description {
            rows.push(vec!["Description".to_string(), desc.clone()]);
        }
        
        Ok((columns, rows))
    });
    
    match result {
        Ok((columns, rows)) => {
            results_table.query_columns = columns;
            results_table.query_rows = rows;
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}


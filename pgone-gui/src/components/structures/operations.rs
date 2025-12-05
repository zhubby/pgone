use super::types::DbTree;
use super::loading;
use super::utils;
use pgone_sql::Session;
use crate::futures;

pub(super) fn create_database(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, name: &str) {
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
            return;
        }
    } else {
        return;
    };
    
    let dsn_clone = dsn.clone();
    let name_clone = name.to_string();
    
    let result = futures::block_on_async(async {
        let session = Session::connect_to_postgres(&dsn_clone)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        session.create_database(&name_clone, None, None, None)
            .await
            .map_err(|e| format!("Failed to create database: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload databases
            tree.loaded_databases = false;
            loading::load_databases(tree, db_manager);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn create_schema(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, name: &str) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
    .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        session.create_schema(name, None)
            .await
            .map_err(|e| format!("Failed to create schema: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload schemas
            tree.loaded_schemas.insert(database.to_string(), false);
            loading::load_schemas(tree, db_manager, database);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn create_table(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, ddl: &str) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
    .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        session.create_table(ddl)
    .await
            .map_err(|e| format!("Failed to create table: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload tables
            let key = format!("{}.{}", database, schema);
            tree.loaded_tables.insert(key.clone(), false);
            loading::load_tables(tree, db_manager, database, schema);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn delete_database(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, name: &str, _cascade: bool) {
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
            return;
        }
    } else {
        return;
    };
    
    let dsn_clone = dsn.clone();
    let name_clone = name.to_string();
    
    let result = futures::block_on_async(async {
        let session = Session::connect_to_postgres(&dsn_clone)
    .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        session.drop_database(&name_clone, false)
            .await
            .map_err(|e| format!("Failed to delete database: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload databases
            tree.loaded_databases = false;
            loading::load_databases(tree, db_manager);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn delete_schema(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, name: &str, cascade: bool) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
    .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        session.drop_schema(name, false, cascade)
            .await
            .map_err(|e| format!("Failed to delete schema: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload schemas
            tree.loaded_schemas.insert(database.to_string(), false);
            loading::load_schemas(tree, db_manager, database);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn delete_table(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, name: &str, cascade: bool) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
            session.drop_table(schema, name, false, cascade)
                .await
                .map_err(|e| format!("Failed to delete table: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload tables
            let key = format!("{}.{}", database, schema);
            tree.loaded_tables.insert(key.clone(), false);
            loading::load_tables(tree, db_manager, database, schema);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn rename_database(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, old_name: &str, new_name: &str) {
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
            return;
        }
    } else {
        return;
    };
    
    let dsn_clone = dsn.clone();
    let old_name_clone = old_name.to_string();
    let new_name_clone = new_name.to_string();
    
    let result = futures::block_on_async(async {
        let session = Session::connect_to_postgres(&dsn_clone)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        session.alter_database(&old_name_clone, Some(&new_name_clone), None, None)
            .await
            .map_err(|e| format!("Failed to rename database: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload databases
            tree.loaded_databases = false;
            loading::load_databases(tree, db_manager);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn rename_schema(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, old_name: &str, new_name: &str) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        session.alter_schema(old_name, Some(new_name), None)
            .await
            .map_err(|e| format!("Failed to rename schema: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload schemas
            tree.loaded_schemas.insert(database.to_string(), false);
            loading::load_schemas(tree, db_manager, database);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn rename_table(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, old_name: &str, new_name: &str) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        session.alter_table(schema, old_name, &format!("RENAME TO {}", utils::quote_ident(new_name)))
            .await
            .map_err(|e| format!("Failed to rename table: {}", e))
    });
    
    match result {
        Ok(_) => {
            // Reload tables
            let key = format!("{}.{}", database, schema);
            tree.loaded_tables.insert(key.clone(), false);
            loading::load_tables(tree, db_manager, database, schema);
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

/// 执行表设计变更，使用事务确保原子性
pub(super) fn design_table(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    _table_name: &str,
    statements: &[String],
) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let dsn_clone = dsn.clone();
    let statements_clone = statements.to_vec();
    
    let result = futures::block_on_async(async {
        use tokio_postgres::NoTls;
        
        // 直接连接数据库以支持事务
        let (mut client, connection) = tokio_postgres::connect(&dsn_clone, NoTls)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // 在后台运行连接任务
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(error = ?e, "Database connection error");
            }
        });
        
        // 开始事务
        let transaction = client
            .transaction()
            .await
            .map_err(|e| format!("Failed to begin transaction: {}", e))?;
        
        // 执行所有 SQL 语句
        for sql in &statements_clone {
            transaction
                .execute(sql, &[])
                .await
                .map_err(|e| format!("Failed to execute SQL: {} - Error: {}", sql, e))?;
        }
        
        // 提交事务
        transaction
            .commit()
            .await
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;
        
        Ok::<(), String>(())
    });
    
    match result {
        Ok(_) => {
            // 重新加载表结构
            let key = format!("{}.{}", database, schema);
            tree.loaded_tables.insert(key.clone(), false);
            loading::load_tables(tree, db_manager, database, schema);
            
            // 清除设计状态
            tree.design_table_detail = None;
            tree.design_table_columns.clear();
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

pub(super) fn drop_table(tree: &mut DbTree, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, name: &str) {
    let dsn = loading::get_dsn_for_database(tree, db_manager, database);
    let Some(dsn) = dsn else { return; };
    
    let result = futures::block_on_async(async {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        
        session.truncate_table(schema, name)
            .await
            .map_err(|e| format!("Failed to truncate table: {}", e))
    });
    
    match result {
        Ok(_) => {
            // TRUNCATE 不改变表结构，所以不需要重新加载表列表
            // 但可以显示成功消息或更新错误状态
        }
        Err(e) => {
            tree.error = Some(e);
        }
    }
}

